# One-time trusted gate bootstrap authorization

## Approval boundary

This is a proposed operational authorization, not executable configuration.
It has no authority from a feature branch, CI result, PR author or automated
continuation message. It becomes owner authorization only if the repository
owner explicitly adopts it by merging its documentation-only PR **after every
required check on that PR is green**. The assistant must not merge that PR.

Adoption authorizes only the bounded operation below. It does not merge a failing
PR, approve a model, authorize any source PR, change repository settings, or
approve registry entries. The initial [registry](reviewed-amendments.md) remains
empty and authorizes no protected changes.

## Why an operational exception is needed

The deployed controller protects its own code and has no amendment path.
Any controller-changing PR therefore remains red under that controller,
regardless of its technical results. Asking for such a PR to be reviewed or
merged was not a valid solution.

The repair reads exact, bounded amendment records only from trusted `main`.
Future records can be proposed through documentation-only PRs that pass CI before
review. Source proposals cannot authorize themselves by changing their own
registry. The controller and normative model/proof Rust remain protected.

## Authorized file identities

The operation may replace only these two regular files, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `7d2b21797598c9fcdd8884cfe88c0db7a42215a0` | `d347c2532b3eeb74014606d8fb4997ff56966305` |
| `validation/tests/test_model_review_hold.py` | `7348836e3774810e792f7a206e046ea593d661f5` | `399f9b675c4e38f315f05147b0a75c58ea3d130f` |

Replacement bytes are retained in repair candidate
`73eaf6effde11e81d650756d9cc3c339f7d3f419`. Its audited maintenance preflight used:

- [Full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40674161313):
  270 Python tests without skips, original Rust 1.88 tests, platform comparisons
  and resource gates passed.
- [Released whole-root Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40674162088):
  3,518 obligations, zero errors; observed function reporting is non-exhaustive.
- All 49 focused controller/collector cases passed, including every original
  blanket-hold negative and candidate-self-approval rejection.

Those `workflow_dispatch` runs are maintenance evidence, **not** a PR-policy pass
or bootstrap approval. Because adopting this document changes the base tree,
they must not be reused as qualification of a subsequently constructed commit.

## Mandatory execution conditions

1. Verify actual owner adoption of this document from a genuinely green PR.
2. Read current `main`; verify both original identities above and the unchanged,
   empty registry blob `6385a7dfb702013524efb8507785f0bc47732c89`.
   Unexpected state requires stopping, not broadening this authorization.
3. Prepare a commit whose parent is that accepted main and whose only changes
   are the two exact replacements above. Preserve this document, the registry,
   other documentation, all runtime/model/proof Rust, manifests/locks,
   toolchain/workflow pins and resource settings byte-for-byte.
4. Run the existing full CI and released whole-root Verus on that exact result
   tree. Independently validate the original artifacts, source identity,
   complete test execution and unchanged thresholds. Do not waive failed,
   pending, skipped or missing checks.
5. Recheck that `main` has not moved; perform only a normal fast-forward update
   to the validated commit. No force push, branch/ruleset change, status override,
   workflow disabling, CI-skip directive, source-PR merge, tag, visibility change
   or package publication is authorized.
6. Require fresh actual-main CI/Verus and original-artifact validation before
   using the new amendment flow. Keep existing source PRs draft until their own
   complete checks genuinely pass.

If main moves or a precondition fails, stop and re-evaluate the exact state;
do not overwrite unrelated changes or infer a broader permission.

## Bounded rollback

If post-deployment validation fails, this authorization also permits restoring
only the two required-original blobs in a new, non-force commit, provided those
paths still contain the exact authorized replacements. Preserve every other
file and validate the resulting main again. Unexpected concurrent changes
require stopping. This restores the original conservative freeze, not a bypass.

The forward authorization is single-use; it cannot authorize later controller
versions. Once the repair is accepted, the old controller-update PRs may be
closed as superseded, not merged with failing checks. New engineering records
still need separate green documentation-only PRs and owner adoption; source
branches then need fresh complete checks before source review is requested.
