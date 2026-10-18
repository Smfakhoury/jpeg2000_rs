# Release process

`publish = false` is mandatory until every gate below is satisfied and the
release owner explicitly approves publication.

## Required bindings

Every release candidate must bind the following to one immutable source
revision:

- support matrix and exact normative editions;
- Rust and Verus toolchains;
- production source and proof coverage;
- all corpus manifests and vector hashes;
- native oracle source and binary hashes;
- conformance, differential, negative, fuzz, and performance results;
- package file list and package-content audit;
- CELA/legal, architecture, and release-owner approvals.

## Gates

1. The support matrix is approved and unsupported features return typed errors.
2. CELA/legal approves standards access, vector use, redistribution, and crate
   publication.
3. Cargo build, tests, Clippy with warnings denied, docs, and package checks
   pass on supported platforms.
4. Pinned Verus verifies the exact production `src/lib.rs` with zero errors.
5. Every shipped production function has exactly one current proof
   disposition; incomplete or newly trusted functions equal zero.
   `normative-coverage.json` must also pass its schema and source-evidence
   validator for all selected Parts 1, 2, 9, and 15 groups; safety-only proofs
   do not count as functional links.
6. Required Part 4 vectors pass for every claimed profile.
7. Open-corpus provenance is reviewed per file; differential disagreements are
   preserved and adjudicated.
8. Lossless output is bit-exact. Lossy output meets the approved metrics and
   tolerances on every supported platform.
9. Negative, metamorphic, fuzz, and sanitizer gates pass within the approved
   resource budgets.
10. Performance and malformed-input worst-case regressions remain within
    approved thresholds.
11. `cargo package --list` contains only approved production and documentation
    files, with no proofs, restricted vectors, native binaries, generated
    build output, or browser adapters.
12. The final release attestation validates against
    `release-attestation.schema.json`, and the release owner approves it.

## Checkpoint report

Each checkpoint reports support-matrix changes, exact editions, implemented
features, proof coverage, assumptions, corpus totals, differential
disagreements, lossy metrics, fuzz evidence, performance and peak memory,
known risks, immutable revision, and reproducible commands.

## Phase H generated validation

The decoder-validation framework is documented in `validation/VALIDATION.md`. Its
corpus is exclusively deterministic project-generated data. It does not
replace required Part 4 vectors and makes no official conformance claim.

Run its CI gate with:

```sh
python3 -m unittest discover -s validation/tests -v
python3 validation/run-validation.py all
```

Missing OpenJPEG, Grok, or JasPer executables are recorded as skipped, not
passed. Before release, rerun with approved oracle builds, preserve and
adjudicate every disagreement, then run legally approved official vectors from
an ephemeral external root as documented in `validation/VALIDATION.md`. Never
copy or package those assets in the repository.

## Phase I release evidence

The non-git workspace uses `source-tree.json` as its immutable revision. The
`sha256-tree-v1` digest is computed over sorted path, byte length, and SHA-256
records for all source-of-truth files; generated validation evidence, build
output, and the manifest itself are excluded. Verify it without rewriting it:

```sh
python3 validation/run-validation.py source --check
```

`architecture-review.json` records the locally actionable evidence checklist.
`legal-publication-review.json` and `owner-approval.json` are deliberately
pending placeholders and must only be completed by the named authorities.
Oracle differences use deterministic IDs in
`validation/results/oracle-adjudications.json`; do not change a record to
`adjudicated` without a decision, rationale, adjudicator, and timestamp.

Run the complete local candidate gate:

```sh
python3 -m pip install -r validation/requirements.txt
python3 -m unittest discover -s validation/tests -v
python3 validation/run-validation.py all
python3 validation/run-validation.py source --check
python3 validation/run-validation.py architecture --check
python3 validation/run-validation.py schemas
cargo test --all-targets --locked
cargo check --release --all-targets --locked
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
diff -u validation/expected-package-files.txt <(cargo package --list --locked)
```

`release-attestation.schema.json` requires the source, toolchain, corpus,
proof, validation, budget, adjudication, architecture, and approval bindings.
A release attestation with `status: passed` is schema-invalid unless every
external gate is complete and `publish_allowed` is true. Locally passing
framework gates never imply ISO/IEC 15444-4 conformance.
