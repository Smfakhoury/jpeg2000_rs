# Final JPIP reproduction package

[FINDINGS.md](FINDINGS.md) gives a compact cited table for new findingF31
(leading-zero VBAS rejection) and additional evidence for existingF11
(initial inherited-header defaults). **Two native cases, one new root cause.**

## Run

Requires Linux, Python3.12+, Git, `patch`, and Rust/Cargo1.93.1 through rustup.
Cargo may need its ordinary dependencies on first use. No DeepSpec, SOMA,
model service, spec database or standards documents are needed.

```sh
python3 reproduce.py --list
python3 reproduce.py --repo /path/to/jpeg2000_rs --finding F31
python3 reproduce.py --repo /path/to/jpeg2000_rs --all --output ./reproduction-results
```

The local repository must contain commit
`69fe0bce9024ab5a4cb6556e3ebdf91134324148`. The runner extracts that revision
into disposable source trees and installs only the original additive test
artifacts. It does not modify the supplied checkout. Use a new output directory
with several GiB free; complete logs and `report.json` are retained.

Both selected Rust tests are **expected to fail**. Runner success requires
exit101 and every specific diagnostic marker; build failures, timeouts,
unrelated panics and unexpected passes do not count. Review the original
recipes before running them; this runner is not a sandbox.

## Evidence

The package preserves all four original submitted artifacts byte-for-byte,
original native results, build recipes, failure markers, and artifact hashes.
The other test inside the ingestion module is preserved but not selected as
an additional reproduction. [UPSTREAM-LICENSE](UPSTREAM-LICENSE) contains the
source license. Standards citations/edition hashes are included, not the
copyrighted documents.

Fresh isolated replay results are in
[verification/report.json](verification/report.json), with complete adjacent
command logs. The original recorded failure tails are also retained.
The expected errors reproduce on unmodified pinned production code.

The first fresh replay correctly exposed a too-strict output marker: serial
libtest interleaves diagnostics between the test name and `FAILED`. Its original
report, logs and manifests are retained in `verification-initial-marker-review`.
Only the package marker was made layout-independent using the named panic;
the exact error, input, exit code and failure-count checks remain mandatory.
No Rust source, test expectation or historical result was changed. Seven runner
checks cover classification, including both real interleaved logs.

```sh
python3 -m unittest test_reproduce.py
sha256sum -c SHA256SUMS
```

The acquired2022 standard and project-selected2026 edition remain unreconciled.
These are bounded parser/cache observations, not complete-image conformance
or completed Verus proofs. See the finding table for exact applicability.
