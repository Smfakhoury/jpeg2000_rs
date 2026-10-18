#!/usr/bin/env python3
"""Reproducible decoder-validation and release-evidence support."""

from __future__ import annotations

import hashlib
import json
import os
import re
import selectors
import shutil
import struct
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise RuntimeError(
        "schema validation requires validation/requirements.txt"
    ) from error

try:
    import resource
except ImportError:  # pragma: no cover - unavailable on Windows
    resource = None

ROOT = Path(__file__).resolve().parents[1]
VALIDATION = ROOT / "validation"
CORPUS_DIR = VALIDATION / "generated" / "corpus"
RESULTS_DIR = VALIDATION / "results"
MANIFEST_PATH = VALIDATION / "corpus.json"
PROFILE_ID = "jpeg2000-rs-first-release-proposed"
GENERATOR_ID = "jpeg2000-rs-validation-generator-v1"
SOURCE_TREE_PATH = ROOT / "source-tree.json"
ARCHITECTURE_REVIEW_PATH = ROOT / "architecture-review.json"
ADJUDICATION_PATH = RESULTS_DIR / "oracle-adjudications.json"
OFFICIAL_RESULT_PATH = RESULTS_DIR / "official-vector-results.json"
OFFICIAL_VECTOR_ROOT_ENV = "JPEG2000_OFFICIAL_VECTOR_ROOT"
OFFICIAL_VECTOR_MANIFEST_ENV = "JPEG2000_OFFICIAL_VECTOR_MANIFEST"
SOURCE_TREE_ROOT_FILES = (
    ".github/workflows/ci.yml",
    ".github/workflows/verus.yml",
    "Cargo.lock",
    "Cargo.toml",
    "README.md",
    "RELEASING.md",
    "SECURITY.md",
    "THREAT_MODEL.md",
    "architecture-review.schema.json",
    "fuzz/Cargo.lock",
    "fuzz/Cargo.toml",
    "legal-publication-review.schema.json",
    "lifetime-analysis-blocker.json",
    "normative-coverage.json",
    "normative-coverage.schema.json",
    "owner-approval.schema.json",
    "release-attestation.schema.json",
    "rust-toolchain.toml",
    "standards-profile.json",
    "support-matrix.json",
    "support-matrix.schema.json",
    "unsupported-boundary-audit.json",
    "validation/VALIDATION.md",
    "validation/budget-result.schema.json",
    "validation/corpus.schema.json",
    "validation/expected-package-files.txt",
    "validation/framework.py",
    "validation/oracle-adjudication.schema.json",
    "validation/oracle-config.schema.json",
    "validation/oracles.json",
    "validation/official-vector-manifest.schema.json",
    "validation/official-vector-result.schema.json",
    "validation/requirements.txt",
    "validation/run-validation.py",
    "validation/validation-result.schema.json",
    "validation/verus-lifetime-reproducer.rs",
)
SOURCE_TREE_DIRECTORIES = (
    "examples",
    "fuzz/fuzz_targets",
    "proofs",
    "specs",
    "src",
    "validation/tests",
)


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json.dumps(value, indent=2, sort_keys=True).encode() + b"\n")


def _tree_entries(paths: list[Path]) -> list[dict[str, Any]]:
    return [
        {
            "path": path.relative_to(ROOT).as_posix(),
            "bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
        for path in sorted(paths, key=lambda item: item.relative_to(ROOT).as_posix())
    ]


def _tree_revision(entries: list[dict[str, Any]]) -> str:
    payload = {"algorithm": "sha256-tree-v1", "files": entries}
    return "sha256-tree-v1:" + sha256_bytes(canonical_bytes(payload))


def build_source_tree_manifest() -> dict[str, Any]:
    paths = [ROOT / relative for relative in SOURCE_TREE_ROOT_FILES]
    missing = [
        path.relative_to(ROOT).as_posix()
        for path in paths
        if not path.is_file()
    ]
    if missing:
        raise FileNotFoundError("missing source-tree inputs: " + ", ".join(missing))
    for relative in SOURCE_TREE_DIRECTORIES:
        directory = ROOT / relative
        if not directory.is_dir():
            missing.append(relative)
            continue
        paths.extend(
            path
            for path in directory.rglob("*")
            if path.is_file() and "__pycache__" not in path.parts
        )
    if missing:
        raise FileNotFoundError("missing source-tree inputs: " + ", ".join(missing))
    entries = _tree_entries(paths)
    production_entries = _tree_entries(
        [path for path in (ROOT / "src").rglob("*.rs") if path.is_file()]
    )
    return {
        "schema_version": 1,
        "algorithm": "sha256-tree-v1",
        "source_revision": _tree_revision(entries),
        "production_source_revision": _tree_revision(production_entries),
        "scope": {
            "included": list(SOURCE_TREE_ROOT_FILES)
            + [f"{path}/**" for path in SOURCE_TREE_DIRECTORIES],
            "excluded_generated_evidence": [
                "source-tree.json",
                "*-attestation.json",
                "validation/generated/**",
                "validation/results/**",
                "target/**",
                "fuzz/target/**",
                ".venv/**",
            ],
        },
        "files": entries,
        "production_files": production_entries,
    }


def write_source_tree_manifest(check: bool = False) -> dict[str, Any]:
    manifest = build_source_tree_manifest()
    expected = json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    if check:
        if not SOURCE_TREE_PATH.is_file() or SOURCE_TREE_PATH.read_text() != expected:
            raise ValueError("source-tree manifest drift")
    else:
        SOURCE_TREE_PATH.write_text(expected)
    return manifest


def validate_evidence_schemas() -> dict[str, Any]:
    pairs = [
        ("support-matrix.schema.json", "support-matrix.json"),
        ("normative-coverage.schema.json", "normative-coverage.json"),
        ("release-attestation.schema.json", "proof-attestation.json"),
        ("release-attestation.schema.json", "fuzz-attestation.json"),
        ("release-attestation.schema.json", "conformance-attestation.json"),
        ("release-attestation.schema.json", "release-attestation.json"),
        ("architecture-review.schema.json", "architecture-review.json"),
        (
            "legal-publication-review.schema.json",
            "legal-publication-review.json",
        ),
        ("owner-approval.schema.json", "owner-approval.json"),
        ("validation/corpus.schema.json", "validation/corpus.json"),
        (
            "validation/validation-result.schema.json",
            "validation/results/validation-results.json",
        ),
        (
            "validation/budget-result.schema.json",
            "validation/results/budget-results.json",
        ),
        (
            "validation/oracle-adjudication.schema.json",
            "validation/results/oracle-adjudications.json",
        ),
        (
            "validation/official-vector-result.schema.json",
            "validation/results/official-vector-results.json",
        ),
    ]
    checked = []
    for schema_name, instance_name in pairs:
        schema = json.loads((ROOT / schema_name).read_text())
        instance = json.loads((ROOT / instance_name).read_text())
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(instance)
        checked.append({"schema": schema_name, "instance": instance_name})
    official_manifest_schema = json.loads(
        (VALIDATION / "official-vector-manifest.schema.json").read_text()
    )
    Draft202012Validator.check_schema(official_manifest_schema)
    checked.append(
        {
            "schema": "validation/official-vector-manifest.schema.json",
            "instance": "external-only",
        }
    )
    validate_normative_coverage()
    return {"status": "passed", "checked": checked}


def _coverage_symbol_present(path: Path, symbol: str) -> bool:
    text = path.read_text(encoding="utf-8")
    short = symbol.rsplit("::", 1)[-1]
    return (
        f"fn {short}" in text
        or f"struct {short}" in text
        or f"enum {short}" in text
        or symbol in text
    )


def validate_normative_coverage(
    coverage: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if coverage is None:
        coverage = json.loads((ROOT / "normative-coverage.json").read_text())
    requirements = coverage.get("requirements", [])
    linked = [
        entry
        for entry in requirements
        if entry.get("status") == "functionally_linked"
    ]
    contract_functions: set[tuple[str, str]] = set()
    for entry in linked:
        for field in (
            "implementation_refs",
            "spec_refs",
            "contract_refs",
            "proof_refs",
            "test_refs",
        ):
            refs = entry.get(field)
            if not refs:
                raise ValueError(
                    f"{entry.get('id', '<unknown>')} lacks {field} evidence"
                )
            for reference in refs:
                path = ROOT / reference["path"]
                if not path.is_file():
                    raise ValueError(f"missing normative evidence path: {reference['path']}")
                if not _coverage_symbol_present(path, reference["symbol"]):
                    raise ValueError(
                        "missing normative evidence symbol: "
                        f"{reference['path']}::{reference['symbol']}"
                    )
        spec_symbols = {
            reference["symbol"] for reference in entry["spec_refs"]
        }
        for reference in entry["contract_refs"]:
            path = ROOT / reference["path"]
            text = path.read_text(encoding="utf-8")
            short = reference["symbol"].rsplit("::", 1)[-1]
            signature = text.find(f"fn {short}")
            if signature < 0:
                raise ValueError(f"missing linked contract function: {short}")
            contract_start = text.rfind("#[cfg_attr", 0, signature)
            contract = text[contract_start:signature] if contract_start >= 0 else ""
            if "verus_spec(" not in contract or "ensures returns" not in contract:
                raise ValueError(
                    f"linked function lacks executable Verus contract: {short}"
                )
            if not any(symbol in contract for symbol in spec_symbols):
                raise ValueError(
                    f"linked function contract lacks normative spec reference: {short}"
                )
            if not any(
                reference["symbol"] in (ROOT / implementation["path"]).read_text(
                    encoding="utf-8"
                )
                for implementation in entry["implementation_refs"]
            ):
                raise ValueError(
                    f"linked function is not used by claimed implementation: {short}"
                )
            contract_functions.add((reference["path"], reference["symbol"]))
    summary = coverage.get("summary", {})
    if summary.get("functionally_linked_requirements") != len(linked):
        raise ValueError("normative linked-requirement summary drift")
    if summary.get("concrete_linked_functions") != len(contract_functions):
        raise ValueError("normative linked-function summary drift")
    unlinked = coverage.get("unlinked_part1_requirement_groups", [])
    if summary.get("unlinked_part1_requirement_groups") != len(unlinked):
        raise ValueError("normative unlinked-group summary drift")
    if coverage.get("part1_closure_complete") and unlinked:
        raise ValueError("complete Part1 closure cannot retain unlinked groups")
    unlinked_part2 = coverage.get("unlinked_selected_part2_requirement_groups", [])
    if summary.get("unlinked_selected_part2_requirement_groups") != len(
        unlinked_part2
    ):
        raise ValueError("normative selected-Part2 unlinked-group summary drift")
    if coverage.get("selected_part2_closure_complete") and unlinked_part2:
        raise ValueError("complete selected Part2 closure cannot retain unlinked groups")
    unlinked_part9 = coverage.get("unlinked_selected_part9_requirement_groups", [])
    if summary.get("unlinked_selected_part9_requirement_groups") != len(
        unlinked_part9
    ):
        raise ValueError("normative selected-Part9 unlinked-group summary drift")
    if coverage.get("selected_part9_closure_complete") and unlinked_part9:
        raise ValueError("complete selected Part9 closure cannot retain unlinked groups")
    unlinked_part15 = coverage.get(
        "unlinked_selected_part15_requirement_groups", []
    )
    if summary.get("unlinked_selected_part15_requirement_groups") != len(
        unlinked_part15
    ):
        raise ValueError("normative selected-Part15 unlinked-group summary drift")
    if coverage.get("selected_part15_closure_complete") and unlinked_part15:
        raise ValueError("complete selected Part15 closure cannot retain unlinked groups")
    expected_part9 = {
        "part9-bin-id-vbas-aux-eor",
        "part9-databin-identity",
        "part9-fragment-final-semantics",
        "part9-transactional-overlap",
        "part9-interval-completeness",
        "part9-range-availability",
        "part9-metadata-placeholder-assembly",
        "part9-codestream-reconstruction",
        "part9-automatic-precinct-mapping",
        "part9-packet-reframing",
        "part9-later-tile-state",
        "part9-progressive-updates",
    }
    actual_part9 = {
        entry["id"] for entry in linked if entry.get("id", "").startswith("part9-")
    }
    if actual_part9 != expected_part9:
        raise ValueError(
            "selected Part9 requirement inventory drift: "
            f"missing={sorted(expected_part9 - actual_part9)}, "
            f"stale={sorted(actual_part9 - expected_part9)}"
        )
    expected_part15 = {
        "part15-jph-branding",
        "part15-profile-signaling",
        "part15-capability-constraints",
        "part15-pass-grouping",
        "part15-mixed-routing",
        "part15-cleanup-partition",
        "part15-stuffing-and-cursors",
        "part15-mel-vlc-uvlc-quads",
        "part15-magsgn-reconstruction",
        "part15-transactional-stream-writes",
        "part15-refinement-traversal",
        "part15-pass-sets-and-continuation",
    }
    actual_part15 = {
        entry["id"]
        for entry in linked
        if entry.get("id", "").startswith("part15-")
    }
    if actual_part15 != expected_part15:
        raise ValueError(
            "selected Part15 requirement inventory drift: "
            f"missing={sorted(expected_part15 - actual_part15)}, "
            f"stale={sorted(actual_part15 - expected_part15)}"
        )
    accessibility = coverage.get("part9_source_accessibility", [])
    classifications = {entry.get("group"): entry for entry in accessibility}
    expected_accessibility = {
        "bounded_non_network_semantics": (
            "selected",
            "authoritative_source_accessible",
        ),
        "network_tls_auth_request_server": (
            "approved_out_of_scope",
            "authoritative_source_accessible",
        ),
        "cache_signaling_syntax": (
            "approved_out_of_scope",
            "authoritative_source_inaccessible",
        ),
    }
    if set(classifications) != set(expected_accessibility):
        raise ValueError("Part9 source-accessibility inventory drift")
    for group, (selection, classification) in expected_accessibility.items():
        entry = classifications[group]
        if (
            entry.get("selection") != selection
            or entry.get("classification") != classification
        ):
            raise ValueError(f"invalid Part9 source-accessibility classification: {group}")
    part15_accessibility = coverage.get("part15_source_accessibility", [])
    part15_classifications = {
        entry.get("group"): entry for entry in part15_accessibility
    }
    expected_part15_accessibility = {
        "selected_executable_htj2k_semantics": (
            "selected",
            "authoritative_source_accessible",
        ),
        "profile_number_registry_interpretation": (
            "approved_out_of_scope",
            "authoritative_source_inaccessible",
        ),
    }
    if set(part15_classifications) != set(expected_part15_accessibility):
        raise ValueError("Part15 source-accessibility inventory drift")
    for group, (selection, classification) in expected_part15_accessibility.items():
        entry = part15_classifications[group]
        if (
            entry.get("selection") != selection
            or entry.get("classification") != classification
        ):
            raise ValueError(
            f"invalid Part15 source-accessibility classification: {group}"
            )
    refinement_path = ROOT / "src/refinement.rs"
    refinement_text = refinement_path.read_text(encoding="utf-8")
    executable_contracts = {
        ("src/refinement.rs", symbol)
        for symbol in re.findall(
            r"pub\(crate\)\s+const\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)",
            refinement_text,
        )
    }
    if contract_functions != executable_contracts:
        missing = sorted(executable_contracts - contract_functions)
        stale = sorted(contract_functions - executable_contracts)
        raise ValueError(
            f"normative executable-contract inventory drift: "
            f"missing={missing}, stale={stale}"
        )
    return {
        "status": "passed",
        "functionally_linked_requirements": len(linked),
        "concrete_linked_functions": len(contract_functions),
        "unlinked_part1_requirement_groups": len(unlinked),
        "unlinked_selected_part2_requirement_groups": len(unlinked_part2),
        "unlinked_selected_part9_requirement_groups": len(unlinked_part9),
        "unlinked_selected_part15_requirement_groups": len(unlinked_part15),
    }


def build_architecture_review() -> dict[str, Any]:
    source_tree = build_source_tree_manifest()
    normative = validate_normative_coverage()
    audit = json.loads((ROOT / "unsupported-boundary-audit.json").read_text())
    audit_summary = audit["classification_summary"]
    proof = json.loads((ROOT / "production-proof-coverage.json").read_text())
    proof_results = proof["results"]
    lifetime = json.loads((ROOT / "lifetime-analysis-blocker.json").read_text())

    expected_normative = {
        "functionally_linked_requirements": 54,
        "concrete_linked_functions": 160,
        "unlinked_part1_requirement_groups": 0,
        "unlinked_selected_part2_requirement_groups": 0,
        "unlinked_selected_part9_requirement_groups": 0,
        "unlinked_selected_part15_requirement_groups": 0,
    }
    if any(normative.get(key) != value for key, value in expected_normative.items()):
        raise ValueError("architecture normative-coverage checklist failed")
    if (
        audit_summary.get(
            "class_c_selected_profile_unsupported_paths_remaining"
        )
        != 0
        or audit_summary.get("contradictions") != 0
    ):
        raise ValueError("architecture unsupported-boundary checklist failed")
    expected_zero_proof_results = (
        "incomplete_production_functions",
        "missing_dispositions",
        "duplicated_dispositions",
        "trusted_codec_functions",
    )
    if any(proof_results.get(key) != 0 for key in expected_zero_proof_results):
        raise ValueError("architecture proof-disposition checklist failed")
    if proof.get("known_proof_boundary", {}).get("termination") != []:
        raise ValueError("architecture termination checklist failed")
    if (
        lifetime.get("status") != "blocked"
        or "VerusErasureCtxt" not in lifetime["production_probe"].get("failure", "")
        or lifetime["production_probe"].get("exit_code") != 1
    ):
        raise ValueError("architecture lifetime-boundary checklist failed")

    return {
        "$schema": "./architecture-review.schema.json",
        "schema_version": 1,
        "status": "passed",
        "source_revision": source_tree["source_revision"],
        "review_kind": "local-evidence-review",
        "reviewer": "phase-m-local-validation",
        "reviewed_at": "2026-10-17T23:54:18Z",
        "checklist": [
            {
                "id": "release-scope-and-boundaries",
                "status": "passed",
                "evidence": [
                    "Cargo.toml",
                    "support-matrix.json",
                    "unsupported-boundary-audit.json",
                    "src/lib.rs",
                ],
                "finding": (
                    "The selected profile is decoder-only, publish remains "
                    "disabled, and encoding, network behavior, ICC execution, "
                    "and other unsupported capabilities are explicit."
                ),
            },
            {
                "id": "memory-safety-and-resource-limits",
                "status": "passed",
                "evidence": [
                    "Cargo.toml",
                    "src/limits.rs",
                    "validation/results/budget-results.json",
                ],
                "finding": (
                    "Unsafe Rust is forbidden and decoder plus process-level "
                    "validation budgets bound untrusted input handling."
                ),
            },
            {
                "id": "typed-failure-model",
                "status": "passed",
                "evidence": [
                    "src/error.rs",
                    "support-matrix.json",
                    "unsupported-boundary-audit.json",
                    "validation/results/validation-results.json",
                ],
                "finding": (
                    "Unsupported and malformed inputs use typed errors and the "
                    "generated negative corpus exercises public format boundaries."
                ),
            },
            {
                "id": "selected-parts-functional-refinement",
                "status": "passed",
                "evidence": [
                    "normative-coverage.json",
                    "normative-coverage.schema.json",
                    "src/refinement.rs",
                    "specs/mod.rs",
                    "proofs/production_impl/mod.rs",
                ],
                "finding": (
                    "All 54 selected executable Parts 1, 2, 9, and 15 "
                    "requirement groups link to 160 concrete contracted "
                    "functions with zero unlinked selected groups."
                ),
            },
            {
                "id": "unsupported-boundary-audit",
                "status": "passed",
                "evidence": [
                    "support-matrix.json",
                    "unsupported-boundary-audit.json",
                    "src/error.rs",
                ],
                "finding": (
                    "The source audit has zero selected-profile unsupported "
                    "class-C paths and zero support-matrix contradictions."
                ),
            },
            {
                "id": "complete-proof-dispositions",
                "status": "passed",
                "evidence": [
                    "proof-attestation.json",
                    "production-proof-coverage.json",
                    "validation/generate-proof-coverage.py",
                ],
                "finding": (
                    "Every production function has one disposition with zero "
                    "termination gaps, incomplete functions, missing or duplicate "
                    "dispositions, and trusted codec functions."
                ),
            },
            {
                "id": "lifetime-analysis-proof-boundary",
                "status": "passed",
                "evidence": [
                    "lifetime-analysis-blocker.json",
                    "proof-attestation.json",
                    "source-tree.json",
                    "validation/verus-lifetime-reproducer.rs",
                ],
                "finding": (
                    "Lifetime analysis remains an explicit upstream Verus "
                    "internal-error boundary; Rust borrow checking remains active "
                    "and no proof assumption or source workaround was added."
                ),
            },
            {
                "id": "package-and-publication-boundary",
                "status": "passed",
                "evidence": [
                    "validation/expected-package-files.txt",
                    "Cargo.toml",
                    "RELEASING.md",
                ],
                "finding": (
                    "The package boundary excludes proofs, validation assets, "
                    "generated results, and native oracle binaries."
                ),
            },
            {
                "id": "independent-validation-and-claims",
                "status": "passed",
                "evidence": [
                    "validation/oracles.json",
                    "validation/results/oracle-adjudications.json",
                    "release-attestation.schema.json",
                ],
                "finding": (
                    "External observations remain distinct from official Part 4 "
                    "conformance and unresolved disagreements block release."
                ),
            },
        ],
    }


def write_architecture_review(check: bool = False) -> dict[str, Any]:
    review = build_architecture_review()
    expected = json.dumps(review, indent=2, sort_keys=True) + "\n"
    if check:
        if (
            not ARCHITECTURE_REVIEW_PATH.is_file()
            or ARCHITECTURE_REVIEW_PATH.read_text() != expected
        ):
            raise ValueError("architecture review drift")
    else:
        ARCHITECTURE_REVIEW_PATH.write_text(expected)
    return review


def segment(marker: int, payload: bytes) -> bytes:
    return struct.pack(">HH", marker, len(payload) + 2) + payload


def box(box_type: bytes, payload: bytes) -> bytes:
    return struct.pack(">I4s", len(payload) + 8, box_type) + payload


def raw_codestream() -> bytes:
    siz = (
        struct.pack(">HIIIIIIIIH", 0, 1, 1, 0, 0, 1, 1, 0, 0, 1)
        + bytes((7, 1, 1))
    )
    data = b"\xff\x4f"
    data += segment(0xFF51, siz)
    data += segment(0xFF52, bytes((0, 0, 0, 1, 0, 0, 0, 0, 0, 1)))
    data += segment(0xFF5C, bytes((0, 0x40)))
    sot_offset = len(data)
    data += segment(0xFF90, struct.pack(">HI2B", 0, 15, 0, 1))
    data += b"\xff\x93\x00"
    assert len(data) - sot_offset == 15
    return data + b"\xff\xd9"


def jp2_file(codestream: bytes, brand: bytes = b"jp2 ") -> bytes:
    signature = box(b"jP  ", b"\x0d\x0a\x87\x0a")
    ftyp = box(b"ftyp", brand + b"\0\0\0\0" + brand)
    ihdr = box(b"ihdr", struct.pack(">IIH4B", 1, 1, 1, 7, 7, 0, 0))
    colr = box(b"colr", bytes((1, 0, 0, 0, 0, 0, 17)))
    return signature + ftyp + box(b"jp2h", ihdr + colr) + box(b"jp2c", codestream)


def jph_file() -> bytes:
    siz = (
        struct.pack(">HIIIIIIIIH", 0x4000, 1, 1, 0, 0, 1, 1, 0, 0, 1)
        + bytes((7, 1, 1))
    )
    codestream = b"\xff\x4f"
    codestream += segment(0xFF51, siz)
    codestream += segment(0xFF50, bytes((0, 2, 0, 0, 0, 8)))
    codestream += segment(0xFF52, bytes((0, 0, 0, 1, 0, 0, 0, 0, 0x40, 1)))
    codestream += segment(0xFF5C, bytes((0, 0x40)))
    codestream += segment(0xFF90, struct.pack(">HI2B", 0, 14, 0, 1))
    codestream += b"\xff\x93\xff\xd9"
    return jp2_file(codestream, b"jph ")


def jpx_file(codestream: bytes) -> bytes:
    signature = box(b"jP  ", b"\x0d\x0a\x87\x0a")
    ftyp = box(b"ftyp", b"jpxb\0\0\0\0jpxb")
    ihdr = box(b"ihdr", struct.pack(">IIH4B", 1, 1, 1, 7, 7, 0, 0))
    colr = box(b"colr", bytes((1, 0, 0, 0, 0, 0, 17)))
    return signature + ftyp + box(b"jp2h", ihdr + colr) + box(b"jp2c", codestream)


def jpip_response() -> bytes:
    # Explicit class/codestream form, one metadata byte, then EOR reason 1.
    return bytes((0x72, 0x08, 0x00, 0x00, 0x01, ord("x"), 0x00, 0x01, 0x00))


@dataclass(frozen=True)
class GeneratedCase:
    case_id: str
    format: str
    kind: str
    data: bytes
    expected: str
    operation: str = "decode"
    parent: str | None = None
    relation: str | None = None


def generated_cases() -> list[GeneratedCase]:
    raw = raw_codestream()
    jp2 = jp2_file(raw)
    jph = jph_file()
    jpx = jpx_file(raw)
    jpip = jpip_response()
    positives = [
        GeneratedCase("raw-minimal", "raw", "positive", raw, "accept"),
        GeneratedCase("jp2-minimal", "jp2", "positive", jp2, "accept"),
        GeneratedCase("jph-minimal-profile", "jph", "positive", jph, "accept", "inspect"),
        GeneratedCase("jpx-minimal", "jpx", "positive", jpx, "accept"),
        GeneratedCase("jpip-minimal-response", "jpip", "positive", jpip, "accept", "ingest"),
    ]
    cases = list(positives)
    for positive in positives:
        prefix_length = max(1, min(12, len(positive.data) // 2))
        cases.append(
            GeneratedCase(
                f"{positive.case_id}-truncated-prefix",
                positive.format,
                "malformed-prefix",
                positive.data[:prefix_length],
                "reject",
                positive.operation,
            )
        )
    cases.extend(
        [
            GeneratedCase(
                "raw-invalid-soc", "raw", "negative", b"\x00\x4f" + raw[2:], "reject"
            ),
            GeneratedCase(
                "jp2-invalid-signature",
                "jp2",
                "negative",
                jp2[:8] + b"\0\0\0\0" + jp2[12:],
                "reject",
            ),
            GeneratedCase(
                "jph-contradictory-brand",
                "jph",
                "negative",
                jph.replace(b"jph \0\0\0\0jph ", b"jph \0\0\0\0jp2 ", 1),
                "reject",
                "inspect",
            ),
            GeneratedCase(
                "jpx-invalid-box-length",
                "jpx",
                "negative",
                b"\0\0\0\x07" + jpx[4:],
                "reject",
            ),
            GeneratedCase(
                "jpip-overlong-vbas",
                "jpip",
                "negative",
                bytes((0x40, 0x80)),
                "reject",
                "ingest",
            ),
            GeneratedCase(
                "raw-comment-metamorphic",
                "raw",
                "metamorphic",
                raw[: raw.index(b"\xff\x90")]
                + segment(0xFF64, b"generated")
                + raw[raw.index(b"\xff\x90") :],
                "accept",
                parent="raw-minimal",
                relation="same-normalized-output",
            ),
            GeneratedCase(
                "jp2-free-box-metamorphic",
                "jp2",
                "metamorphic",
                jp2[:32] + box(b"free", b"generated") + jp2[32:],
                "accept",
                parent="jp2-minimal",
                relation="same-normalized-output",
            ),
        ]
    )
    return cases


def build_manifest(cases: list[GeneratedCase]) -> dict[str, Any]:
    entries = []
    for case in cases:
        suffix = {
            "raw": ".j2k",
            "jp2": ".jp2",
            "jph": ".jph",
            "jpx": ".jpx",
            "jpip": ".jpip",
        }[case.format]
        relative = f"generated/corpus/{case.case_id}{suffix}"
        entry: dict[str, Any] = {
            "id": case.case_id,
            "path": relative,
            "format": case.format,
            "kind": case.kind,
            "operation": case.operation,
            "expected": case.expected,
            "sha256": sha256_bytes(case.data),
            "bytes": len(case.data),
            "classification": "generated",
            "redistribution": "approved",
            "license": "project-generated-no-external-rights",
            "generator": {
                "id": GENERATOR_ID,
                "recipe": case.case_id,
                "inputs": [],
            },
        }
        if case.parent:
            entry["metamorphic"] = {"parent": case.parent, "relation": case.relation}
        entries.append(entry)
    return {
        "$schema": "./corpus.schema.json",
        "schema_version": 1,
        "profile_id": PROFILE_ID,
        "policy": {
            "official_vectors_included": False,
            "restricted_vectors_included": False,
            "allowed_classifications": ["generated"],
        },
        "generator": {
            "id": GENERATOR_ID,
            "source": "validation/framework.py",
            "deterministic": True,
        },
        "entries": entries,
    }


def generate_corpus(check: bool = False) -> dict[str, Any]:
    cases = generated_cases()
    manifest = build_manifest(cases)
    mismatches: list[str] = []
    for case, entry in zip(cases, manifest["entries"]):
        path = VALIDATION / entry["path"]
        if check:
            if not path.is_file() or path.read_bytes() != case.data:
                mismatches.append(entry["path"])
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(case.data)
    expected_manifest = json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    if check:
        if not MANIFEST_PATH.is_file() or MANIFEST_PATH.read_text() != expected_manifest:
            mismatches.append(MANIFEST_PATH.relative_to(ROOT).as_posix())
        if mismatches:
            raise ValueError("generated corpus drift: " + ", ".join(mismatches))
    else:
        MANIFEST_PATH.write_text(expected_manifest)
    return manifest


def load_manifest() -> dict[str, Any]:
    return json.loads(MANIFEST_PATH.read_text())


def verify_manifest(manifest: dict[str, Any] | None = None) -> dict[str, Any]:
    manifest = manifest or load_manifest()
    errors: list[str] = []
    ids: set[str] = set()
    if manifest.get("schema_version") != 1:
        errors.append("unsupported schema_version")
    policy = manifest.get("policy", {})
    if policy.get("official_vectors_included") or policy.get("restricted_vectors_included"):
        errors.append("manifest may not include official or restricted vectors")
    entries = manifest.get("entries")
    if not isinstance(entries, list) or not entries:
        errors.append("entries must be a non-empty array")
        entries = []
    for entry in entries:
        case_id = entry.get("id")
        if not isinstance(case_id, str) or not case_id or case_id in ids:
            errors.append(f"invalid or duplicate id: {case_id!r}")
            continue
        ids.add(case_id)
        if entry.get("classification") != "generated":
            errors.append(f"{case_id}: only generated assets are permitted")
        if entry.get("redistribution") != "approved":
            errors.append(f"{case_id}: redistribution is not approved")
        if entry.get("generator", {}).get("id") != GENERATOR_ID:
            errors.append(f"{case_id}: unrecognized generator")
        relative = entry.get("path", "")
        path = (VALIDATION / relative).resolve()
        try:
            path.relative_to(CORPUS_DIR.resolve())
        except ValueError:
            errors.append(f"{case_id}: path escapes generated corpus")
            continue
        if not path.is_file():
            errors.append(f"{case_id}: missing file")
            continue
        data = path.read_bytes()
        if len(data) != entry.get("bytes"):
            errors.append(f"{case_id}: byte length mismatch")
        if sha256_bytes(data) != entry.get("sha256"):
            errors.append(f"{case_id}: SHA-256 mismatch")
    for entry in entries:
        relation = entry.get("metamorphic")
        if relation and relation.get("parent") not in ids:
            errors.append(f"{entry.get('id')}: unknown metamorphic parent")
    disk_files = {
        path.relative_to(VALIDATION).as_posix()
        for path in CORPUS_DIR.glob("*")
        if path.is_file()
    }
    manifest_files = {entry.get("path") for entry in entries}
    extras = sorted(disk_files - manifest_files)
    if extras:
        errors.append("unmanifested corpus files: " + ", ".join(extras))
    if errors:
        raise ValueError("; ".join(errors))
    return {
        "status": "passed",
        "entries": len(entries),
        "manifest_sha256": sha256_file(MANIFEST_PATH),
    }


def parse_subject_output(stdout: bytes, stderr: bytes, returncode: int) -> dict[str, Any]:
    text = stdout.decode("utf-8", "replace").strip()
    if returncode != 0 and not text.startswith("ERR\t"):
        return {
            "accepted": False,
            "category": "process-error",
            "returncode": returncode,
            "stderr": stderr.decode("utf-8", "replace")[:4096],
        }
    if text.startswith("ERR\t"):
        fields = text.split("\t", 2)
        return {
            "accepted": False,
            "category": fields[1] if len(fields) > 1 else "unknown",
        }
    if text.startswith("META\t"):
        return {"accepted": True, "metadata": text.split("\t")[1:]}
    lines = text.splitlines()
    if not lines or lines[0] != "IMAGE":
        return {"accepted": False, "category": "invalid-harness-output"}
    header = [int(value) for value in lines[1].split()]
    components = []
    for line in lines[2:]:
        fields = line.split("\t")
        samples = [] if len(fields) < 6 or not fields[5] else [
            int(value) for value in fields[5].split(",")
        ]
        components.append(
            {
                "width": int(fields[0]),
                "height": int(fields[1]),
                "precision": int(fields[2]),
                "signed": fields[3] == "1",
                "sample_kind": fields[4],
                "samples": samples,
            }
        )
    return {
        "accepted": True,
        "image": {
            "width": header[0],
            "height": header[1],
            "components": components,
        },
    }


def parse_pnm(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    offset = 0

    def token() -> bytes:
        nonlocal offset
        while offset < len(data):
            if data[offset : offset + 1] == b"#":
                offset = data.find(b"\n", offset)
                if offset < 0:
                    raise ValueError("unterminated PNM comment")
            elif data[offset : offset + 1].isspace():
                offset += 1
            else:
                break
        start = offset
        while offset < len(data) and not data[offset : offset + 1].isspace():
            offset += 1
        return data[start:offset]

    magic = token()
    if magic not in (b"P5", b"P6"):
        raise ValueError("only binary PGM/PPM outputs are supported")
    width, height, maximum = int(token()), int(token()), int(token())
    if offset >= len(data) or not data[offset : offset + 1].isspace():
        raise ValueError("missing PNM header delimiter")
    offset += 1
    if data[offset - 1 : offset] == b"\r" and data[offset : offset + 1] == b"\n":
        offset += 1
    channels = 1 if magic == b"P5" else 3
    sample_bytes = 1 if maximum < 256 else 2
    expected = width * height * channels * sample_bytes
    payload = data[offset:]
    if len(payload) != expected:
        raise ValueError("PNM payload length mismatch")
    values = list(payload) if sample_bytes == 1 else [
        int.from_bytes(payload[index : index + 2], "big")
        for index in range(0, len(payload), 2)
    ]
    components = []
    for channel in range(channels):
        components.append(
            {
                "width": width,
                "height": height,
                "precision": maximum.bit_length(),
                "signed": False,
                "sample_kind": "unsigned",
                "samples": values[channel::channels],
            }
        )
    return {
        "accepted": True,
        "image": {"width": width, "height": height, "components": components},
    }


def normalized_hash(outcome: dict[str, Any]) -> str:
    stable = {
        key: value
        for key, value in outcome.items()
        if key not in {"elapsed_ms", "stderr", "returncode"}
    }
    return sha256_bytes(canonical_bytes(stable))


def disagreement_id(disagreement: dict[str, Any]) -> str:
    identity = {
        key: disagreement.get(key)
        for key in (
            "case_id",
            "oracle",
            "kind",
            "subject_sha256",
            "oracle_sha256",
        )
    }
    return "oracle-disagreement-v1:" + sha256_bytes(canonical_bytes(identity))


def build_adjudications(
    disagreements: list[dict[str, Any]],
    validation_result: Path,
    external_oracles_available: int,
) -> dict[str, Any]:
    previous: dict[str, dict[str, Any]] = {}
    if ADJUDICATION_PATH.is_file():
        existing = json.loads(ADJUDICATION_PATH.read_text())
        previous = {
            record["id"]: record
            for record in existing.get("records", [])
            if isinstance(record, dict) and isinstance(record.get("id"), str)
        }
    records = []
    for disagreement in disagreements:
        record_id = disagreement_id(disagreement)
        retained = previous.get(record_id, {})
        records.append(
            {
                "id": record_id,
                **{
                    key: disagreement.get(key)
                    for key in (
                        "case_id",
                        "oracle",
                        "kind",
                        "subject_sha256",
                        "oracle_sha256",
                    )
                },
                "status": retained.get("status", "unadjudicated"),
                "decision": retained.get("decision"),
                "rationale": retained.get("rationale"),
                "adjudicator": retained.get("adjudicator"),
                "adjudicated_at": retained.get("adjudicated_at"),
            }
        )
    unresolved = sum(record["status"] == "unadjudicated" for record in records)
    status = (
        "skipped"
        if external_oracles_available == 0
        else "passed"
        if unresolved == 0
        else "blocked"
    )
    return {
        "$schema": "../oracle-adjudication.schema.json",
        "schema_version": 1,
        "status": status,
        "validation_result_sha256": sha256_file(validation_result),
        "summary": {
            "external_oracles_available": external_oracles_available,
            "total": len(records),
            "unadjudicated": unresolved,
            "adjudicated": len(records) - unresolved,
        },
        "records": records,
    }


def resolve_subject() -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    return ROOT / "target" / "debug" / "examples" / f"validation_decoder{suffix}"


def build_subject() -> Path:
    subprocess.run(
        ["cargo", "build", "--example", "validation_decoder", "--locked"],
        cwd=ROOT,
        check=True,
    )
    executable = resolve_subject()
    if not executable.is_file():
        raise FileNotFoundError(f"subject harness not built: {executable}")
    return executable


def _official_source_url() -> str:
    profile = json.loads((ROOT / "standards-profile.json").read_text())
    for edition in profile["normative_editions"]:
        if edition.get("part") == 4:
            return edition["test_asset_url"]
    raise ValueError("selected profile has no Part 4 test asset URL")


def _official_limits(
    max_case_ms: float,
    max_total_ms: float,
    max_input_bytes: int,
    max_output_bytes: int,
    max_address_space_bytes: int,
) -> dict[str, Any]:
    return {
        "max_case_ms": max_case_ms,
        "max_total_ms": max_total_ms,
        "max_input_bytes": max_input_bytes,
        "max_output_bytes": max_output_bytes,
        "max_address_space_bytes": max_address_space_bytes,
        "memory_enforcement": (
            "POSIX RLIMIT_AS"
            if resource is not None and hasattr(resource, "RLIMIT_AS")
            else "unavailable on this platform; decoder ResourceLimits remain active"
        ),
    }


def _official_blocked_result(
    access_status: str,
    blocker: str,
    final_url: str | None,
    http_status: int | None,
    limits: dict[str, Any],
) -> dict[str, Any]:
    source_revision = build_source_tree_manifest()["source_revision"]
    result = {
        "$schema": "../official-vector-result.schema.json",
        "schema_version": 1,
        "profile_id": PROFILE_ID,
        "status": "blocked",
        "source_url": _official_source_url(),
        "acquisition": {
            "attempted_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            "access_status": access_status,
            "final_url": final_url,
            "http_status": http_status,
            "archive_sha256": None,
            "archive_bytes": None,
        },
        "manifest_sha256": None,
        "immutable_source_revision": source_revision,
        "tool_revision": (
            f"python {sys.version_info.major}.{sys.version_info.minor}."
            f"{sys.version_info.micro}"
        ),
        "subject": {
            "binary_sha256": None,
            "source_revision": source_revision,
        },
        "selection": {"complete": False, "applicable": 0, "inapplicable": 0},
        "summary": {
            "verified": 0,
            "executed": 0,
            "passed": 0,
            "failed": 0,
            "blocked": 0,
            "total_elapsed_ms": 0.0,
        },
        "limits": limits,
        "blockers": [blocker],
        "records": [],
    }
    write_json(OFFICIAL_RESULT_PATH, result)
    return result


def _external_path(root: Path, relative: str) -> Path:
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError("official vector manifest path escapes external root") from error
    return candidate


def _image_metrics(outcome: dict[str, Any]) -> dict[str, Any] | None:
    image = outcome.get("image")
    if not isinstance(image, dict):
        return None
    return {
        "width": image["width"],
        "height": image["height"],
        "components": [
            {
                key: component[key]
                for key in ("width", "height", "precision", "signed", "sample_kind")
            }
            for component in image["components"]
        ],
    }


def _run_process_bounded(
    command: list[str],
    timeout_seconds: float,
    max_output_bytes: int,
    preexec_fn: Any,
) -> tuple[subprocess.CompletedProcess[bytes], float, bool]:
    start = time.monotonic()
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        preexec_fn=preexec_fn,
    )
    streams = {"stdout": process.stdout, "stderr": process.stderr}
    output = {"stdout": bytearray(), "stderr": bytearray()}
    selector = selectors.DefaultSelector()
    for name, stream in streams.items():
        if stream is not None:
            selector.register(stream, selectors.EVENT_READ, name)
    output_limited = False
    try:
        while selector.get_map():
            remaining = timeout_seconds - (time.monotonic() - start)
            if remaining <= 0:
                process.kill()
                process.wait()
                raise subprocess.TimeoutExpired(command, timeout_seconds)
            for key, _ in selector.select(min(remaining, 0.1)):
                chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                current_bytes = len(output["stdout"]) + len(output["stderr"])
                if current_bytes + len(chunk) > max_output_bytes:
                    output_limited = True
                    process.kill()
                    for stream in streams.values():
                        if stream is not None:
                            stream.close()
                    process.wait()
                    break
                output[key.data].extend(chunk)
            if output_limited:
                break
        if not output_limited:
            remaining = timeout_seconds - (time.monotonic() - start)
            try:
                process.wait(timeout=max(remaining, 0.001))
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise subprocess.TimeoutExpired(command, timeout_seconds)
    finally:
        selector.close()
        for stream in streams.values():
            if stream is not None and not stream.closed:
                stream.close()
    completed = subprocess.CompletedProcess(
        command,
        process.returncode,
        bytes(output["stdout"]),
        bytes(output["stderr"]),
    )
    return completed, (time.monotonic() - start) * 1000, output_limited


def run_official_vectors(
    executable: Path | None = None,
    vector_root: Path | None = None,
    manifest_path: Path | None = None,
    *,
    access_status: str = "not-configured",
    blocked_reason: str = "official vector root is not configured",
    final_url: str | None = None,
    http_status: int | None = None,
    max_case_ms: float = 10000.0,
    max_total_ms: float = 300000.0,
    max_input_bytes: int = 64 * 1024 * 1024,
    max_output_bytes: int = 64 * 1024 * 1024,
    max_address_space_bytes: int = 512 * 1024 * 1024,
) -> dict[str, Any]:
    """Verify and execute externally held official vectors without copying them."""
    limits = _official_limits(
        max_case_ms,
        max_total_ms,
        max_input_bytes,
        max_output_bytes,
        max_address_space_bytes,
    )
    configured_root = vector_root or (
        Path(os.environ[OFFICIAL_VECTOR_ROOT_ENV])
        if os.environ.get(OFFICIAL_VECTOR_ROOT_ENV)
        else None
    )
    if configured_root is None:
        return _official_blocked_result(
            access_status, blocked_reason, final_url, http_status, limits
        )
    root = configured_root.resolve()
    if not root.is_dir():
        return _official_blocked_result(
            "unavailable",
            "configured official vector root is not an accessible directory",
            final_url,
            http_status,
            limits,
        )
    configured_manifest = manifest_path or (
        Path(os.environ[OFFICIAL_VECTOR_MANIFEST_ENV])
        if os.environ.get(OFFICIAL_VECTOR_MANIFEST_ENV)
        else root / "official-vector-manifest.json"
    )
    if not configured_manifest.is_file():
        return _official_blocked_result(
            "unavailable",
            "external official vector manifest is unavailable",
            final_url,
            http_status,
            limits,
        )
    manifest_bytes = configured_manifest.read_bytes()
    try:
        manifest = json.loads(manifest_bytes)
        schema = json.loads(
            (VALIDATION / "official-vector-manifest.schema.json").read_text()
        )
        Draft202012Validator(schema).validate(manifest)
    except (json.JSONDecodeError, ValueError) as error:
        raise ValueError(f"invalid external official vector manifest: {error}") from error
    except Exception as error:
        raise ValueError(f"invalid external official vector manifest: {error}") from error

    suite = manifest["suite"]
    if suite["source_url"] != _official_source_url():
        raise ValueError("official vector source URL does not match selected profile")
    entries = manifest["entries"]
    applicable = [entry for entry in entries if entry["applicable"]]
    if suite["selection"]["applicable_vector_count"] != len(applicable):
        raise ValueError("applicable official vector count does not match selection")
    ids = [entry["id"] for entry in entries]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate official vector identifier")

    archive = suite["archive"]
    archive_path = _external_path(root, archive["path"])
    archive_valid = (
        archive_path.is_file()
        and archive_path.stat().st_size == archive["bytes"]
        and sha256_file(archive_path) == archive["sha256"]
    )
    blockers: list[str] = []
    if not archive_valid:
        blockers.append("official vector archive hash or byte length does not match manifest")

    if executable is None:
        executable = build_subject()
    source_revision = build_source_tree_manifest()["source_revision"]
    records: list[dict[str, Any]] = []
    verified = 0
    executed = 0
    total_elapsed_ms = 0.0
    for entry in applicable:
        vector_path = _external_path(root, entry["path"])
        vector_key = "official-vector-v1:" + sha256_bytes(
            canonical_bytes({"id": entry["id"], "sha256": entry["sha256"]})
        )
        record: dict[str, Any] = {
            "vector_key": vector_key,
            "file_sha256": entry["sha256"],
            "bytes": entry["bytes"],
            "format": entry["format"],
            "status": "blocked",
            "accepted": None,
            "category": None,
            "normalized_output_sha256": None,
            "metrics": None,
            "elapsed_ms": 0.0,
            "failure": None,
        }
        if entry["identifier_public"]:
            record["public_id"] = entry["id"]
        if not vector_path.is_file():
            record["failure"] = "vector file unavailable"
            records.append(record)
            continue
        actual_bytes = vector_path.stat().st_size
        actual_sha256 = sha256_file(vector_path)
        if actual_bytes != entry["bytes"] or actual_sha256 != entry["sha256"]:
            record["status"] = "failed"
            record["failure"] = "vector file hash or byte length mismatch"
            records.append(record)
            continue
        verified += 1
        if actual_bytes > max_input_bytes:
            record["status"] = "failed"
            record["failure"] = "input resource limit exceeded"
            records.append(record)
            continue
        if total_elapsed_ms >= max_total_ms:
            record["failure"] = "total execution time limit exhausted"
            records.append(record)
            continue

        preexec_fn = None
        if resource is not None and hasattr(resource, "RLIMIT_AS"):
            def limit_address_space() -> None:
                resource.setrlimit(
                    resource.RLIMIT_AS,
                    (max_address_space_bytes, max_address_space_bytes),
                )

            preexec_fn = limit_address_space
        command = [
            str(executable),
            entry["operation"],
            entry["format"],
            str(vector_path),
        ]
        start = time.monotonic()
        try:
            completed, elapsed_ms, output_limited = _run_process_bounded(
                command,
                max_case_ms / 1000,
                max_output_bytes,
                preexec_fn,
            )
            executed += 1
            if output_limited:
                outcome = {"accepted": False, "category": "output-limit"}
            else:
                outcome = parse_subject_output(
                    completed.stdout, completed.stderr, completed.returncode
                )
        except subprocess.TimeoutExpired:
            elapsed_ms = (time.monotonic() - start) * 1000
            executed += 1
            outcome = {"accepted": False, "category": "timeout"}
        except (OSError, ValueError) as error:
            elapsed_ms = (time.monotonic() - start) * 1000
            executed += 1
            outcome = {"accepted": False, "category": "harness-error"}
            record["failure"] = type(error).__name__
        elapsed_ms = round(elapsed_ms, 3)
        total_elapsed_ms += elapsed_ms
        output_sha256 = normalized_hash(outcome)
        expected = entry["expected"]
        failure = record["failure"]
        if outcome["accepted"] != expected["accepted"]:
            failure = "acceptance mismatch"
        elif outcome["accepted"] and output_sha256 != expected["normalized_sha256"]:
            failure = "normalized output hash mismatch"
        elif (
            not outcome["accepted"]
            and expected.get("category") is not None
            and outcome.get("category") != expected["category"]
        ):
            failure = "rejection category mismatch"
        record.update(
            {
                "status": "failed" if failure else "passed",
                "accepted": outcome["accepted"],
                "category": outcome.get("category"),
                "normalized_output_sha256": output_sha256,
                "metrics": _image_metrics(outcome),
                "elapsed_ms": elapsed_ms,
                "failure": failure,
            }
        )
        records.append(record)

    failed = sum(record["status"] == "failed" for record in records)
    blocked = sum(record["status"] == "blocked" for record in records)
    passed = sum(record["status"] == "passed" for record in records)
    if not archive_valid:
        status = "failed"
    elif blocked:
        status = "blocked"
    elif failed:
        status = "failed"
    elif not applicable or executed != len(applicable) or passed != len(applicable):
        status = "blocked"
    else:
        status = "passed"
    if status != "passed" and not blockers:
        blockers.append("not all selected-profile applicable official vectors passed")
    result = {
        "$schema": "../official-vector-result.schema.json",
        "schema_version": 1,
        "profile_id": PROFILE_ID,
        "status": status,
        "source_url": suite["source_url"],
        "acquisition": {
            "attempted_at": suite["acquired_at"],
            "access_status": "acquired",
            "final_url": suite["source_url"],
            "http_status": 200,
            "archive_sha256": archive["sha256"],
            "archive_bytes": archive["bytes"],
        },
        "manifest_sha256": sha256_bytes(manifest_bytes),
        "immutable_source_revision": source_revision,
        "tool_revision": (
            f"python {sys.version_info.major}.{sys.version_info.minor}."
            f"{sys.version_info.micro}; rust subject {sha256_file(executable)}"
        ),
        "subject": {
            "binary_sha256": sha256_file(executable),
            "source_revision": source_revision,
        },
        "selection": {
            "complete": suite["selection"]["complete"],
            "applicable": len(applicable),
            "inapplicable": len(entries) - len(applicable),
        },
        "summary": {
            "verified": verified,
            "executed": executed,
            "passed": passed,
            "failed": failed,
            "blocked": blocked,
            "total_elapsed_ms": round(total_elapsed_ms, 3),
        },
        "limits": limits,
        "blockers": blockers,
        "records": records,
    }
    write_json(OFFICIAL_RESULT_PATH, result)
    return result


def load_oracles(path: Path | None = None) -> list[dict[str, Any]]:
    config_path = path or VALIDATION / "oracles.json"
    return json.loads(config_path.read_text())["oracles"]


def _run_process(command: list[str], timeout_seconds: float) -> tuple[subprocess.CompletedProcess[bytes], float]:
    start = time.monotonic()
    completed = subprocess.run(
        command,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
        check=False,
    )
    return completed, (time.monotonic() - start) * 1000


def run_validation(
    executable: Path,
    include_external: bool = True,
    timeout_seconds: float = 5.0,
    oracle_config: Path | None = None,
) -> dict[str, Any]:
    manifest = load_manifest()
    oracle_specs = load_oracles(oracle_config) if include_external else []
    oracle_status: list[dict[str, Any]] = [
        {
            "name": "jpeg2000_rs",
            "status": "available",
            "executable": "cargo-example:validation_decoder",
            "binary_sha256": sha256_file(executable),
        }
    ]
    available_oracles: list[tuple[dict[str, Any], str]] = []
    for spec in oracle_specs:
        configured = os.environ.get(spec["environment"])
        found = configured or shutil.which(spec["executable"])
        if found:
            available_oracles.append((spec, found))
            oracle_status.append(
                {
                    "name": spec["name"],
                    "status": "available",
                    "executable": Path(found).name,
                    "binary_sha256": sha256_file(Path(found).resolve()),
                }
            )
        else:
            oracle_status.append(
                {
                    "name": spec["name"],
                    "status": "skipped",
                    "reason": f"{spec['environment']} unset and {spec['executable']} not found",
                }
            )
    records: list[dict[str, Any]] = []
    by_case: dict[str, dict[str, Any]] = {}
    for entry in manifest["entries"]:
        input_path = VALIDATION / entry["path"]
        command = [str(executable), entry["operation"], entry["format"], str(input_path)]
        try:
            process, elapsed = _run_process(command, timeout_seconds)
            outcome = parse_subject_output(process.stdout, process.stderr, process.returncode)
        except subprocess.TimeoutExpired:
            elapsed = timeout_seconds * 1000
            outcome = {"accepted": False, "category": "timeout"}
        outcome["elapsed_ms"] = round(elapsed, 3)
        outcome["normalized_sha256"] = normalized_hash(outcome)
        record: dict[str, Any] = {
            "case_id": entry["id"],
            "expected": entry["expected"],
            "kind": entry["kind"],
            "format": entry["format"],
            "outcomes": {"jpeg2000_rs": outcome},
        }
        by_case[entry["id"]] = record
        for spec, oracle_executable in available_oracles:
            if entry["format"] not in spec["formats"] or entry["operation"] != "decode":
                record["outcomes"][spec["name"]] = {
                    "status": "skipped",
                    "reason": "unsupported format or operation",
                }
                continue
            output_base = RESULTS_DIR / "oracle-output" / f"{spec['name']}-{entry['id']}"
            output_base.parent.mkdir(parents=True, exist_ok=True)
            output_path = output_base.with_suffix(".pnm")
            command = [
                value.format(
                    executable=oracle_executable,
                    input=str(input_path),
                    output=str(output_path),
                    format=entry["format"],
                )
                for value in spec["command"]
            ]
            try:
                process, oracle_elapsed = _run_process(command, timeout_seconds)
                if process.returncode == 0 and output_path.is_file():
                    oracle_outcome = parse_pnm(output_path)
                else:
                    oracle_outcome = {
                        "accepted": False,
                        "category": "decoder-rejected",
                        "returncode": process.returncode,
                    }
                oracle_outcome["elapsed_ms"] = round(oracle_elapsed, 3)
                oracle_outcome["normalized_sha256"] = normalized_hash(oracle_outcome)
            except (subprocess.TimeoutExpired, ValueError) as error:
                oracle_outcome = {
                    "accepted": False,
                    "category": "timeout" if isinstance(error, subprocess.TimeoutExpired) else "output-error",
                }
            finally:
                if output_path.exists():
                    output_path.unlink()
            record["outcomes"][spec["name"]] = oracle_outcome
        records.append(record)
    disagreements: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for entry, record in zip(manifest["entries"], records):
        subject = record["outcomes"]["jpeg2000_rs"]
        expected_accept = entry["expected"] == "accept"
        if subject["accepted"] != expected_accept:
            failures.append(
                {
                    "case_id": entry["id"],
                    "expected": entry["expected"],
                    "actual": "accept" if subject["accepted"] else "reject",
                    "category": subject.get("category"),
                }
            )
        relation = entry.get("metamorphic")
        if relation and subject["accepted"]:
            parent = by_case[relation["parent"]]["outcomes"]["jpeg2000_rs"]
            if subject.get("normalized_sha256") != parent.get("normalized_sha256"):
                failures.append(
                    {
                        "case_id": entry["id"],
                        "expected": relation["relation"],
                        "actual": "normalized-output-differs",
                    }
                )
        for oracle_name, oracle_outcome in record["outcomes"].items():
            if oracle_name == "jpeg2000_rs" or "accepted" not in oracle_outcome:
                continue
            acceptance_differs = oracle_outcome["accepted"] != subject["accepted"]
            output_differs = (
                oracle_outcome["accepted"]
                and subject["accepted"]
                and oracle_outcome.get("normalized_sha256") != subject.get("normalized_sha256")
            )
            if acceptance_differs or output_differs:
                disagreement = {
                        "case_id": entry["id"],
                        "oracle": oracle_name,
                        "kind": "acceptance" if acceptance_differs else "normalized-output",
                        "subject_sha256": subject.get("normalized_sha256"),
                        "oracle_sha256": oracle_outcome.get("normalized_sha256"),
                        "status": "unadjudicated",
                    }
                disagreement["id"] = disagreement_id(disagreement)
                disagreements.append(disagreement)
    result = {
        "$schema": "../validation-result.schema.json",
        "schema_version": 1,
        "profile_id": PROFILE_ID,
        "corpus_manifest_sha256": sha256_file(MANIFEST_PATH),
        "oracles": oracle_status,
        "summary": {
            "cases": len(records),
            "passed": len(records) - len(failures),
            "failed": len(failures),
            "disagreements": len(disagreements),
            "external_oracles_available": len(available_oracles),
            "external_oracles_skipped": sum(
                status["status"] == "skipped" for status in oracle_status
            ),
        },
        "failures": failures,
        "disagreements": disagreements,
        "records": records,
    }
    validation_path = RESULTS_DIR / "validation-results.json"
    write_json(validation_path, result)
    write_json(
        RESULTS_DIR / "disagreements.json",
        {
            "schema_version": 1,
            "status": "blocked" if disagreements else "passed",
            "disagreements": disagreements,
        },
    )
    adjudications = build_adjudications(
        disagreements,
        validation_path,
        len(available_oracles),
    )
    write_json(ADJUDICATION_PATH, adjudications)
    return result


def run_budget(
    executable: Path,
    max_case_ms: float = 5000.0,
    max_total_ms: float = 30000.0,
    max_output_bytes: int = 1024 * 1024,
    max_input_bytes: int = 1024 * 1024,
    max_address_space_bytes: int = 512 * 1024 * 1024,
) -> dict[str, Any]:
    manifest = load_manifest()
    cases: list[dict[str, Any]] = []
    total_ms = 0.0
    failures: list[str] = []
    for entry in manifest["entries"]:
        input_path = VALIDATION / entry["path"]
        if entry["bytes"] > max_input_bytes:
            failures.append(f"{entry['id']}: input budget")
        command = [str(executable), entry["operation"], entry["format"], str(input_path)]
        start = time.monotonic()
        preexec_fn = None
        if resource is not None and hasattr(resource, "RLIMIT_AS"):
            def limit_address_space() -> None:
                resource.setrlimit(
                    resource.RLIMIT_AS,
                    (max_address_space_bytes, max_address_space_bytes),
                )

            preexec_fn = limit_address_space
        try:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=max_case_ms / 1000,
                check=False,
                preexec_fn=preexec_fn,
            )
            elapsed = (time.monotonic() - start) * 1000
            output_bytes = len(completed.stdout) + len(completed.stderr)
            status = "passed"
            if output_bytes > max_output_bytes:
                status = "failed"
                failures.append(f"{entry['id']}: output budget")
        except subprocess.TimeoutExpired:
            elapsed = (time.monotonic() - start) * 1000
            output_bytes = 0
            status = "failed"
            failures.append(f"{entry['id']}: time budget")
        total_ms += elapsed
        cases.append(
            {
                "case_id": entry["id"],
                "elapsed_ms": round(elapsed, 3),
                "output_bytes": output_bytes,
                "status": status,
            }
        )
    if total_ms > max_total_ms:
        failures.append("total time budget")
    result = {
        "$schema": "../budget-result.schema.json",
        "schema_version": 1,
        "profile_id": PROFILE_ID,
        "budgets": {
            "max_case_ms": max_case_ms,
            "max_total_ms": max_total_ms,
            "max_output_bytes": max_output_bytes,
            "max_input_bytes": max_input_bytes,
            "max_address_space_bytes": max_address_space_bytes,
        },
        "summary": {
            "status": "passed" if not failures else "failed",
            "cases": len(cases),
            "total_ms": round(total_ms, 3),
            "failures": failures,
            "memory_enforcement": (
                "POSIX RLIMIT_AS"
                if resource is not None and hasattr(resource, "RLIMIT_AS")
                else "unavailable on this platform; decoder ResourceLimits remain active"
            ),
        },
        "cases": cases,
    }
    write_json(RESULTS_DIR / "budget-results.json", result)
    return result


def build_release_attestation() -> dict[str, Any]:
    source_tree = write_source_tree_manifest()
    architecture = write_architecture_review()
    validation_result = RESULTS_DIR / "validation-results.json"
    budget_result = RESULTS_DIR / "budget-results.json"
    required = [
        SOURCE_TREE_PATH,
        ROOT / "source-provenance.json",
        ROOT / "rust-toolchain.toml",
        VALIDATION / "requirements.txt",
        ROOT / "proof-attestation.json",
        ROOT / "production-proof-coverage.json",
        ROOT / "normative-coverage.json",
        ROOT / "lifetime-analysis-blocker.json",
        ROOT / "conformance-attestation.json",
        ROOT / "fuzz-attestation.json",
        ROOT / "standards-profile.json",
        ROOT / "support-matrix.json",
        ROOT / "unsupported-boundary-audit.json",
        MANIFEST_PATH,
        validation_result,
        budget_result,
        ADJUDICATION_PATH,
        OFFICIAL_RESULT_PATH,
        ROOT / "architecture-review.json",
        ROOT / "legal-publication-review.json",
        ROOT / "owner-approval.json",
        VALIDATION / "expected-package-files.txt",
    ]
    missing = [path.relative_to(ROOT).as_posix() for path in required if not path.is_file()]
    if missing:
        raise FileNotFoundError("missing attestation inputs: " + ", ".join(missing))
    validation = json.loads(validation_result.read_text())
    budget = json.loads(budget_result.read_text())
    adjudication = json.loads(ADJUDICATION_PATH.read_text())
    official_result = json.loads(OFFICIAL_RESULT_PATH.read_text())
    legal = json.loads((ROOT / "legal-publication-review.json").read_text())
    owner = json.loads((ROOT / "owner-approval.json").read_text())
    proof = json.loads((ROOT / "proof-attestation.json").read_text())
    proof_coverage_path = ROOT / "production-proof-coverage.json"
    proof_coverage = json.loads(proof_coverage_path.read_text())
    proof_coverage_matches = (
        proof.get("input_hashes", {}).get("production-proof-coverage.json")
        == sha256_file(proof_coverage_path)
    )
    proof_sources_match = all(
        (ROOT / relative).is_file()
        and sha256_file(ROOT / relative) == expected
        for relative, expected in proof_coverage.get("source_hashes", {}).items()
    )
    proof_results = proof.get("result", {})
    proof_locally_complete = (
        proof_coverage_matches
        and proof_sources_match
        and proof_results.get("errors") == 0
        and proof_results.get("incomplete_functions") == 0
        and proof_results.get("missing_dispositions") == 0
        and proof_results.get("duplicated_dispositions") == 0
        and proof_results.get("trusted_codec_functions") == 0
    )
    lifetime = json.loads((ROOT / "lifetime-analysis-blocker.json").read_text())
    proof_boundary_status = (
        "blocked"
        if proof_locally_complete and lifetime.get("status") == "blocked"
        else "passed"
        if proof_locally_complete
        else "failed"
    )
    bindings = {
        path.relative_to(ROOT).as_posix(): sha256_file(path)
        for path in required
    }
    framework_complete = (
        validation["summary"]["failed"] == 0
        and budget["summary"]["status"] == "passed"
    )
    official_vectors = official_result["selection"]["applicable"]
    official_vectors_passed = (
        official_result["status"] == "passed"
        and official_result["selection"]["complete"]
        and official_vectors > 0
        and official_result["summary"]["executed"] == official_vectors
        and official_result["summary"]["passed"] == official_vectors
        and official_result["summary"]["failed"] == 0
        and official_result["summary"]["blocked"] == 0
    )
    architecture_status = (
        "passed"
        if architecture.get("status") == "passed"
        and architecture.get("source_revision") == source_tree["source_revision"]
        else "blocked"
    )
    legal_status = (
        "passed"
        if legal.get("status") == "approved"
        and legal.get("candidate_revision") == source_tree["source_revision"]
        else "blocked"
    )
    owner_status = (
        "passed"
        if owner.get("status") == "approved"
        and owner.get("candidate_revision") == source_tree["source_revision"]
        else "blocked"
    )
    adjudication_status = (
        "passed"
        if adjudication["status"] == "passed"
        and adjudication["summary"]["unadjudicated"] == 0
        else "blocked"
    )
    gates = {
        "official_vectors": {
            "status": (
                "passed"
                if official_vectors_passed
                else "failed"
                if official_result["status"] == "failed"
                else "blocked"
            ),
            "evidence": ["validation/results/official-vector-results.json"],
            "vector_count": official_vectors,
            "selection_complete": official_result["selection"]["complete"],
            "executed": official_result["summary"]["executed"],
            "passed": official_result["summary"]["passed"],
            "failed": official_result["summary"]["failed"],
            "manifest_sha256": official_result["manifest_sha256"],
            "archive_sha256": official_result["acquisition"]["archive_sha256"],
            "result_sha256": sha256_file(OFFICIAL_RESULT_PATH),
        },
        "oracle_adjudication": {
            "status": adjudication_status,
            "evidence": [
                "validation/results/validation-results.json",
                "validation/results/oracle-adjudications.json",
            ],
            "unresolved": adjudication["summary"]["unadjudicated"],
            "external_oracles_available": adjudication["summary"][
                "external_oracles_available"
            ],
        },
        "architecture": {
            "status": architecture_status,
            "evidence": ["architecture-review.json"],
        },
        "proof_boundary": {
            "status": proof_boundary_status,
            "evidence": [
                "proof-attestation.json",
                "production-proof-coverage.json",
                "lifetime-analysis-blocker.json",
            ],
        },
        "legal_publication": {
            "status": legal_status,
            "evidence": ["legal-publication-review.json"],
        },
        "owner_approval": {
            "status": owner_status,
            "evidence": ["owner-approval.json"],
        },
    }
    blocking_gates = [
        name for name, gate in gates.items() if gate["status"] != "passed"
    ]
    publish_allowed = framework_complete and not blocking_gates
    attestation = {
        "$schema": "./release-attestation.schema.json",
        "schema_version": 2,
        "attestation_type": "release",
        "status": "passed" if publish_allowed else "blocked",
        "immutable_source_revision": source_tree["source_revision"],
        "profile_id": PROFILE_ID,
        "tool_revision": f"python {sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
        "input_hashes": bindings,
        "bindings": {
            "source": ["source-tree.json", "source-provenance.json"],
            "toolchains": [
                "rust-toolchain.toml",
                "validation/requirements.txt",
                "source-provenance.json",
            ],
            "proof": [
                "proof-attestation.json",
                "production-proof-coverage.json",
                "normative-coverage.json",
                "lifetime-analysis-blocker.json",
            ],
            "corpus": ["validation/corpus.json"],
            "validation": [
                "conformance-attestation.json",
                "validation/results/validation-results.json",
                "validation/results/official-vector-results.json",
            ],
            "budgets": ["validation/results/budget-results.json"],
            "fuzz": ["fuzz-attestation.json"],
            "profile": [
                "standards-profile.json",
                "support-matrix.json",
                "unsupported-boundary-audit.json",
            ],
            "package": ["validation/expected-package-files.txt"],
            "adjudication": ["validation/results/oracle-adjudications.json"],
            "architecture": ["architecture-review.json"],
            "approvals": [
                "legal-publication-review.json",
                "owner-approval.json",
            ],
            "results": [
                "validation/results/validation-results.json",
                "validation/results/budget-results.json",
                "validation/results/official-vector-results.json",
            ],
        },
        "gates": gates,
        "result": {
            "publish_allowed": publish_allowed,
            "validation_framework_complete": framework_complete,
            "official_conformance_claimed": publish_allowed and official_vectors_passed,
            "official_vectors": official_vectors,
            "official_vector_evidence": {
                "status": official_result["status"],
                "manifest_sha256": official_result["manifest_sha256"],
                "archive_sha256": official_result["acquisition"]["archive_sha256"],
                "result_sha256": sha256_file(OFFICIAL_RESULT_PATH),
                "source_revision": official_result["immutable_source_revision"],
                "subject_binary_sha256": official_result["subject"]["binary_sha256"],
            },
            "external_oracles": validation["oracles"],
            "proof_reference": {
                "status": (
                    "verified"
                    if proof_locally_complete
                    else "failed"
                ),
                "production_source_revision": source_tree[
                    "production_source_revision"
                ],
                "attestation": "proof-attestation.json",
                "coverage_hash_matches_attestation": proof_coverage_matches,
                "all_proof_input_hashes_match": proof_sources_match,
            },
            "blocking_gates": blocking_gates,
        },
        "commands": [
            "python3 validation/run-validation.py generate --check",
            "python3 validation/run-validation.py verify",
            "python3 validation/run-validation.py run",
            "python3 validation/run-validation.py budget",
            "python3 validation/run-validation.py official",
            "python3 validation/run-validation.py source --check",
            "python3 validation/run-validation.py architecture --check",
            "python3 validation/run-validation.py attest",
            "python3 validation/run-validation.py schemas",
        ],
        "created_at": None,
    }
    write_json(ROOT / "release-attestation.json", attestation)
    write_json(
        ROOT / "release-blockers.json",
        {
            "schema_version": 1,
            "phase": "phase-i-release-conformance",
            "status": "blocked" if blocking_gates else "passed",
            "immutable_source_revision": source_tree["source_revision"],
            "release_attestation_sha256": sha256_file(
                ROOT / "release-attestation.json"
            ),
            "blockers": [
                {
                    "gate": name,
                    "status": gates[name]["status"],
                    "evidence": gates[name]["evidence"],
                }
                for name in blocking_gates
            ],
        },
    )
    return attestation
