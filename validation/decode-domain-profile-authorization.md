# Exact delegated authorization for the decoder-domain layout profile

## Decision

The owner requested the JBIG2-style responsibility-based source layout and
explicitly delegated engineering review and normal PR merge, with a separate
review once PRs are green. This proposes a new single-use operational
authorization. It is inactive on this feature branch and becomes authority only
after a fully green documentation-only PR is adopted on trusted main under that
delegation. Record delegated agent review as such, not as independent human or
normative approval.

Earlier forward permissions, including #176, are spent. No publication,
visibility/tag/history change, admin/status override or skipped gate is
authorized.

## Exact permitted operation

Only these two regular files may be replaced, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `62c64100a3cf1c4fee3d5e5afc920f09b6c231e7` | `f8e2aa8f5b4abfd8ded291af528343756317f1c8` |
| `validation/tests/test_model_review_hold.py` | `0c7aa72b1123c6a78e3077db1607106a4670d9fe` | `89ea1199418d1c4fa782cc935453abde2838939f` |

Replacement bytes are retained in maintenance candidate
`fdcd0768d57aa62986fc554fba7c3c8977a4a8d8`.

The controller adds only `decode-domain-layout`, with this entire fixed set:

- `src/decode.rs`
- `src/decode/api.rs`
- `src/decode/arithmetic.rs`
- `src/decode/code_blocks.rs`
- `src/decode/execution.rs`
- `src/decode/geometry.rs`
- `src/decode/output.rs`
- `src/decode/planning.rs`
- `src/decode/proof_support.rs`
- `src/decode/reconstruction.rs`
- `src/decode/selection.rs`
- `src/decode/tests.rs`
- `validation/expected-package-files.txt`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

These are exactly the 16 protected paths of one coherent decoder-layout change,
not a generic module, codec, model or controller override. Existing profiles,
all six adopted records and rejection cases remain unchanged. Four new tests
cover trusted-only adoption, each changed path identity, extra model/module/
controller changes and forbidden profile expansion. All 66 focused controller/
package tests pass. No executing/model/proof Rust or tool/workflow/pin/cap changes
are part of this two-file operation. It adds no source record.

## Evidence required before adoption

Independently accept the profile's
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40981417923) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40981417986)
original artifacts, all checks on this documentation PR and its fresh ready
policy gate. Require a separate engineering review of the green PR and exact
two-file candidate before adoption.

The source experiment `39431e36cb8723365e951a2db203c4e7256d2a50` is already
qualified from [full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40978411701)
and [repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40978411630)
originals: 300 Python tests without skips, 3,518/0 per proof replay, 63 shipped
files and 47 byte-bound Rust files. The 367-line facade and nine real runtime
modules of 98-1,248 lines preserve all 146 executing declaration contracts/body
tokens, shared type/field tokens and all 666 native test identities. Only the
exact 124 free-function module namespaces move; root ghost-helper namespaces
and all other observed identities remain intact. Private helpers become
parent-visible where required; optional parameter trailing commas follow pinned
formatting. No function result, model, contract, budget or error ordering changes.

The first source CI correctly rejected six mutation fixtures still selecting any
call in a former monolith. Only the shared fixture's caller selection was fixed;
the stronger actual named-body check and every negative control remain. The
fresh corrected-head runs above, not the failed first CI, are qualification.

This is engineering evidence, not exhaustive source proof coverage, lifetime
verification or independent normative correspondence. The source remains
unintegrated and needs its own separate exact adopted record and green PR.

## Execution and rollback

After actual adoption on trusted main:

1. Read accepted current main and verify both required-original identities.
2. Prepare a NEW commit whose sole parent is that main and whose only changes
   are the two exact replacements above. Preserve every registry entry and all
   other source, documentation, manifests, locks, workflows, pins and caps.
3. Run existing full CI and released repository/archive Verus on that exact
   result tree. Independently audit original artifacts and thresholds; changed
   base trees require fresh result qualification.
4. Recheck unchanged main and use only a normal fast-forward update. No force,
   settings/status/admin override, skipped gate or expanded scope.
5. Accept fresh actual-main full CI/Verus before using the profile. A separate
   green exact source-record PR must be adopted, then the source refreshed and
   fully qualified again before its separate review and normal merge.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided the paths still have the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

This forward permission is single-use and cannot integrate the layout itself.
