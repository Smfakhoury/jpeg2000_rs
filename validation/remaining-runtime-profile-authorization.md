# Exact delegated authorization for the remaining runtime module profile

## Decision

The owner requested autonomous completion of publication preparation, including
the responsibility-based file-size reduction, separate engineering review of
ready/green PRs, and normal guarded merges. This proposes a new single-use
operational authorization. It is inactive on this feature branch and becomes
authority only after its fully green documentation-only PR is adopted on trusted
main under that delegation. Agent engineering review is not independent human,
normative or legal approval.

Earlier forward permissions, including #179, are spent. No publishing,
visibility/tag/history change, admin/status/settings override or skipped gate
is authorized.

## Exact permitted operation

Only these two regular files may be replaced, retaining mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `f8e2aa8f5b4abfd8ded291af528343756317f1c8` | `bc96700322c051a7dc45c0e5a7ebdc56284025ed` |
| `validation/tests/test_model_review_hold.py` | `89ea1199418d1c4fa782cc935453abde2838939f` | `ae8df0e88f379c2effdc3d8568043ef8b68a7570` |

Replacement bytes are retained in maintenance candidate
`450411dc9e0799bad2b88c25e593d7593e2708fd`.

The controller adds only `remaining-runtime-domain-layout`. Its entire,
literal 124-path `frozenset` is bound by the exact replacement blob above;
it contains only the ten remaining runtime facades, their explicitly named
responsibility/test/ghost-support files, and these four evidence inputs:

- `validation/expected-package-files.txt`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

The ten facades are `codestream`, `color`, `image`, `jp2`, `jpip`, `jpx`,
`packet`, `refinement`, `tier1` and `transform`. This is not a directory,
wildcard, unknown-module or generic codec override. Decoder modules, other
runtime files, normative/model/proof Rust under `specs/` and `proofs/`,
controllers, manifests, locks, workflows, pins and resource ceilings are
outside the profile. No source record is added by this operation.

All existing profiles, adopted records and rejection cases remain unchanged.
Four new tests require trusted adoption, reject every changed source/evidence
identity, reject nine extra-scope families, and reject profile-set expansion.
The complete controller module has 56 passing tests; the original focused
output, not an earlier maintenance-commit test-count typo, is authoritative.

## Qualification required before adoption

Independently accept the profile's
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/41024492454) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/41024495523)
originals, all complete checks on this documentation PR and its fresh ready
policy gate. Require a separate engineering review of the green PR and the
exact two-file candidate. Do not review or merge red, pending or missing gates.

The coherent source experiment is
`bebe34dc1b0eab628d8f7b56d087ef611c638884`, with
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/41024230240) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/41024233243).
Require the strict original-artifact audit, not a workflow conclusion alone.
Native evidence binds all 1,636 original declaration projections, including
four cfg-test helpers not observed by Verus. All 4,554 observed project
identities must match with only the exact 850 source-derived free-function
module moves and 27 same-token constant IDs (26 associated and one local).
Original canonical method and parent ghost identities remain.
This is not measured exhaustive source proof coverage or certification.

The first source proof correctly rejected implicit private-trait visibility
after a child-module move. The corrected source keeps both unchanged symbol
trait implementation blocks beside their original private Tier-1 trait.
It does not open formerly opaque specifications, change proof bodies, weaken
contracts, add trust, change mathematical models or relax any tool/budget pin.
The failed original is retained, not counted as qualification.

The source experiment's 122 ordinary runtime files have a maximum of 1,321
lines; tests and proof/model material remain separate. Existing public paths,
the one Cargo/Verus implementation, compiled native identities, error/resource
behavior and actual archive/consumer boundaries must remain intact.
This profile does not integrate or approve that source.

## Execution and rollback

After actual adoption on trusted main:

1. Read accepted current main and verify both required-original identities.
2. Prepare a NEW commit whose sole parent is that main and whose only changes
   are the two exact replacements above. Preserve every registry entry and all
   other files.
3. Run existing full CI and released whole-root and exact-archive Verus on that
   exact result tree. Independently audit original artifacts and thresholds;
   changed base trees require fresh result qualification.
4. Recheck unchanged main and use only a normal fast-forward update. No force,
   admin/status/settings override, skipped gate or expanded scope.
5. Accept fresh actual-main full CI/Verus before using the profile. A separate
   exact source-record PR must then be fully green, separately reviewed and
   normally adopted. Refresh the source from accepted main and require fresh
   complete checks, separate ready/green review, normal exact-head guarded
   merge and fresh actual-main acceptance.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided both paths still have the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

The forward permission is single-use and cannot integrate the module layout.
