# Exact delegated authorization for packet-metadata proof development

## Owner decision and limits

On 2026-10-30 at 15:24 UTC, the owner explicitly authorized bounded model/proof
development and autonomous continuation of the implementation plan. Existing
delegation permits separate agent engineering review of genuinely ready,
all-green proposals and normal guarded merges without repeated owner PR
assessments.

This proposes one new single-use operational authorization. It is inactive on
a feature branch and becomes authority only after its fully green,
documentation-only PR is separately reviewed and adopted on trusted main under
that delegation. Engineering agent review is not independent normative,
human certification or document-use approval. Earlier forward authorizations
remain spent.

## Exact permitted operation

Only these two regular files may be replaced, retaining mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `bc96700322c051a7dc45c0e5a7ebdc56284025ed` | `85297c4fc897fec8b3d305f18166b2a40bfd8738` |
| `validation/tests/test_model_review_hold.py` | `ae8df0e88f379c2effdc3d8568043ef8b68a7570` | `b2fe168b3cbfdf5e1d8c96aad8d895ac7444d897` |

Replacement bytes are retained in maintenance candidate
`f3239620550d359131a02bf61e3b8606b3876ddf`. The controller adds only the literal
seven-path `packet-metadata-finalization` profile:

- `src/codestream/packet_headers.rs`
- `src/codestream/proof_support.rs`
- `src/codestream/tests.rs`
- `specs/packet_input.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

This is not a directory, wildcard or generic Rust/model exception. The scope
is the actual raw packet-length sum and length/packed-header finalizers, their
source-derived result models and outcomes, native regressions, and strict
inventory/contract mutation bindings. Existing native behavior and existing
model meanings must remain unchanged. Models describe the actual executing
implementation; their independent correspondence with selected standards
editions remains unreviewed.

All eight existing profiles, adopted records, registry schema, exact-blob and
regular-file checks, trusted-main selection and rejection logic remain
unchanged. Five added tests require exact trusted adoption, reject each altered
identity, extra models/controls, incomplete or expanded path sets, and
candidate self-approval. All 61 controller regressions passed locally.

This operation adds no source record. Other models/proofs, tile inheritance,
layout/public-output composition, controllers, manifests, locks, workflows,
verifier/dependency pins, collector policy, resource caps and tolerances remain
outside the profile. No PDF processing, standards-rights decision, official
conformance claim, registry upload, visibility/history/tag operation, force
push or administrative/status/settings bypass is authorized.

## Qualification and adoption

Before review or adoption, independently accept the maintenance candidate's
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/41081055146) and
[repository/actual-archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/41081054826)
original artifacts, source identities and unchanged limits. Workflow-dispatch
success is technical evidence, not a PR-policy pass or source approval.

Require every check on this documentation PR to be genuinely complete and
successful, a fresh ready policy check, and a separate engineering review of
this authorization and both exact replacement blobs. Pending, missing, skipped
or failed checks cannot be waived or presented as a ready/green handoff.

## Execution and rollback

After actual adoption on trusted main:

1. Read accepted current main and verify both required-original identities.
2. Prepare a NEW commit whose sole parent is that main and whose only changes
   are the two exact replacements above. Preserve every registry entry and all
   other files.
3. Run existing full CI and released whole-root and exact-archive Verus on that
   exact result tree. Independently audit original artifacts and thresholds;
   a changed base requires fresh qualification.
4. Recheck unchanged main and deploy only by a normal fast-forward update.
5. Accept fresh actual-main full CI/Verus before using the profile. A separate
   exact source-record PR must then be fully green, separately reviewed and
   normally adopted. Refresh the source from accepted main and require fresh
   complete checks, separate ready/green review, normal exact-head guarded
   merge and fresh actual-main acceptance.

The source record must bind every changed protected blob. Its engineering
review must check actual raw-input production, error precedence, mutable and
borrow frames, verified recursive definitions, existing-model preservation,
single-source execution, negative controls and unchanged trust/termination
boundaries. Passing Verus is not independent standards correspondence.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided both paths still have the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

The forward permission is single-use. It cannot itself integrate packet
finalization, complete #133, or clear the separate #149/#187/#188 decisions.
