#!/usr/bin/env python3

import argparse
import hashlib
import json
from pathlib import Path

EXPECTED_VERUS_COMMIT = "426d8b01e7ffb910a36c15e1f870ff985bb61eef"
TERMINATION_GAPS: set[str] = set()
TERMINATION_GAP_SOURCES: dict[str, tuple[str, str]] = {}
RUST_ONLY_ADAPTERS = [
    {
        "function": "lib::error::<DecodeError as core::fmt::Display>::fmt",
        "disposition": "rust-standard-library-adapter",
    },
    {
        "function": "lib::error::<DecodeError as std::error::Error>",
        "disposition": "marker-trait-adapter",
    },
    {
        "function": "lib::error::<DecodeError as From<std::io::Error>>::from",
        "disposition": "rust-standard-library-adapter",
    },
    {
        "function": "lib::jp2::<BoxReader as Iterator>::next",
        "disposition": "delegates-to-verified-next_box_ref",
    },
    {
        "function": "lib::packet::<PacketIterator as Iterator>::next",
        "disposition": "delegates-to-verified-next_packet",
    },
]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def _substring_offsets(text: str, needle: str) -> list[int]:
    offsets = []
    offset = text.find(needle)
    while offset >= 0:
        offsets.append(offset)
        offset = text.find(needle, offset + len(needle))
    return offsets


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("verus_json", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()

    result = json.loads(args.verus_json.read_text(encoding="utf-8"))
    verification = result["verification-results"]
    verus = result["verus"]
    if not verification["success"] or verification["errors"] != 0:
        raise SystemExit("Verus did not complete successfully with zero errors")
    if not verification["is-verifying-entire-crate"]:
        raise SystemExit("partial Verus results cannot produce production coverage")
    if verus["commit"] != EXPECTED_VERUS_COMMIT:
        raise SystemExit(f"unexpected Verus commit: {verus['commit']}")

    functions = []
    for name, details in sorted(result["func-details"].items()):
        if not name.startswith("lib::"):
            continue
        if details["failed_proof_notes"]:
            raise SystemExit(f"failed proof notes for {name}")
        disposition = (
            "verified-with-termination-gap"
            if name in TERMINATION_GAPS
            else "verified"
        )
        functions.append({"function": name, "disposition": disposition})

    observed = {entry["function"] for entry in functions}
    if set(TERMINATION_GAP_SOURCES) != TERMINATION_GAPS:
        raise SystemExit("termination gap/source mappings disagree")
    missing_gaps = TERMINATION_GAPS - observed
    if missing_gaps:
        raise SystemExit(f"stale termination gaps: {sorted(missing_gaps)}")

    source_paths = []
    for source_directory in ("src", "specs", "proofs"):
        source_paths.extend((args.root / source_directory).glob("**/*.rs"))
    source_hashes = {
        str(path.relative_to(args.root)): sha256(path)
        for path in sorted(source_paths)
    }
    source_text = {
        str(path.relative_to(args.root)): path.read_text(encoding="utf-8")
        for path in source_paths
    }
    termination_attributes = {
        (source_path, offset)
        for source_path, text in source_text.items()
        for offset in _substring_offsets(
            text,
            "verifier::exec_allows_no_decreases_clause",
        )
    }
    mapped_attributes: set[tuple[str, int]] = set()
    for function, (source_path, signature) in TERMINATION_GAP_SOURCES.items():
        if source_path not in source_text:
            raise SystemExit(f"stale termination source path for {function}")
        text = source_text[source_path]
        signature_offset = text.find(signature)
        if signature_offset < 0:
            raise SystemExit(f"stale termination source mapping for {function}")
        attribute_offset = text.rfind(
            "verifier::exec_allows_no_decreases_clause",
            0,
            signature_offset,
        )
        if attribute_offset < 0 or signature_offset - attribute_offset > 160:
            raise SystemExit(f"missing termination-gap attribute for {function}")
        mapped_attribute = (source_path, attribute_offset)
        if mapped_attribute in mapped_attributes:
            raise SystemExit(
                f"duplicate termination source mapping at "
                f"{source_path}:{text.count(chr(10), 0, attribute_offset) + 1}"
            )
        mapped_attributes.add(mapped_attribute)
    unmapped_attributes = termination_attributes - mapped_attributes
    stale_attribute_mappings = mapped_attributes - termination_attributes
    if unmapped_attributes or stale_attribute_mappings:
        def locations(attributes: set[tuple[str, int]]) -> list[str]:
            return sorted(
                f"{source_path}:"
                f"{source_text[source_path].count(chr(10), 0, offset) + 1}"
                for source_path, offset in attributes
            )

        raise SystemExit(
            "termination attribute/source mappings are not exact: "
            f"unmapped={locations(unmapped_attributes)}, "
            f"stale={locations(stale_attribute_mappings)}"
        )
    for source_path, text in source_text.items():
        for forbidden in ("external_body", "assume(", "admit("):
            if forbidden in text:
                raise SystemExit(f"forbidden proof escape {forbidden!r} in {source_path}")
    coverage = {
        "schema_version": 1,
        "source_root": "src/lib.rs",
        "source_hashes": source_hashes,
        "verus": {
            "version": verus["version"],
            "commit": verus["commit"],
            "profile": verus["profile"],
            "toolchain": verus["toolchain"],
            "flags": ["--no-lifetime", "--crate-type", "lib"],
        },
        "results": {
            "verus_verified_obligations": verification["verified"],
            "verus_errors": verification["errors"],
            "project_functions_with_dispositions": len(functions)
            + len(RUST_ONLY_ADAPTERS),
            "verified_project_functions": len(functions),
            "incomplete_production_functions": len(TERMINATION_GAPS),
            "missing_dispositions": 0,
            "duplicated_dispositions": 0,
            "trusted_codec_functions": 0,
        },
        "known_proof_boundary": {
            "lifetime_analysis": (
                "disabled; a pinned run without --no-lifetime exits 1 with the "
                "VerusErasureCtxt/thir_body internal error; Rust borrow checking "
                "remains active"
            ),
            "termination": sorted(TERMINATION_GAPS),
        },
        "functions": functions,
        "rust_only_adapters": RUST_ONLY_ADAPTERS,
    }
    args.output.write_text(
        json.dumps(coverage, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
