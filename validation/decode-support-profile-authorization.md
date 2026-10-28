# One-time authorization for the bounded decode-support profile

## Owner decision

This is a proposed operational authorization. It has no authority from a feature
branch, CI result, PR author or automated continuation message. It becomes
authorization only after the repository owner adopts its documentation-only PR
with every required check green. The assistant must not merge that policy PR.

This is a new, single-use authorization. The earlier #166 bootstrap authorization
is spent and cannot authorize these different file identities.

## Exact permitted scope

The proposed controller adds only the `decode-proof-support` profile, whose entire
protected path set is:

- `src/decode.rs`
- `src/decode/proof_support.rs`
- `validation/normative-coverage.json`
- `validation/expected-package-files.txt`

It does not authorize another module, the controller, normative model/proof
targets, combined amendments, or generic source changes. The controller still
requires a separately owner-adopted exact record binding original/proposed
regular-file Git blob identities. Adding the profile alone approves no source
exception and changes no registry entry.

Only these two regular files may be replaced, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `d347c2532b3eeb74014606d8fb4997ff56966305` | `b6e0e8f0cb08293cd752b41e5ae0fa35127a5b28` |
| `validation/tests/test_model_review_hold.py` | `399f9b675c4e38f315f05147b0a75c58ea3d130f` | `e06acdb075e0663c9aefc8f4b153dc53e4cc080d` |

Replacement bytes are retained in candidate
`c9c8bb32a7a0f8d89dfc9bb4f7b661187bc89312`. All previous rejection cases remain;
four new cases check exact trusted adoption, changed runtime/proof content,
extra models and forbidden module/controller paths. All 53 focused tests pass.

## Motivation and required evidence

The isolated layout experiment `233574e0392a7b460c6732d597008eca039f3971`
moves an existing 132-line ghost proof/view block into a cfg-guarded include in
the same logical `decode` module. Body bytes, open/closed declarations and
visibility are unchanged; recombination reproduces the original file exactly.
It adds no executable decoder alternative or normative-model changes.

The real repository and archive verifier run
[40729319509](https://github.com/microsoft/jpeg2000_rs/actions/runs/40729319509)
reported 3,518/0 each and retained all 4,554 observed project function identities,
including the moved proof lemma at its original path. That is not exhaustive
source coverage, lifetime verification or normative correspondence.

Before any activation, require complete successful and source-bound original
results for both the layout's
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40730904780) and
the control candidate's [full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40732899661)
and [repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40732897983).
Pending or queued platforms are not passes. These non-PR preflights are evidence
only, not policy/source approval.

## Mandatory execution and rollback

After actual owner adoption of this document:

1. Read accepted current main and verify both required-original identities.
2. Prepare a new commit whose parent is that main and whose only changes are the
   two exact replacements above. Preserve every registry entry and all other
   source, documentation, manifests/locks, toolchain/workflow pins and caps.
3. Run existing full CI and released repository/archive Verus on that exact
   result tree. Independently validate original artifacts, function identities,
   tests, archive binding/formatting and thresholds. Fresh base-tree changes
   require fresh result qualification, not reuse of an earlier candidate's run.
4. Recheck that main has not moved, then use only a normal fast-forward update.
   No force push, status override, settings/ruleset change, workflow disabling,
   skipped gate, source-PR merge, visibility change, tag or publication is
   authorized. Unexpected state requires stopping and re-evaluating.
5. Accept fresh actual-main CI/Verus before using the profile. Any source
   exception still requires a separate green registry-only policy PR, owner
   adoption, source refresh and fresh complete checks before source review.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided the paths still contain the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

The forward permission is single-use and cannot authorize later controller
versions or integrate the layout experiment.
