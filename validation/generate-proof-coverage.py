#!/usr/bin/env python3

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

from framework import report_path, report_schema, source_binding, validate_schema, write_json

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


def generate_coverage(result: object, root: Path) -> dict:
    if not isinstance(result, dict):
        raise ValueError("Verus result must be an object")
    verification = result.get("verification-results")
    verus = result.get("verus")
    details_by_function = result.get("func-details")
    if not isinstance(verification, dict) or not isinstance(verus, dict):
        raise ValueError("missing Verus verification/tool result")
    if (
        verification.get("success") is not True
        or type(verification.get("errors")) is not int
        or verification["errors"] != 0
        or type(verification.get("verified")) is not int
        or verification["verified"] <= 0
        or verification.get("encountered-error") is not False
        or verification.get("encountered-vir-error") is not False
    ):
        raise ValueError("Verus did not complete successfully with nonzero obligations and zero errors")
    if verification.get("is-verifying-entire-crate") is not True:
        raise ValueError("partial Verus results cannot produce production coverage")
    if not all(isinstance(verus.get(key), str) and verus[key] for key in ("version", "commit", "profile", "toolchain")):
        raise ValueError("missing Verus tool identity")
    if verus["commit"] != EXPECTED_VERUS_COMMIT:
        raise ValueError(f"unexpected Verus commit: {verus['commit']}")
    if not isinstance(details_by_function, dict) or not details_by_function:
        raise ValueError("empty or invalid Verus function results")

    functions = []
    for name, details in sorted(details_by_function.items()):
        if not isinstance(name, str) or not isinstance(details, dict):
            raise ValueError("invalid Verus function detail")
        if not name.startswith("lib::"):
            continue
        if details.get("failed_proof_notes") != []:
            raise ValueError(f"missing or failed proof notes for {name}")
        if details.get("obligation_proof_notes") != []:
            raise ValueError(f"missing or unreviewed obligation notes for {name}")
        if set(details) != {"failed_proof_notes", "obligation_proof_notes"}:
            raise ValueError(f"unreviewed project function metadata for {name}")
        disposition = (
            "verified-with-termination-gap"
            if name in TERMINATION_GAPS
            else "verified"
        )
        functions.append({"function": name, "disposition": disposition})
    if not functions:
        raise ValueError("no project functions in Verus result")

    observed = {entry["function"] for entry in functions}
    if set(TERMINATION_GAP_SOURCES) != TERMINATION_GAPS:
        raise ValueError("termination gap/source mappings disagree")
    missing_gaps = TERMINATION_GAPS - observed
    if missing_gaps:
        raise ValueError(f"stale termination gaps: {sorted(missing_gaps)}")

    source_paths = []
    for source_directory in ("src", "specs", "proofs"):
        source_paths.extend((root / source_directory).glob("**/*.rs"))
    if not (root / "src/lib.rs").is_file() or not source_paths:
        raise ValueError("missing project source root")
    source_hashes = {
        str(path.relative_to(root)): sha256(path)
        for path in sorted(source_paths)
    }
    source_text = {
        str(path.relative_to(root)): path.read_text(encoding="utf-8")
        for path in source_paths
    }
    termination_attributes = {
        (source_path, offset)
        for source_path, text in source_text.items()
        for offset in (
            match.start() for match in re.finditer(r"\bexec_allows_no_decreases_clause\b", text)
        )
    }
    mapped_attributes: set[tuple[str, int]] = set()
    for function, (source_path, signature) in TERMINATION_GAP_SOURCES.items():
        if source_path not in source_text:
            raise ValueError(f"stale termination source path for {function}")
        text = source_text[source_path]
        signature_offset = text.find(signature)
        if signature_offset < 0:
            raise ValueError(f"stale termination source mapping for {function}")
        attribute_offset = text.rfind("exec_allows_no_decreases_clause", 0, signature_offset)
        if attribute_offset < 0 or signature_offset - attribute_offset > 160:
            raise ValueError(f"missing termination-gap attribute for {function}")
        mapped_attribute = (source_path, attribute_offset)
        if mapped_attribute in mapped_attributes:
            raise ValueError(
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

        raise ValueError(
            "termination attribute/source mappings are not exact: "
            f"unmapped={locations(unmapped_attributes)}, "
            f"stale={locations(stale_attribute_mappings)}"
        )
    for source_path, text in source_text.items():
        forbidden = re.search(
            r"\b(?:external_body|external_fn_specification|assume_specification|axiom)\b"
            r"|\b(?:assume|admit|assume_termination)\s*!?\s*\("
            r"|\bverifier\s*::\s*external\b"
            r"|\bverifier\s*::\s*verify\s*\(\s*false\b",
            text,
        )
        if forbidden:
            raise ValueError(f"forbidden proof escape {forbidden.group()!r} in {source_path}")
    coverage = {
        "$schema": "../../validation/proof-coverage.schema.json",
        "schema_version": 1,
        "provenance": source_binding(root),
        "source_root": "src/lib.rs",
        "source_hashes": source_hashes,
        "verus": {
            "version": verus["version"],
            "commit": verus["commit"],
            "profile": verus["profile"],
            "toolchain": verus["toolchain"],
            "workflow_flags": ["--no-lifetime", "--crate-type", "lib"],
        },
        "results": {
            "verus_verified_obligations": verification["verified"],
            "verus_errors": verification["errors"],
            "verified_project_functions": len(functions),
            "documented_rust_only_adapters": len(RUST_ONLY_ADAPTERS),
            "incomplete_production_functions": len(TERMINATION_GAPS),
        },
        "known_proof_boundary": {
            "lifetime_analysis": (
                "disabled by --no-lifetime for this stable-release workflow; "
                "this report does not verify lifetimes; Rust borrow checking "
                "is checked separately by Cargo"
            ),
            "termination": sorted(TERMINATION_GAPS),
        },
        "functions": functions,
        "rust_only_adapters": RUST_ONLY_ADAPTERS,
        "scope": (
            "Observed lib:: functions in the supplied whole-crate Verus result; "
            "source-token trust scan is conservative, not a Rust parser. "
            "Rust-only adapters are documented, not verified by this report. "
            "No exhaustive source-function coverage claim is inferred."
        ),
    }
    validate_schema(Path(__file__).with_name("proof-coverage.schema.json"), coverage)
    return coverage


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("verus_json", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    try:
        result = json.loads(args.verus_json.read_text(encoding="utf-8"))
        coverage = generate_coverage(result, args.root)
        coverage["verus_result_sha256"] = sha256(args.verus_json)
        coverage["$schema"] = report_schema(args.output_dir, "proof-coverage.schema.json")
        write_json(report_path(args.output_dir, "proof-coverage.json"), coverage)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"proof coverage error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
