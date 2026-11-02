# JPEG 2000 deviation review and reproduction bundle

Start with **[FINDINGS.md](FINDINGS.md)**: a human-readable table linking each
reviewed mismatch to its document citation, pinned source location, and native
failure evidence.

## What is included

- 27 conservative review families, separated into standards-grounded behavior,
  structural-validation gaps, and documented project behavior.
- Original accepted test artifacts and build/test argument lists under `cases/`.
- Recorded host failure output and identification/provenance under `evidence/`.
- `manifest.json`: case-to-finding mapping, artifact SHA-256 hashes, document
  edition/hash metadata, and the pinned upstream revision.
- `reproduce.py`: a standalone runner requiring no DeepSpec, SOMA, model access,
  specification database, or standards PDFs.

These tests are regression witnesses: **they are expected to fail on the pinned
implementation**. The runner reports `reproduced` only when the native command
returns the recorded failure code **and** its specific diagnostic markers appear.
A build error, timeout, missing test, unrelated panic, or newly passing test does
not count as reproducing the finding.

## Run

Requirements: Linux, Python 3.12+, Git, Bash/sh, `patch`, and Rust/Cargo 1.93.1
through rustup. Cargo may need network access for ordinary crate dependencies on
its first run. No Verus verification is performed.

```sh
git clone https://github.com/microsoft/jpeg2000_rs.git
python3 reproduce.py --list
python3 reproduce.py --repo ./jpeg2000_rs --finding F18
python3 reproduce.py --repo ./jpeg2000_rs --finding F13 --finding F17
python3 reproduce.py --repo ./jpeg2000_rs --all --output ./reproduction-results
```

An existing clone containing commit
`69fe0bce9024ab5a4cb6556e3ebdf91134324148` also works. The runner uses `git archive`
of that exact commit, not the clone's current checkout. It does not modify that
repository, its uncommitted files, or the packaged evidence.

Each harness builds in its own disposable source tree. Shared cases build once
per run, execute sequentially, and leave complete command logs plus `report.json`
in the requested **new** output directory. Temporary build trees are removed
after their results are saved; keep several GiB free. Use `--timeout` to adjust
the per-command timeout, or `--toolchain` to explicitly select another installed
toolchain (results then describe that toolchain).

Review the captured recipes and artifacts before running them: this executes
Rust, Python, and shell test/build code. The runner is not a security sandbox.

## Read the evidence correctly

“Reproduced” is a bounded native observation with reviewed source/document
support, not a formal proof, certified decoder result, or an assertion that every
row has a separate root cause. Candidate diagnoses, unsupported inferred-intent
families, and the known overstrong cross-box hypothesis conjunctions are excluded.

The structural-validation table is deliberately separate: a violated file
invariant does not by itself imply that every permissive reader is required to
reject that file. The project-behavior table is not a list of universal standard
prohibitions. Test scopes and specific caveats are listed below the table.

The acquired T.800/T.801/T.808 editions differ from newer editions selected by the
project. This bundle does not resolve those edition differences. It includes
citations and hashes, **not copies of the standards**.

Original evidence was retained read-only. Fresh bundle-verification results are
separate from those historical observations. The full repository evaluation
continues outside this bundle; this is a review snapshot, not its final report.

Original harness files can contain additional, unselected tests; the runner
executes only the cases in the manifest. Claims are limited to the reviewed
witnesses, not every assertion in a multi-condition test.

## Attribution and runner checks

The target and copied source overlays are from Microsoft Corporation's
`jpeg2000_rs`; its MIT license is retained in [UPSTREAM-LICENSE](UPSTREAM-LICENSE).
Test registration overlays and patches expose or exercise the existing
implementation; they are not proposed production fixes.

Run `python3 -m unittest test_reproduce.py` from this directory to check the
runner's failure matching, timeout handling, and artifact-path validation.

## Bundle verification

All **27 selected native cases across 24 harnesses reproduced their expected
failures** in fresh, isolated copies of the pinned revision. See the
[full replay report](verification/report.json) and its adjacent complete logs.
Canonical tracked source remained unchanged.

The final runner also passed six runner-check tests and a separate
[F18 replay](verification-runner-final/report.json), which records the final
runner and manifest hashes. `SHA256SUMS` covers the packaged files; verify it from
this directory with `sha256sum -c SHA256SUMS`.
