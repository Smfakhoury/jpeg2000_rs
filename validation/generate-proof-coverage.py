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
EXPECTED_CRATE = "jpeg2000_rs"
RUST_ONLY_ADAPTERS = [
    {
        "function": "jpeg2000_rs::error::<DecodeError as core::fmt::Display>::fmt",
        "disposition": "rust-standard-library-adapter",
    },
    {
        "function": "jpeg2000_rs::error::<DecodeError as std::error::Error>",
        "disposition": "marker-trait-adapter",
    },
    {
        "function": "jpeg2000_rs::error::<DecodeError as From<std::io::Error>>::from",
        "disposition": "rust-standard-library-adapter",
    },
    {
        "function": "jpeg2000_rs::jp2::<BoxReader as Iterator>::next",
        "disposition": "delegates-to-verified-next_box_ref",
    },
    {
        "function": "jpeg2000_rs::packet::<PacketIterator as Iterator>::next",
        "disposition": "delegates-to-verified-next_packet",
    },
]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def load_cargo_verus_result(text: str) -> dict:
    """Accept one root JSON result, not Cargo success or dependency-only output."""
    decoder = json.JSONDecoder()
    results = []
    for start in re.finditer(r"(?m)^\{", text):
        try:
            result, _ = decoder.raw_decode(text, start.start())
        except json.JSONDecodeError as error:
            raise ValueError("malformed JSON in Cargo-Verus output") from error
        if not isinstance(result, dict) or "verification-results" not in result:
            raise ValueError("unexpected JSON in Cargo-Verus output")
        results.append(result)
    if len(results) != 1:
        raise ValueError("expected exactly one root verification JSON result from Cargo-Verus")
    return results[0]


def generate_coverage(result: object, root: Path) -> dict:
    if not isinstance(result, dict):
        raise ValueError("Verus result must be an object")
    verification = result.get("verification-results")
    verus = result.get("verus")
    details_by_function = result.get("func-details")
    if not isinstance(verification, dict) or not isinstance(verus, dict):
        raise ValueError("missing Verus verification/tool result")
    if verification.get("is-verifying-entire-crate") is not True:
        raise ValueError("partial Verus results cannot produce production coverage")
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
        if not name.startswith(EXPECTED_CRATE + "::"):
            continue
        if details.get("failed_proof_notes") != []:
            raise ValueError(f"missing or failed proof notes for {name}")
        if details.get("obligation_proof_notes") != []:
            raise ValueError(f"missing or unreviewed obligation notes for {name}")
        if set(details) != {"failed_proof_notes", "obligation_proof_notes"}:
            raise ValueError(f"unreviewed project function metadata for {name}")
        functions.append({"function": name, "disposition": "verified"})
    if not functions:
        raise ValueError(f"no {EXPECTED_CRATE} project functions in Verus result")

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
    for source_path, text in source_text.items():
        forbidden = re.search(
            r"\b(?:external_body|external_fn_specification|assume_specification|axiom)\b"
            r"|\bexec_allows_no_decreases_clause\b"
            r"|\b(?:assume|admit|assume_termination)\s*!?\s*\("
            r"|\bverifier\s*::\s*external\b"
            r"|\bverifier\s*::\s*verify\s*\(\s*false\b",
            text,
        )
        if forbidden:
            raise ValueError(f"forbidden proof escape {forbidden.group()!r} in {source_path}")
    coverage = {
        "$schema": "../../validation/proof-coverage.schema.json",
        "schema_version": 3,
        "crate_name": EXPECTED_CRATE,
        "verification_entrypoint": "cargo verus verify",
        "provenance": source_binding(root),
        "source_root": "src/lib.rs",
        "source_hashes": source_hashes,
        "verus": {
            "version": verus["version"],
            "commit": verus["commit"],
            "profile": verus["profile"],
            "toolchain": verus["toolchain"],
            "workflow_flags": ["--no-lifetime"],
        },
        "results": {
            "verus_verified_obligations": verification["verified"],
            "verus_errors": verification["errors"],
            "verified_project_functions": len(functions),
            "documented_rust_only_adapters": len(RUST_ONLY_ADAPTERS),
            "source_function_coverage": "not-measured",
        },
        "known_proof_boundary": {
            "lifetime_analysis": (
                "disabled by --no-lifetime for this stable-release workflow; "
                "this report does not verify lifetimes; Rust borrow checking "
                "is checked separately by Cargo"
            ),
            "termination": [],
        },
        "functions": functions,
        "rust_only_adapters": RUST_ONLY_ADAPTERS,
        "scope": (
            f"Observed {EXPECTED_CRATE}:: functions in the supplied whole-crate Cargo-Verus result; "
            "source-token trust scan is conservative, not a Rust parser. "
            "Rust-only adapters are documented, not verified by this report. "
            "No exhaustive source-function coverage claim is inferred."
        ),
    }
    validate_schema(Path(__file__).with_name("proof-coverage.schema.json"), coverage)
    return coverage


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("cargo_output", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    try:
        result = load_cargo_verus_result(args.cargo_output.read_text(encoding="utf-8"))
        coverage = generate_coverage(result, args.root)
        normalized = report_path(args.output_dir, "verus-result.json")
        if args.cargo_output.resolve() != normalized:
            write_json(normalized, result)
        coverage["verus_result_sha256"] = sha256(normalized)
        coverage["cargo_verus_output_sha256"] = sha256(args.cargo_output)
        coverage["$schema"] = report_schema(args.output_dir, "proof-coverage.schema.json")
        write_json(report_path(args.output_dir, "proof-coverage.json"), coverage)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"proof coverage error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
