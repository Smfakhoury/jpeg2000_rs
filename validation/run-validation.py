#!/usr/bin/env python3
"""CI-friendly decoder checks; reports are outputs, never publication approvals."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from framework import (
    build_subject,
    generate_corpus,
    resolve_subject,
    run_budget,
    run_official_vectors,
    run_validation,
    validate_evidence_schemas,
    validate_normative_coverage,
    verify_manifest,
)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    commands = result.add_subparsers(dest="command", required=True)
    generate = commands.add_parser("generate")
    generate.add_argument("--check", action="store_true")
    commands.add_parser("verify")
    commands.add_parser("inventory")
    schemas = commands.add_parser("schemas")
    schemas.add_argument("--output-dir", type=Path)
    for name in ("run", "budget", "official", "all"):
        command = commands.add_parser(name)
        command.add_argument("--output-dir", type=Path)
        command.add_argument("--subject", type=Path)
        command.add_argument("--no-build", action="store_true")
        if name in ("run", "all"):
            command.add_argument("--no-external", action="store_true")
            command.add_argument("--oracle-config", type=Path)
            command.add_argument("--adjudication-file", type=Path)
            command.add_argument("--timeout-seconds", type=float, default=5.0)
        if name in ("budget", "official"):
            command.add_argument("--max-case-ms", type=float, default=10000.0 if name == "official" else 5000.0)
            command.add_argument("--max-total-ms", type=float, default=300000.0 if name == "official" else 30000.0)
        if name == "official":
            command.add_argument("--vector-root", type=Path)
            command.add_argument("--manifest", type=Path)
            command.add_argument("--access-status", choices=[
                "authentication-required", "license-blocked", "not-configured", "unavailable",
            ], default="not-configured")
            command.add_argument("--blocked-reason", default="official vector root is not configured")
            command.add_argument("--final-url")
            command.add_argument("--http-status", type=int)
            command.add_argument("--max-input-bytes", type=int, default=64 * 1024 * 1024)
            command.add_argument("--max-output-bytes", type=int, default=64 * 1024 * 1024)
            command.add_argument("--max-address-space-bytes", type=int, default=512 * 1024 * 1024)
    return result


def subject_for(arguments: argparse.Namespace) -> Path:
    if arguments.subject:
        return arguments.subject.resolve()
    return resolve_subject() if arguments.no_build else build_subject()


def emit(value: object) -> None:
    print(json.dumps(value, indent=2, sort_keys=True))


def main() -> int:
    arguments = parser().parse_args()
    try:
        if arguments.command == "generate":
            emit({"status": "passed", "entries": len(generate_corpus(arguments.check)["entries"])})
        elif arguments.command == "verify":
            emit(verify_manifest())
        elif arguments.command == "inventory":
            emit(validate_normative_coverage())
        elif arguments.command == "schemas":
            emit(validate_evidence_schemas(arguments.output_dir))
        elif arguments.command == "official":
            result = run_official_vectors(
                executable=subject_for(arguments) if arguments.subject or arguments.no_build else None,
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
                output_dir=arguments.output_dir,
            )
            emit({key: result[key] for key in ("status", "selection", "summary", "blockers")})
            return int(result["status"] != "passed")
        elif arguments.command == "budget":
            result = run_budget(
                subject_for(arguments), arguments.max_case_ms,
                arguments.max_total_ms, output_dir=arguments.output_dir,
            )
            emit(result["summary"])
            return int(result["summary"]["status"] != "passed")
        else:
            if arguments.command == "all":
                generate_corpus(check=True)
                emit(verify_manifest())
                emit(validate_evidence_schemas(arguments.output_dir))
            executable = subject_for(arguments)
            result = run_validation(
                executable,
                include_external=not arguments.no_external,
                timeout_seconds=arguments.timeout_seconds,
                oracle_config=arguments.oracle_config,
                adjudication_file=arguments.adjudication_file,
                output_dir=arguments.output_dir,
            )
            emit(result["summary"])
            failed = result["summary"]["status"] != "passed"
            if arguments.command == "all":
                budget = run_budget(executable, output_dir=arguments.output_dir)
                emit(budget["summary"])
                emit(validate_evidence_schemas(arguments.output_dir))
                failed |= budget["summary"]["status"] != "passed"
            return int(failed)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"validation error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
