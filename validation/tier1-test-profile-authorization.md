# One-time authorization for a fixed Tier-1 test-layout profile

## Owner decision

This proposes a new, single-use operational authorization. It is inactive on a
feature branch and becomes authority only after the repository owner adopts its
documentation-only PR with every required check green. The assistant must not
merge that policy PR. Earlier #166, #169 and #170 forward permissions are spent
and cannot authorize this different operation.

The purpose is to prepare the next bounded readability cleanup, not to approve
its source change. The nondefault Tier-1 experiment moves the existing private
tests out of the 15,501-line runtime-facing file, leaving 8,250 lines. Its complete
production/contract/proof prefix is byte-identical; test-module identity and
private access are unchanged.

## Exact permitted operation

Only these two regular files may be replaced, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `b6e0e8f0cb08293cd752b41e5ae0fa35127a5b28` | `12bf19c1f9c803e51121aceff8f6fd993f23199f` |
| `validation/tests/test_model_review_hold.py` | `e06acdb075e0663c9aefc8f4b153dc53e4cc080d` | `7b2195bc548b52e3e2cb8286652bd4f86bc0b482` |

Replacement bytes are retained in maintenance candidate
`ac4988cf631c3f35a4198e1a6cfd8fa25536909b`.

The controller adds only the `tier1-test-extraction` profile with this entire
protected path set:

- `src/tier1.rs`
- `src/tier1/tests.rs`
- `validation/normative-coverage.json`
- `validation/expected-package-files.txt`
- `validation/tests/test_framework.py`

It adds no generic module/controller/model override and no source record.
Existing profiles and rejection cases remain. A shared layout-fixture builder
supports the existing decode tests and four new cases: exact trusted adoption;
changed runtime/tests/inventory/bindings; extra models/modules/controller; and
forbidden paths inside the profile. All 57 focused controller/package tests pass.
No executable/model/proof Rust, manifest, workflow, tool/dependency pin,
verification setting or resource limit changes are part of this operation.

## Evidence required before review

The maintenance candidate's
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40772803806) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40772804248)
must finish successfully and pass independent original-artifact audits before
owner review is requested.

The separate refreshed source experiment is
`79c6f9aecd316e70fbcf323eda7fdbfd58c8c036`, based on accepted ghost-support main
`24dd11ed3f32ae899802ce019076cda5e61fbe48`. Require its fresh
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40772805265) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40772806908)
originals too. It preserves the accepted ghost support, updates exactly 119 test
references, retains all 666 compiled native test identities and ships the real
new test module in a 53-file/37-Rust archive. The only refresh conflict was
documentation; both the private-test and ghost-support explanations were kept.

Pending, skipped, missing or failed gates are not passes. Require native/MSRV/
resource/actual-consumer gates, actual pinned archive formatting and unchanged
input bindings. Whole-root obligations and observed function identities are
engineering observations, not exhaustive source coverage, lifetime verification
or independent normative correspondence.

## Mandatory execution and rollback

After actual owner adoption:

1. Read accepted current main and verify both required-original identities.
2. Prepare a NEW commit whose sole parent is that main and whose only changes
   are the two exact replacements above. Preserve every registry entry and all
   other source, documentation, manifests, locks, workflows, pins and caps.
3. Run existing full CI and released repository/archive Verus on that exact
   result tree. Independently audit original artifacts, function identities,
   tests, actual archive formatting/binding and thresholds. Fresh base-tree
   changes require fresh result qualification, not an earlier candidate's run.
4. Recheck that main has not moved, then use only a normal fast-forward update.
   No force push, status override, settings/ruleset change, disabled/skipped
   gate, assistant PR merge, source integration, visibility change, tag or
   publication is authorized. Unexpected state requires stopping and evaluating.
5. Accept fresh actual-main full CI/Verus before using the profile. Any Tier-1
   source exception still needs a separate green registry-only policy PR, owner
   adoption, source refresh and fresh complete checks before source review.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided these paths still contain the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

The forward permission is single-use and cannot authorize later controller
versions or integrate the Tier-1 experiment.
