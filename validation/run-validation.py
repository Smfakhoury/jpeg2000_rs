#!/usr/bin/env python3
"""CI-friendly entry point for the decoder-validation framework."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from framework import (
    build_release_attestation,
    build_subject,
    generate_corpus,
    resolve_subject,
    run_budget,
    run_official_vectors,
    run_validation,
    validate_evidence_schemas,
    write_architecture_review,
    verify_manifest,
    write_source_tree_manifest,
)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    commands = result.add_subparsers(dest="command", required=True)
    generate = commands.add_parser("generate")
    generate.add_argument("--check", action="store_true")
    commands.add_parser("verify")
    run = commands.add_parser("run")
    run.add_argument("--subject", type=Path)
    run.add_argument("--no-build", action="store_true")
    run.add_argument("--no-external", action="store_true")
    run.add_argument("--oracle-config", type=Path)
    run.add_argument("--timeout-seconds", type=float, default=5.0)
    budget = commands.add_parser("budget")
    budget.add_argument("--subject", type=Path)
    budget.add_argument("--no-build", action="store_true")
    budget.add_argument("--max-case-ms", type=float, default=5000.0)
    budget.add_argument("--max-total-ms", type=float, default=30000.0)
    official = commands.add_parser("official")
    official.add_argument("--subject", type=Path)
    official.add_argument("--no-build", action="store_true")
    official.add_argument("--vector-root", type=Path)
    official.add_argument("--manifest", type=Path)
    official.add_argument(
        "--access-status",
        choices=[
            "authentication-required",
            "license-blocked",
            "not-configured",
            "unavailable",
        ],
        default="not-configured",
    )
    official.add_argument(
        "--blocked-reason",
        default="official vector root is not configured",
    )
    official.add_argument("--final-url")
    official.add_argument("--http-status", type=int)
    official.add_argument("--max-case-ms", type=float, default=10000.0)
    official.add_argument("--max-total-ms", type=float, default=300000.0)
    official.add_argument(
        "--max-input-bytes", type=int, default=64 * 1024 * 1024
    )
    official.add_argument(
        "--max-output-bytes", type=int, default=64 * 1024 * 1024
    )
    official.add_argument(
        "--max-address-space-bytes", type=int, default=512 * 1024 * 1024
    )
    commands.add_parser("attest")
    source = commands.add_parser("source")
    source.add_argument("--check", action="store_true")
    architecture = commands.add_parser("architecture")
    architecture.add_argument("--check", action="store_true")
    commands.add_parser("schemas")
    all_command = commands.add_parser("all")
    all_command.add_argument("--no-external", action="store_true")
    return result


def subject_for(arguments: argparse.Namespace) -> Path:
    if getattr(arguments, "subject", None):
        return arguments.subject.resolve()
    if getattr(arguments, "no_build", False):
        return resolve_subject()
    return build_subject()


def emit(value: object) -> None:
    print(json.dumps(value, indent=2, sort_keys=True))


def main() -> int:
    arguments = parser().parse_args()
    try:
        if arguments.command == "generate":
            manifest = generate_corpus(arguments.check)
            emit({"status": "passed", "entries": len(manifest["entries"])})
        elif arguments.command == "verify":
            emit(verify_manifest())
        elif arguments.command == "run":
            result = run_validation(
                subject_for(arguments),
                include_external=not arguments.no_external,
                timeout_seconds=arguments.timeout_seconds,
                oracle_config=arguments.oracle_config,
            )
            emit(result["summary"])
            return 1 if result["summary"]["failed"] else 0
        elif arguments.command == "budget":
            result = run_budget(
                subject_for(arguments),
                arguments.max_case_ms,
                arguments.max_total_ms,
            )
            emit(result["summary"])
            return 1 if result["summary"]["status"] != "passed" else 0
        elif arguments.command == "official":
            executable = None
            if arguments.subject is not None or arguments.no_build:
                executable = subject_for(arguments)
            result = run_official_vectors(
                executable=executable,
                vector_root=arguments.vector_root,
                manifest_path=arguments.manifest,
                access_status=arguments.access_status,
                blocked_reason=arguments.blocked_reason,
                final_url=arguments.final_url,
                http_status=arguments.http_status,
                max_case_ms=arguments.max_case_ms,
                max_total_ms=arguments.max_total_ms,
                max_input_bytes=arguments.max_input_bytes,
                max_output_bytes=arguments.max_output_bytes,
                max_address_space_bytes=arguments.max_address_space_bytes,
            )
            emit(
                {
                    "status": result["status"],
                    "selection": result["selection"],
                    "summary": result["summary"],
                    "blockers": result["blockers"],
                }
            )
            return 0 if result["status"] == "passed" else 1
        elif arguments.command == "attest":
            emit(build_release_attestation()["result"])
        elif arguments.command == "source":
            manifest = write_source_tree_manifest(arguments.check)
            emit(
                {
                    "status": "passed",
                    "source_revision": manifest["source_revision"],
                    "production_source_revision": manifest[
                        "production_source_revision"
                    ],
                    "files": len(manifest["files"]),
                }
            )
        elif arguments.command == "architecture":
            review = write_architecture_review(arguments.check)
            emit(
                {
                    "status": review["status"],
                    "source_revision": review["source_revision"],
                    "checklist_items": len(review["checklist"]),
                }
            )
        elif arguments.command == "schemas":
            emit(validate_evidence_schemas())
        else:
            generate_corpus(check=True)
            emit(verify_manifest())
            executable = build_subject()
            validation = run_validation(
                executable, include_external=not arguments.no_external
            )
            budget = run_budget(executable)
            attestation = build_release_attestation()
            schemas = validate_evidence_schemas()
            emit(
                {
                    "validation": validation["summary"],
                    "budget": budget["summary"],
                    "attestation": attestation["result"],
                    "schemas": schemas["status"],
                }
            )
            return int(
                validation["summary"]["failed"] != 0
                or budget["summary"]["status"] != "passed"
            )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"validation error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
