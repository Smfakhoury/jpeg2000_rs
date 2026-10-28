# One-time recovery authorization for decode-support preparation

## What the owner is deciding

This proposes a new, single-use operational authorization. It is inactive on a
feature branch and becomes authority only after the repository owner adopts its
documentation-only PR with every required check green. The assistant must not
merge that policy PR.

The earlier #169 forward permission is spent. Its exact two-file operation
passed preflight, but mandatory actual-main
[Verus run 40742135085](https://github.com/microsoft/jpeg2000_rs/actions/runs/40742135085)
failed while installing the unchanged Rust 1.93.1 formatter. Rustup reported a
partially installed toolchain and a `bin/cargo-clippy` component conflict; the
archive replay did not run. That was not an accepted deployment.

The authorized rollback restored only the original controller/test blobs at
`69fe0bce9024ab5a4cb6556e3ebdf91134324148`. Its complete tree equals owner-adopted
`ab58b2039449c22415204638278c94318c1da3b5`. The decode-support profile is not
active, the three source records are unchanged, and the layout is not integrated.

## Exact permitted operation

Only these three regular files may be replaced, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `.github/workflows/verus.yml` | `11772c4f5f30a1f39dec8214a5a04741cd14782c` | `205b94b59176f66c4750d8806e60668768680872` |
| `validation/check-model-review-hold.py` | `d347c2532b3eeb74014606d8fb4997ff56966305` | `b6e0e8f0cb08293cd752b41e5ae0fa35127a5b28` |
| `validation/tests/test_model_review_hold.py` | `399f9b675c4e38f315f05147b0a75c58ea3d130f` | `e06acdb075e0663c9aefc8f4b153dc53e4cc080d` |

Replacement bytes are retained in maintenance candidate
`c884b0df6c885e9a3ac5d38a5bff25832c9d9265`.

The workflow installs and runs the same Rust 1.93.1 formatter in a dedicated
`RUNNER_TEMP/jpeg2000-archive-formatter` Rustup home instead of repairing the
runner's shared toolchain. The verifier still uses its existing Rust 1.98.1
toolchain and pinned Verus release. No formatter check, archive replay, collector,
tool/dependency pin, verification setting, threshold or resource limit is removed
or weakened. The origin of the runner's partial installation is not established.

The controller/test replacements are byte-identical to the earlier tested
proposal. They add only the `decode-proof-support` profile, with these four
protected paths:

- `src/decode.rs`
- `src/decode/proof_support.rs`
- `validation/normative-coverage.json`
- `validation/expected-package-files.txt`

The profile still requires a separately owner-adopted record for exact
original/proposed file identities and the entire protected path set. This
operation adds no record and permits no source-PR merge, other module,
normative-model change, controller override or generic engineering exception.
All previous rejection cases remain; 53 focused controller/package tests pass.

## Evidence required before review and execution

Before requesting owner review, accept complete source-bound original results
from the restored actual main's
[CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40743360147) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40743359828),
and the maintenance candidate's
[CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40743922290) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40743919964).
Require the actual isolated formatter installation and archive execution,
unchanged input bindings, native/MSRV/consumer/resource gates and all 4,554
observed project function identities. Pending, skipped or failed checks are not
passes.

The separate layout experiment remains
`233574e0392a7b460c6732d597008eca039f3971`. Its
[CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40730904780) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40729319509)
are engineering evidence, not source approval, exhaustive proof coverage,
lifetime verification or independent normative correspondence.

## Mandatory execution and rollback

After actual owner adoption of this document:

1. Read accepted current main and verify all three required-original identities.
2. Prepare a new commit whose parent is that main and whose only changes are the
   three exact replacements above. Preserve every registry entry and all other
   source, documentation, manifests, locks, pins and resource settings.
3. Run existing full CI and released repository/archive Verus on that exact
   result tree. Independently audit original artifacts, function identities,
   tests, isolated pinned formatting, archive binding and thresholds. Fresh
   base-tree changes require fresh result qualification.
4. Recheck that main has not moved, then use only a normal fast-forward update.
   No force push, status override, settings/ruleset change, disabled or skipped
   gate, source-PR merge, visibility change, tag or publication is authorized.
   Unexpected state requires stopping and re-evaluating.
5. Accept fresh actual-main full CI/Verus before using the profile. A source
   exception still requires a separate green registry-only policy PR, owner
   adoption, source refresh and fresh complete checks before source review.

If post-deployment validation fails, restore only the three required-original
blobs in a new non-force commit, provided those paths still contain the exact
authorized replacements. Preserve every other file and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

The forward permission is single-use. It cannot reuse #169, authorize later
controller/workflow versions or integrate the layout experiment.
