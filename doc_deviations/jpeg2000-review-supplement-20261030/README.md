# JPEG 2000 reproduction supplement

[FINDINGS.md](FINDINGS.md) adds three reviewed findings, F28-F30, to the
unchanged [27-case bundle](../jpeg2000-review-package-20261029/README.md).
This supplement is independently runnable; the earlier bundle is not required.

F28 concerns a **SHOULD recommendation** about no-action instructions.
F29 and F30 concern optional SOP and terminal packet-header stuffing.
Each row links a document citation, pinned source location, original native
failure, and replay recipe. These are bounded unit observations, not full
encoded-image reproductions, completed Verus proofs, or decoder certification.

## Run

Requirements: Linux, Python 3.12+, Git, Bash/sh, `patch`, and Rust/Cargo 1.93.1
through rustup. Cargo may need its ordinary dependencies on first use.
No DeepSpec, SOMA, specification database, model service, or standards PDFs
are needed.

```sh
python3 reproduce.py --list
python3 reproduce.py --repo /path/to/jpeg2000_rs --finding F29
python3 reproduce.py --repo /path/to/jpeg2000_rs --all --output ./reproduction-results
```

The repository must contain commit
`69fe0bce9024ab5a4cb6556e3ebdf91134324148`. The runner uses `git archive`
of that revision in disposable source trees, leaving the checkout unchanged.
Use a new output directory with several GiB free; complete command logs and
`report.json` are retained there.

The selected Rust regressions are **expected to fail**. The runner reports
`reproduced` only when the recorded failure exit code and every specific
diagnostic marker match. Build errors, timeouts, unrelated failures, and
unexpected passes do not count. Review the recipes before executing them;
the runner is not a sandbox.

## Evidence and limits

`manifest.json` records the three cases, seven original submitted artifacts,
their SHA-256 hashes, document editions, and source revision. Original
submissions and historical output are retained; they are not proposed fixes.
The source's MIT license is included in [UPSTREAM-LICENSE](UPSTREAM-LICENSE).

F29-F30 rely on reviewed normative mapping-parent authority. Their internal
model-routing/accounting clauses must not be mistaken for independent
standards oracles. Unselected cases inside the original submitted modules
are not additional reproduced findings. In particular, the no-EPH count
witness's separate test-error attribution is retained, not promoted.

T.800 (2019), T.801 (2021), and T.808 (2022) differ from newer editions selected
by the project; those differences remain unresolved. Citations and hashes are
included, not copyrighted standards documents. Candidates and unsupported
hypotheses are excluded. The full repository evaluation remains unfinished.

## Verification

All three selected cases freshly reproduced their expected native failures
in isolated copies of the pinned revision using Rust 1.93.1. See
[verification/report.json](verification/report.json) and the adjacent complete
logs. Each build succeeded; each selected test returned exit 101 with its
specific expected diagnostic. The report records runner and manifest hashes.

The six runner checks also passed. Run `python3 -m unittest test_reproduce.py`
to repeat them. Verify all packaged files with `sha256sum -c SHA256SUMS`.
