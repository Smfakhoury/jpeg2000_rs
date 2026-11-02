#!/usr/bin/env python3
"""Reproduce reviewed failures in disposable copies of an exact upstream revision."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
from uuid import uuid4


PACKAGE = Path(__file__).resolve().parent


def artifact_path(value):
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise ValueError(f"Unsafe artifact path: {value}")
    return Path(*path.parts)


def execute(argv, cwd, environment, logfile, timeout):
    started = time.monotonic()
    with logfile.open("x") as output:
        process = subprocess.Popen(
            argv, cwd=cwd, env=environment, stdout=output, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            code = process.wait(timeout=timeout)
            timed_out = False
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait()
            timed_out = True
    return {"argv": argv, "exit_code": code, "timed_out": timed_out,
            "duration_s": round(time.monotonic() - started, 3),
            "log": logfile.name}


def classify_failure(run, text, case):
    missing = [marker for marker in case["markers"] if marker not in text]
    reproduced = (
        run["exit_code"] != 0 and run["exit_code"] == case["expected_exit"]
        and not run["timed_out"] and bool(case["markers"])
        and all(case["markers"]) and not missing
    )
    return {"status": "reproduced" if reproduced else "unexpected_result",
            "missing_expected_markers": missing}


def reproduce_suite(suite, cases, repository, revision, output, timeout, toolchain):
    logs = output / suite["id"]
    logs.mkdir()
    result = {"suite": suite["id"], "build": [], "tests": []}
    with tempfile.TemporaryDirectory(prefix=suite["id"] + "-", dir=output / ".work") as temp:
        temp = Path(temp)
        workspace = temp / "source"
        workspace.mkdir()
        archive_path = temp / "source.tar"
        with archive_path.open("xb") as stream:
            subprocess.run(["git", "-C", str(repository), "archive", "--format=tar", revision],
                           stdout=stream, check=True)
        with tarfile.open(archive_path) as archive:
            archive.extractall(workspace, filter="data")
        archive_path.unlink()
        for relative, expected in suite["artifacts"].items():
            name = artifact_path(relative)
            source = PACKAGE / "cases" / suite["id"] / "artifacts" / name
            data = source.read_bytes()
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError(f"Artifact checksum mismatch: {source}")
            target = workspace / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        environment = os.environ.copy()
        for name in ("CARGO_TARGET_DIR", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                     "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
            environment.pop(name, None)
        environment["GIT_CEILING_DIRECTORIES"] = str(temp)
        environment["CARGO_TERM_COLOR"] = "never"
        environment["RUST_TEST_THREADS"] = "1"
        if toolchain:
            environment["RUSTUP_TOOLCHAIN"] = toolchain
        for index, argv in enumerate(suite["build"]):
            run = execute(argv, workspace, environment, logs / f"build-{index}.log", timeout)
            result["build"].append(run)
            if run["exit_code"] != 0 or run["timed_out"]:
                result["status"] = "build_failed"
                return result
        for case in cases:
            run = execute(case["command"], workspace, environment,
                          logs / f"{case['id']}.log", timeout)
            text = (logs / run["log"]).read_text(errors="replace")
            result["tests"].append({
                "case": case["id"], "findings": case["finding_ids"], **run,
                **classify_failure(run, text, case),
            })
    result["status"] = (
        "reproduced" if all(t["status"] == "reproduced" for t in result["tests"])
        else "unexpected_result"
    )
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--all", action="store_true", help="Run all packaged regression cases")
    choice.add_argument("--finding", action="append", help="Review-table ID; repeat to select several")
    choice.add_argument("--list", action="store_true", help="List packaged findings without execution")
    parser.add_argument("--repo", type=Path, help="Existing local upstream Git repository")
    parser.add_argument("--output", type=Path, help="New directory for complete logs and report.json")
    parser.add_argument("--timeout", type=float, default=600, help="Seconds per build/test command")
    parser.add_argument("--toolchain", default="1.93.1", help="Rustup toolchain; empty uses environment")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    manifest = json.loads((PACKAGE / "manifest.json").read_text())
    by_id = {f["id"]: f for f in manifest["findings"]}
    if args.list:
        for identity, finding in by_id.items():
            print(f"{identity} [{finding['group']}] {finding['title']}")
        return 0
    if args.repo is None:
        parser.error("--repo is required for execution")
    selected = set(by_id) if args.all else set(args.finding)
    if selected - by_id.keys():
        parser.error("Unknown finding IDs: " + ", ".join(sorted(selected - by_id.keys())))
    repository = args.repo.resolve()
    subprocess.run(["git", "-C", str(repository), "cat-file", "-e",
                    manifest["revision"] + "^{commit}"], check=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = (args.output or Path(f"jpeg-review-results-{stamp}-{uuid4().hex[:6]}")).resolve()
    output.mkdir(parents=True, exist_ok=False)
    (output / ".work").mkdir()
    report = {"revision": manifest["revision"], "selected_findings": sorted(selected),
              "started_at": stamp, "suites": [], "toolchain": args.toolchain,
              "python_version": sys.version,
              "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "manifest_sha256": hashlib.sha256((PACKAGE / "manifest.json").read_bytes()).hexdigest(),
              "qualification": "Expected original failures, not successful conformance tests."}
    try:
        for suite in manifest["suites"]:
            cases = [c for c in suite["tests"] if selected.intersection(c["finding_ids"])]
            if not cases:
                continue
            result = reproduce_suite(
                suite, cases, repository, manifest["revision"], output,
                args.timeout, args.toolchain,
            )
            report["suites"].append(result)
            (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
            print(f"{suite['id']}: {result['status']}", flush=True)
    finally:
        (output / ".work").rmdir()
    passed = all(s["status"] == "reproduced" for s in report["suites"])
    report["status"] = "reproduced" if passed else "needs_review"
    report["executed_cases"] = sum(len(s["tests"]) for s in report["suites"])
    report["finished_at"] = datetime.now(timezone.utc).isoformat()
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"{report['status']}: {report['executed_cases']} cases; logs: {output}", flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
