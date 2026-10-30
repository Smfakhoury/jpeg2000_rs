# Exact delegated authorization for two independent tile-producer profiles

## Decision and limits

The owner delegates autonomous engineering qualification, separate review of
ready/green proposals and normal guarded merges. This proposes a new single-use
operational authorization for the measured duplicate-cache correction under
#190 and the next actual PLT/PPT producer milestone under #133. It is inactive
on a feature branch and becomes authority only after its
fully green documentation-only PR is separately reviewed and normally adopted
on trusted main.

All earlier forward permissions, including #192, remain spent. This operation
approves no source record, normative correspondence, document use, publication,
visibility/history/tag change, force push or administrative/status/settings
bypass. Engineering agent review is not independent human certification.

## Exact permitted operation

Only these regular files may be replaced, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `cafe1f2b3b05f57c6e3ad12706c0fe2d551cfb81` | `a69de44c2e000a5685e95affb0a1763ff2a554d5` |
| `validation/tests/test_model_review_hold.py` | `7abda2279ca3dc05a2357573f71d6f18057ab94d` | `97d84c9214ada27a16fd48d07690df82e98e322f` |

Replacement bytes are retained in maintenance candidate
`39292acb54fd863e9671e05c6fc71fe9df55c2f6`. The controller adds only two
independent, literal profiles. The three-path `tile-component-style-cache` is:

- `src/codestream/layout.rs`
- `src/codestream/tiles.rs`
- `src/codestream/tests.rs`

Its source scope is the replacement of duplicated per-tile component-style
snapshots by indices into retained payload owners, the actual successful
payload identity and inductive latest-owner/absence proofs, and native
persistence/error regressions.

The separate seven-path `tile-packet-admission` is:

- `src/codestream/tiles.rs`
- `src/codestream/packet_headers.rs`
- `src/codestream/tests.rs`
- `specs/packet_input.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

Its source scope is extraction of the real inline PLT/PPT admission into actual
executed producers, complete source-derived result/reader/partial-counter frame
contracts and models, and strict native/inventory/mutation regressions. Existing
model meanings and runtime error/commit order must remain unchanged.

All nine existing profiles, adopted registry
records, schema, exact-blob/regular-file admission and trusted-main selection
remain unchanged. Nine added regressions require trusted adoption and reject
each changed identity, incomplete/expanded scopes and extra models, limits or
controls. They also explicitly reject combining the two source records into one
source amendment. All 70 controller tests passed locally. This batches only
capability deployment, not source approval or a generic combined override.

No source record is added here. Other than the explicitly listed source-derived model additions, all other
runtime/model/proof files, manifests,
locks, workflows, verifier/dependency pins, collector policy, resource
thresholds, fixture selection and oracle/numeric tolerances remain outside
the profile. The profile cannot approve itself or a combined source amendment.

## Evidence and required qualification

Before adoption, independently accept the current exact candidate's fresh
[CI41109829211](https://github.com/microsoft/jpeg2000_rs/actions/runs/41109829211)
and [Verus41109829361](https://github.com/microsoft/jpeg2000_rs/actions/runs/41109829361)
originals on its accepted packet-source baseline. These preflights are pending
when this proposal is written; no success or source approval is inferred.

The superseded cache-only maintenance candidate
`cc400a097d49eaa9f3ff38c4f9c380f378e60ce5` passed
[CI41096477654](https://github.com/microsoft/jpeg2000_rs/actions/runs/41096477654)
and [Verus41096477622](https://github.com/microsoft/jpeg2000_rs/actions/runs/41096477622)
strict original audits: 319 Python tests without skips, 3,518/0 per source and
actual-archive replay, and all existing native/MSRV/numerical/HT/consumer/
resource gates. Those results bind that original candidate and its then-current
runtime tree; they are not qualification of the current two-profile candidate,
a later result commit or changed base.

The original source experiment is
`c9ba5f1eb55784359cb83f1b1fd6da334f4bfdbe` in draft #195. Its full original
CI41096490494 and Verus41096490742 passed 315 Python tests without skips,
662 library plus six example tests and three doctests on the MSRV, 3,518/0 per
root/archive replay, all 4,554 prior observed items and all normal gates.
Hosted selected/full RSS was 0.5434326479227864 under the unchanged 0.75
threshold; all 65 resource/accounting records passed. Source-function coverage
remains not measured. These are historical experiment results, not approval of
a refreshed source or whole-resource-envelope guarantee.

Before adoption, require every check on this documentation PR to be genuinely
complete/successful, strict original-artifact audits, a fresh ready policy
check and a separate engineering review of this document and both exact
replacement blobs. Do not waive failed, pending, missing or skipped gates.
Original failed measurements remain retained; no infrastructure-flake cause
or universal RSS bound is inferred.

## Execution and rollback

After normal adoption on trusted main:

1. Read accepted current main and verify both required-original identities.
2. Prepare a NEW commit whose sole parent is that main and whose only changes
   are the two exact replacements above. Preserve every registry entry and all
   other files.
3. Qualify that exact result tree with full CI and released whole-root and
   byte-bound archive Verus, including original logs, source identities,
   unchanged policies and all current tests. Changed bases need fresh result
   qualification; original maintenance counts cannot be relabeled as new ones.
4. Recheck unchanged main and deploy only by a normal fast-forward update.
5. Accept fresh actual-main full CI/Verus before using the profile. Separately
   adopt an exact source-record proposal only after its full green checks and
   engineering review. Refresh source from accepted main; require fresh whole
   source/archive qualification, ready/green source review, guarded source merge
   and actual-main acceptance before integration is complete.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided both paths still hold the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

The forward permission is single-use and cannot itself integrate #195 or
approve a PLT/PPT source proposal.
