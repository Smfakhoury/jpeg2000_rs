#!/usr/bin/env python3
"""Verify and replay the bundled JPEG 2000 findings without DeepSpec or SOMA."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile
from uuid import uuid4


ROOT = Path(__file__).resolve().parent
REVISION = "69fe0bce9024ab5a4cb6556e3ebdf91134324148"
ARCHIVES = {
    "jpeg2000-review-package-20261029": "82699bdcf73436ce711b61dc7d309c9e880d370a280c2bdf73a01824edc3f3ac",
    "jpeg2000-review-supplement-20261030": "b6553db1e0262265c38659e1900a561ae5f33e7d9dc7bb442bdcb68d3cbd5294",
    "jpeg2000-final-jpip-witnesses-20261030": "5a3c1ad255a4ab9915ba6325b6cf7a95c8f56d82782e67100522ca44eb40a80a",
    "jpeg2000-u-extension-witness-20261030": "47f75362318c29461d57ff039293211b06c8bf8b4487c853d279d2507ec67565",
    "jpeg2000-transform-witnesses-20261030": "564e795646daac9dda2a52fb914e1d3caff18b1e25a34ed8280b98cf18e942f0",
}
TRANSFORMS = "jpeg2000-transform-witnesses-20261030"
FINDINGS = {f"F{i:02}" for i in range(1, 32)} | {"T1", "T2", "U1"}


def verify_integrity(root):
    manifests = {}
    checked = 0
    for name, expected in ARCHIVES.items():
        path = root / (name + ".tar.gz")
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError(f"Archive checksum mismatch: {path}")
        with tarfile.open(path, "r:gz") as archive:
            for member in archive.getmembers():
                relative = PurePosixPath(member.name)
                if relative.is_absolute() or ".." in relative.parts or relative.parts[0] != name:
                    raise ValueError(f"Unsafe archive member: {member.name}")
                if member.isdir():
                    continue
                if not member.isfile():
                    raise ValueError(f"Unexpected non-file archive member: {member.name}")
                if name != TRANSFORMS:
                    actual = root / member.name
                    if archive.extractfile(member).read() != actual.read_bytes():
                        raise ValueError(f"Unpacked evidence differs from sealed archive: {actual}")
                    checked += 1
            if name == TRANSFORMS:
                original = archive.extractfile(
                    name + "/src/codestream/wave86_transform_witnesses.rs"
                ).read()
                if original != (root / "jpeg2000-review-test-sources/wave86_transform_witnesses.rs").read_bytes():
                    raise ValueError("Displayed transform test differs from archived original")
        if name != TRANSFORMS:
            manifest = json.loads((root / name / "manifest.json").read_text())
            if manifest["revision"] != REVISION:
                raise ValueError(f"Unexpected source revision in {name}")
            manifests[name] = manifest
    covered = {f["id"] for m in manifests.values() for f in m["findings"]} | {"T1", "T2"}
    if covered != FINDINGS:
        raise ValueError("Packaged finding coverage is incomplete")
    return manifests, checked


def selected_cases(manifest, selected):
    return {
        (suite["id"], case["id"])
        for suite in manifest["suites"]
        for case in suite["tests"]
        if selected.intersection(case["finding_ids"])
    }


def report_matches(report, expected, selected):
    if (report.get("status") != "reproduced" or report.get("revision") != REVISION
            or report.get("toolchain") != "1.93.1"
            or set(report.get("selected_findings", [])) != selected):
        return False
    seen = []
    for suite in report.get("suites", []):
        if suite.get("status") != "reproduced" or not suite.get("build"):
            return False
        if any(b.get("exit_code") != 0 or b.get("timed_out") is not False for b in suite["build"]):
            return False
        for test in suite.get("tests", []):
            if (test.get("status") != "reproduced" or test.get("exit_code") != 101
                    or test.get("timed_out") is not False or test.get("missing_expected_markers") != []):
                return False
            seen.append((suite["suite"], test["case"]))
    return (bool(expected) and set(seen) == expected and len(seen) == len(expected)
            and report.get("executed_cases") == len(expected))


def transform_expectation(selected):
    if selected == {"T1"}:
        selector, counts = "wave86_arbitrary_", "1 passed; 1 failed; 0 ignored"
    elif selected == {"T2"}:
        selector, counts = "wave86_irreversible_singleton_standard_witness", "0 passed; 1 failed; 0 ignored"
    elif selected == {"T1", "T2"}:
        selector, counts = "wave86_", "1 passed; 2 failed; 0 ignored"
    else:
        raise ValueError("Select T1, T2, or both")
    markers = ["test result: FAILED. " + counts]
    if "T1" in selected:
        markers += [
            "wave86_arbitrary_even_origin_control ... ok",
            "thread 'codestream::tests::wave86_arbitrary_odd_origin_standard_witness'",
            "left: [218, 308, 268]", "right: [118, 208, 168]",
        ]
    if "T2" in selected:
        markers += [
            "thread 'codestream::tests::wave86_irreversible_singleton_standard_witness'",
            "left: [131072]", "right: [65536]",
        ]
    return selector, markers


def run_transforms(runner, selected, output, timeout):
    selector, markers = transform_expectation(selected)
    argv = ["cargo", "test", "--locked", "--lib", selector,
            "--", "--nocapture", "--test-threads=1"]
    environment = os.environ.copy()
    for key in ("CARGO_TARGET_DIR", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        environment.pop(key, None)
    environment.update(RUSTUP_TOOLCHAIN="1.93.1", CARGO_TERM_COLOR="never", RUST_TEST_THREADS="1")
    with tempfile.TemporaryDirectory(prefix="transform-source-", dir=output) as temp:
        environment["GIT_CEILING_DIRECTORIES"] = temp
        with tarfile.open(ROOT / (TRANSFORMS + ".tar.gz"), "r:gz") as archive:
            archive.extractall(temp, filter="data")
        log = output / "transforms.log"
        result = runner.execute(argv, Path(temp) / TRANSFORMS, environment, log, timeout)
        verdict = runner.classify_failure(
            result, log.read_text(errors="replace"), {"expected_exit": 101, "markers": markers})
    return {"package": TRANSFORMS, "findings": sorted(selected), **result, **verdict}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--check", action="store_true", help="Verify bundled evidence without executing tests")
    choice.add_argument("--all", action="store_true", help="Replay all 33 review rows plus qualified U1")
    choice.add_argument("--finding", choices=sorted(FINDINGS), action="append", help="Repeat to select rows")
    parser.add_argument("--repo", type=Path, default=ROOT.parent, help="Git checkout containing the pinned revision")
    parser.add_argument("--output", type=Path, help="New directory for complete logs and aggregate report")
    parser.add_argument("--timeout", type=float, default=600, help="Seconds per child build/test command")
    args = parser.parse_args()
    if sys.version_info < (3, 12):
        parser.error("Python 3.12 or newer is required")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be a positive finite number")
    manifests, checked = verify_integrity(ROOT)
    print(f"Integrity verified: five archives, {checked} unpacked files, 33 review rows plus U1.", flush=True)
    if args.check:
        return 0
    missing = [name for name in ("git", "cargo", "rustup", "patch", "bash", "sh", "python3")
               if shutil.which(name) is None]
    if missing:
        parser.error("Missing prerequisites: " + ", ".join(missing))
    subprocess.run(["rustup", "run", "1.93.1", "rustc", "--version"], check=True)
    repository = args.repo.resolve()
    subprocess.run(["git", "-C", str(repository), "cat-file", "-e", REVISION + "^{commit}"], check=True)
    selected = FINDINGS if args.all else set(args.finding)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = (args.output or Path(f"deviation-results-{stamp}-{uuid4().hex[:6]}")).resolve()
    output.mkdir(parents=True, exist_ok=False)
    module_spec = importlib.util.spec_from_file_location(
        "bundled_reproduction", ROOT / "jpeg2000-review-package-20261029/reproduce.py")
    if module_spec is None or module_spec.loader is None:
        raise RuntimeError("Cannot load the integrity-verified bundled runner")
    runner = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(runner)
    jobs = [(name, manifest, selected & {f["id"] for f in manifest["findings"]})
            for name, manifest in manifests.items()]
    jobs = [job for job in jobs if job[2]]
    transforms = selected & {"T1", "T2"}
    expected_components = len(jobs) + bool(transforms)
    report = {
        "revision": REVISION, "selected_findings": sorted(selected), "components": [],
        "started_at": stamp, "toolchain": "1.93.1", "status": "running",
        "qualification": "Expected-failure reproductions, not conformance certification. U1 remains qualified; F11 has two wrappers. Newer-edition reconciliation remains outstanding.",
    }
    try:
        for name, manifest, subset in jobs:
            argv = [sys.executable, str(ROOT / name / "reproduce.py"), "--repo", str(repository),
                    "--output", str(output / name), "--timeout", str(args.timeout)]
            for finding in sorted(subset):
                argv.extend(["--finding", finding])
            expected = selected_cases(manifest, subset)
            commands = sum(len(s["build"]) + len(s["tests"]) for s in manifest["suites"])
            result = runner.execute(argv, ROOT, os.environ.copy(), output / (name + ".log"),
                                    commands * (args.timeout + 5) + 30)
            child_path = output / name / "report.json"
            child = json.loads(child_path.read_text()) if child_path.exists() else {}
            passed = (result["exit_code"] == 0 and not result["timed_out"]
                      and report_matches(child, expected, subset))
            report["components"].append({
                "package": name, "findings": sorted(subset), **result,
                "status": "reproduced" if passed else "needs_review",
                "expected_cases": len(expected), "observed_cases": child.get("executed_cases"),
                "report": str(child_path.relative_to(output)) if child_path.exists() else None,
            })
            print(f"{name}: {report['components'][-1]['status']}", flush=True)
        if transforms:
            report["components"].append(run_transforms(runner, transforms, output, args.timeout))
            print(f"Transforms: {report['components'][-1]['status']}", flush=True)
    finally:
        passed = (len(report["components"]) == expected_components
                  and all(c["status"] == "reproduced" for c in report["components"]))
        report.update(status="reproduced" if passed else "needs_review",
                      finished_at=datetime.now(timezone.utc).isoformat())
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"{report['status']}: {output / 'report.json'}", flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
