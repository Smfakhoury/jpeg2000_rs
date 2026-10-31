# Exact deferred main-component and SOT profile operation

## Separate decisions and limits

The accompanying registry proposes only the exact component #198 source record.
This document separately proposes one deferred operation adding two independent
fixed producer profiles. Neither decision is active on this branch. Both require
complete checks, original-artifact qualification, a fresh ready policy check,
separate engineering review and normal adoption on trusted main.

The operation below must wait until #198 itself is finally refreshed, fully
qualified, separately source-reviewed, normally merged and accepted by fresh
actual-main validation. Record adoption alone does not meet that condition.
All earlier forward permissions, including #196 and #199, are spent.

No main-component or SOT source record is added here. This is not permission
for combined source amendments, different models, additional files, changed
toolchain/dependency pins, resource caps, fixture/oracle policy, publication,
visibility/history/tag changes or normative certification.

## Exact permitted operation

Replace only these regular files, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `1ce6bc12747839e766bbfb4c5e8ab44a21e786ed` | `c89bbc969173e6d4d7bb547cf8e06ceedba4f831` |
| `validation/tests/test_model_review_hold.py` | `36d8dcdbc8567ee5480305ff51036eee9aaf78e6` | `6e55976d832da89f9f301adef1643b30d5b0f05e` |

Replacement bytes are retained in inactive maintenance candidate
`599a8497dc7cda38bac22b3782e2c9249a4c45ec`. All thirteen existing profiles,
eight controller functions, previous tests, schema, exact-blob/regular-file
checks, trusted-main selection and registry data remain unchanged.

The separate nine-path `main-component-finalization` is:

- `src/codestream/layout.rs`
- `src/codestream/coding_style.rs`
- `src/codestream/quantization.rs`
- `src/codestream/proof_support.rs`
- `src/codestream/tests.rs`
- `specs/mod.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

Its source scope is the actual owner factories, quantization setter and
compatibility query, ordered main component-style/quantization finalizers and
their real layout calls; ten additive source-derived models and four closed
projections; complete result, owner, error-order, broadcast, tail and immutable
reader frames; and strict regressions. Shared factory bodies must remain one
executing implementation with existing public facades. Existing DFS/ADS
dependency meanings and raw producer semantics must remain unchanged.

The separate eight-path `sot-reader-marker` is:

- `src/codestream/tiles.rs`
- `src/codestream/reader.rs`
- `src/codestream/proof_support.rs`
- `src/codestream/tests.rs`
- `specs/mod.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

Its source scope is the actual SOT reader and three marker-admission predicates;
four additive source-derived models and two closed projections; exact raw
results, partial reader/work/position updates and unused-field frames; and
strict full-state/real-reader regressions. Preserve executing statements,
the existing standalone-marker model, original marker errors, and native
zero-length/count bypasses.

Each later source record must independently bind its entire exact profile and
all before/after identities. These are not a combined source profile or a
generic Rust/model allowance. Unknown or expanded scopes remain held.

## Qualification

All 85 controller tests passed locally. Five parameterized regressions cover
both independent scopes: trusted adoption, changed identities, unlisted source
and controls, incomplete/expanded records and combined-source rejection even
when each individual record exactly binds its final union. The initial local
fixture error was corrected without changing any existing test or controller
behavior, and its failed log remains retained.

Maintenance originals
[CI41142216033](https://github.com/microsoft/jpeg2000_rs/actions/runs/41142216033)
and [Verus41142219136](https://github.com/microsoft/jpeg2000_rs/actions/runs/41142219136)
accepted that exact candidate: 340 Python tests without skips, 3,519/0 per
root/archive replay, 666 library plus six example tests and three doctests on
the MSRV, all 4,560 observed items unchanged and all normal gates.
These counts bind its earlier accepted source baseline, not a future result.

This documentation proposal needs its own full qualification and separate
ready/green review. Each source implementation still requires pristine released
whole-root and byte-bound actual Cargo archive proof, all normal checks and
independent source review. No source, lifetime-tool release or normative
qualification is inferred from this capability operation.

## Execution and rollback

After normal document adoption and fresh #198 actual-main acceptance:

1. Verify the required-original blobs on accepted current main.
2. Construct a NEW commit with that main as its sole parent and only the two
   exact replacements above. Preserve every registry entry and all integrated
   runtime/model/proof changes.
3. Qualify that exact result with full CI and released whole-root/actual-archive
   Verus, unchanged policies, original source identities and all current tests.
   A changed base needs fresh result qualification.
4. Recheck unchanged main and deploy by a normal fast-forward.
5. Mark this forward spent and accept fresh actual-main full validation before
   using either profile. Separate exact records and final source qualification,
   ready policy checks, source reviews and guarded merges are still required.

If post-deployment validation fails, restore only the two required-original
blobs in a new normal commit, provided both files still have the exact
authorized replacements. Preserve all other files and validate actual main.
Concurrent changes require stopping rather than expanding this operation.
