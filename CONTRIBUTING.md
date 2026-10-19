# Contributing

Please use [SECURITY.md](SECURITY.md), not public issues, for suspected
vulnerabilities or sensitive input files.

## Development setup

Install rustup; the checked-in toolchain selects the tested Rust version.
The production crate has no external Cargo dependencies. Validation tooling
has its own pinned Python requirements.

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r validation/requirements.txt
cargo test --all-targets --locked
cargo test --doc --locked
```

## Required local checks

External numerical checks require OpenJPEG's `opj_compress` and
`opj_decompress` (Debian/Ubuntu package `libopenjp2-tools`). Use
`all --no-external` only when intentionally omitting native comparisons;
that does not establish numerical agreement.

```sh
cargo fmt --all -- --check
cargo test --all-targets --locked
cargo check --release --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
python -m unittest discover -s validation/tests -v
python validation/run-validation.py all
cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked
cargo test --manifest-path fuzz/Cargo.toml --lib --locked
diff -u validation/expected-package-files.txt <(cargo package --list --locked)
```

The package-list comparison uses a Bash-compatible shell. Follow [PROOFS.md](PROOFS.md)
for the exact Verus command and [validation instructions](validation/VALIDATION.md)
for optional external oracles and official-vector runs.

## Implementation and review

- Keep one executable implementation under `src/`.
- Preserve typed failure and resource-limit behavior.
- Add a focused regression before changing decoder semantics.
- Keep specification helpers connected to concrete implementation behavior.
- Pin public reference revisions and preserve required third-party notices.
- Do not commit standards text, purchased vectors, personal credentials, native
  oracle binaries, local research material, or generated run reports.
- Minimize a redistributable failing input before adding it as a regression.
- Avoid unreviewed additions to the supported standards profile.

## Fuzzing

The [fuzz crate](fuzz/) provides arbitrary-input and structured-input targets.
Compile targets independently from the main crate:

```sh
cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked
cargo run --manifest-path fuzz/Cargo.toml --bin arbitrary_codestream -- \
  -runs=1000 -max_len=4096
```

Smoke runs do not establish coverage or sanitizer completeness. Use an
appropriately instrumented cargo-fuzz/nightly setup for sustained campaigns.
Keep campaign output outside tracked source, and include execution budgets and
reproduction commands when reporting a result.

With cargo-fuzz and a nightly toolchain installed, run a bounded,
AddressSanitizer/coverage-guided check on a copy of the structured seeds:

```sh
mkdir -p target/validation/fuzz-corpus
cp -R fuzz/corpus/structured_framing/. target/validation/fuzz-corpus/
cargo +nightly fuzz run structured_framing target/validation/fuzz-corpus -- \
  -runs=1000 -max_len=4096 -timeout=20 -rss_limit_mb=2048 \
  -artifact_prefix=target/validation/
```

This is still a smoke budget, not a sustained coverage campaign. Do not mutate
the checked-in corpus during exploratory runs.

Fuzz targets share a tighter profile than the production defaults: one-MiB
input/metadata, four-MiB decoded storage, 256 code-blocks, 4,096 packets,
256 JPIP fragments and one million work units. These bounds leave room for
AddressSanitizer overhead within the process RSS cap. Larger production
resource profiles need separate representative-image budget tests.

For a longer bounded run, replace `-runs=1000` with
`-max_total_time=60 -print_final_stats=1`. Apply the same explicit time, input,
timeout and RSS budgets to all seven targets. Capture the source commit/dirty
state, harness and binary hashes, nightly/cargo-fuzz versions, final execution
counts, coverage counters, and peak RSS. Keep corpus growth and crash artifacts
in the ignored output directory; a successful minute is not exhaustive coverage.
