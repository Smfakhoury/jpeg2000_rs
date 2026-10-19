# Verus proofs

## Single executable implementation

Cargo and Verus use the exact [src/lib.rs](src/lib.rs) crate root. Production
algorithms live in [src/](src/); a second proof-only decoder implementation is
not maintained.

Verus-only attributes and modules are enabled with `verus_keep_ghost`:

- [src/refinement.rs](src/refinement.rs) contains executable helpers with
  functional contracts.
- [specs/](specs/) contains mathematical models.
- [proofs/shared/](proofs/shared/) contains representation views.
- [proofs/spec_properties/](proofs/spec_properties/) contains model lemmas.
- [proofs/production_impl/](proofs/production_impl/) contains refinement lemmas.

Ordinary Rust builds erase ghost code and retain Rust's borrow checking.
Source and proof changes must be validated together.

## Reproduce verification

The [Verus workflow](.github/workflows/verus.yml) pins the verifier archive,
checksum, required Rust toolchain, and command. Use the same versions when
reproducing it locally. Do not substitute a rolling compiler without
revalidating the source and standard-library specifications.

After installing the workflow's verifier and Rust toolchain:

```sh
/path/to/verus --no-lifetime --output-json --crate-type lib src/lib.rs \
  > target/validation/verus-result.json
python3 validation/generate-proof-coverage.py \
  target/validation/verus-result.json
```

Create the output directory first. Reports are generated artifacts, not
committed snapshots.

The checker rejects failed or partial verification and unaccounted termination
bypasses or forbidden trusted codec escapes. Keep this gate enabled; removing
old report files must not weaken the source checks.

## Current lifetime-analysis limitation

The pinned released verifier requires `--no-lifetime` for verified-module
constant initialization because of an upstream THIR/erasure initialization
error. The proposed upstream fix is
[verus-lang/verus#3266](https://github.com/verus-lang/verus/pull/3266).

This flag disables Verus lifetime analysis, not Rust borrow checking.
Documentation must not imply that lifetime analysis ran. Remove the flag
only after a reproducibly pinned fixed verifier passes the exact production
root and the relevant upstream regression suites.

## What the proofs establish

Verification checks the properties expressed by contracts and loop invariants,
including bounds, arithmetic checks, termination, and selected functional
relations to specification helpers. The technical inventory
[normative-coverage.json](normative-coverage.json) links implementation,
contract, specification, proof, and test symbols.

That inventory is not an exhaustive normative-clause checklist. A helper
contract is only as strong as its model and its relationship to the production
state/output. Review those relationships independently; symbol links and
successful obligation counts alone do not prove full standards conformance.

Numerical agreement of irreversible transforms, fixed-point color conversion,
and nonlinear transforms requires separate conformance metrics and reference
outputs. The Rust/Verus toolchains, solver, and reviewed standard-library
contracts remain part of the trusted computing base.

## Changes to proof code

- Do not introduce codec `external_body`, `assume`, or `admit` escapes.
- Use explicit loop progress measures; do not bypass termination obligations.
- Add concrete postconditions/refinement evidence rather than disconnected
  model-only lemmas.
- Run the exact-root verifier and ordinary Rust tests.
- Review [supported-profile limitations](SUPPORTED.md) before strengthening
  public claims.
