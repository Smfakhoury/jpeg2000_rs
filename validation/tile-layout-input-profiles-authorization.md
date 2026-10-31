# Exact deferred tile-window and layout-prologue profile operation

## Separate decisions and limits

The accompanying registry proposes only the exact nine-path #200 source record.
This document separately proposes one deferred operation adding two independent
fixed input-producer profiles. Neither decision is active on this branch. Both
require complete checks, original-artifact qualification, a fresh Ready policy
check, independent engineering review and normal adoption on trusted main.

The operation below must wait until #200 itself is finally refreshed, fully
qualified, separately source-reviewed, normally merged and accepted by fresh
actual-main validation. Record adoption alone does not meet that condition.
All earlier forwards, including #196, #199 and #203, are spent.

No tile-window, layout-prologue, SOT or main-component source record is added
here. This is not permission for combined source amendments, different models,
additional files, changed toolchain/dependency pins, resource caps,
fixture/oracle policy, publication, visibility/history/tag changes or normative
certification.

## Exact permitted operation

Replace only these regular files, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `c89bbc969173e6d4d7bb547cf8e06ceedba4f831` | `0dd06ce928e91d06abd47891577bdb217fef521b` |
| `validation/tests/test_model_review_hold.py` | `6e55976d832da89f9f301adef1643b30d5b0f05e` | `71613c7da2eb0f13edddadb053afc1b02b49571d` |

Replacement bytes are retained in inactive maintenance candidate
`23fe4c57d7ca0816d19a8a84439c15bd172cbb01`. All fifteen existing profile objects,
eight controller functions, existing test-function bodies, schema,
exact-blob/regular-file checks, trusted-main selection and registry data remain
unchanged.

The separate seven-path `tile-data-window` profile is:

- `src/codestream/tiles.rs`
- `src/codestream/proof_support.rs`
- `src/codestream/tests.rs`
- `specs/packet_input.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

Its source scope is the actual raw tile-data range helper and its real
`read_tile_part` call, one additive source-derived model in the existing packet
input module and one closed projection, complete result/parser frames and strict
raw/real-reader regressions. Preserve executing order, error precedence and the
original early zero-length versus late fixed-length cursor commits around
metadata-finalization errors. Previously integrated models stay unchanged.

The separate nine-path `layout-prologue` profile is:

- `src/codestream/layout.rs`
- `src/codestream/proof_support.rs`
- `src/codestream/tests.rs`
- `specs/layout_input.rs`
- `specs/mod.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`
- `validation/expected-package-files.txt`

Its source scope is the actual layout SOC/SIZ admission helper and real layout
call, one additive source-derived model and closed projection, its module and
exact package entry, complete raw result/parser frames and strict real-reader
regressions. Reuse unchanged reader/SIZ contracts and preserve their original
order, partial updates, errors, image fields, raw capability bits and header
start. Remaining layout orchestration and all existing models stay unchanged.

Each later source record must independently bind its entire exact profile and
all before/after identities. These are not a combined source profile or a
generic Rust/model allowance. Unknown or expanded scopes remain held. Each new
path set is incomparable with every other profile, including SOT: neither a
subset nor a superset. The window model uses the existing packet input module
so that its seven-path scope is not a subset of the eight-path SOT scope.

## Qualification

All 90 controller tests passed locally without skips. Five new parameterized
regressions cover both independent scopes: trusted adoption including a genuinely
new layout model file, each changed identity, unlisted source and controls,
incomplete/expanded records and exactly bound combined-source rejection against
every other profile. All old controller/test function bodies and fifteen profile
objects were checked unchanged.

Maintenance originals
[CI41151975657](https://github.com/microsoft/jpeg2000_rs/actions/runs/41151975657)
and [Verus41151978268](https://github.com/microsoft/jpeg2000_rs/actions/runs/41151978268)
accepted that exact candidate: 347 Python tests without skips, 3,528 verified/
zero errors per root and actual package archive, MSRV 674 library plus six
example tests and three doctests, all 4,573 observed items unchanged and all
normal gates. These counts bind its earlier accepted source baseline, not a
future result. They are not a function-coverage measurement.

This documentation proposal needs its own full qualification and separate
Ready/green review. Each source implementation still requires pristine released
whole-root and byte-bound actual Cargo archive proof, all normal checks and
independent actual-source review. No source, lifetime-tool release or normative
qualification is inferred from this capability operation.

## Execution and rollback

After normal document adoption and fresh #200 actual-main acceptance:

1. Verify the required-original blobs on accepted current main.
2. Construct a NEW commit with that main as its sole parent and only the two
   exact replacements above. Preserve every registry entry and all integrated
   runtime/model/proof changes.
3. Qualify that exact result with full CI and released whole-root/actual-archive
   Verus, unchanged policies, original source identities and all current tests.
   A changed base needs fresh result qualification.
4. Recheck unchanged main and deploy by a normal fast-forward.
5. Mark this forward spent and accept fresh actual-main full validation before
   using either profile. Separate exact records, final source qualification,
   Ready policy checks, source reviews and guarded merges are still required.

If post-deployment validation fails, restore only the two required-original
blobs in a new normal commit, provided both files still have the exact authorized
replacements. Preserve all other files and validate actual main. Concurrent
changes require stopping rather than expanding this operation.
