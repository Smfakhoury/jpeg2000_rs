# Exact main-header admission profile decision

This documentation-only decision defines a conditional review-control operation
for [#213](https://github.com/microsoft/jpeg2000_rs/issues/213). It does not
itself supply the required explicit owner authorization or deploy a controller.
The existing seventeen profiles and seventeen trusted source records remain
unchanged by this document.

## Exact proposal

The technical prototype is `d7bb88f25f5ae984f8fcc48bd82459c6e51ff48e`,
based on accepted main `bac5ed3ac0042a5ace060f3b7c8d074b38f4858f`.
It changes only these existing regular `100644` files:

| File | Before Git blob | After Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `0dd06ce928e91d06abd47891577bdb217fef521b` | `7c84ca123b4663526d6e526f12d7e75ee0241dd0` |
| `validation/tests/test_model_review_hold.py` | `71613c7da2eb0f13edddadb053afc1b02b49571d` | `36fcb8513fb39b95b7e73a6f23684517a6caca3c` |

The sole added profile, `main-header-admission`, contains exactly:

- `src/codestream/layout.rs`
- `src/codestream/reader.rs`
- `src/codestream/proof_support.rs`
- `src/codestream/tests.rs`
- `specs/layout_input.rs`
- `validation/framework.py`
- `validation/normative-coverage.json`
- `validation/tests/test_framework.py`

All seventeen existing profile objects are identical. Outside the profile-data
assignment, the controller AST is identical. The new eight-path set is neither
a subset nor a superset of any existing profile. The controller still requires
one complete trusted exact record; candidate records, subsets, unions and
author-controlled approval flags do not provide authorization.

The proposed controller continues to hold both its own two-file amendment and
the eight source paths of draft
[#212](https://github.com/microsoft/jpeg2000_rs/pull/212). No source record is
added by the prototype. Its regression suite adds explicit candidate-record
self-approval rejection and exercises the new profile with the existing
trusted-adoption, identity, scope, combination and existing-file checks.

## Technical evidence and limits

Prototype CI `41432205363` and Verus `41432209020` are technical-only preflight,
not deployment or source approval. They qualify the exact prototype; their
results are not transferred to a later constructed result. The ordinary source
and actual-package verification policy, compiler/verifier pins and all resource
caps remain unchanged.

Draft source #212, `ec592a52185c6b3d6562d62ccc45da2c79e1b735`, has separate
held technical CI `41431553462` and Verus `41431553537`. Its scope derives the
actual raw main-header segment admission Result and reader/counter frame;
it does not establish complete header traversal, public image decoding,
lifetime analysis, normative correspondence or publication readiness.

## Conditions for one exact operation

Every condition below is required:

1. The owner explicitly authorizes this exact one-use two-file operation in
   #213 or an equally explicit recorded owner decision. Agent-authored issue
   text, restored write access, or adoption of this document is not that
   authorization.
2. This documentation decision receives its own complete gates, separate
   independent review, normal guarded merge and fresh actual-main acceptance.
   The accepted SOC/SIZ source and its accepted ancestors remain intact.
3. Only afterward, construct a **new sole-parent result** on the then-current
   accepted main. Its complete diff is exactly the two regular-file transitions
   in the table. The parent's two before blobs and result's two after blobs
   must match exactly. Do not reuse the prototype as a deployment result.
4. The new exact result receives complete original CI and source/actual-package
   Verus preflight. Existing tests and old profile meanings remain preserved.
   A failed or partial run, or a prototype run, cannot substitute for this.
5. Apply that exact result once using a normal non-forced fast-forward from its
   accepted sole parent. Do not change settings, statuses, branch rules,
   workflows, selectors, source/model files, the registry, pins or limits.
   Any changed parent, scope, file mode or blob identity stops the operation.
6. Require fresh actual-main CI and source/actual-package Verus acceptance.
   Retain the operation's identities and originals as a spent one-use receipt.

Until every prerequisite is satisfied, the operation remains deferred. The
previous #204 operation is already spent and provides no authorization here.
This procedure is not a general permission for controller edits or future
profile additions.

## Separate source adoption

Even a completed profile operation only adds a possible exact record shape.
The seventeen existing registry records remain unchanged and source #212
remains held until a separate complete eight-path record is independently
reviewed, normally adopted and accepted on actual main.

Then refresh source ancestry while preserving the recorded source blobs;
require fresh source gates and Ready hold, separate actual-source review,
normal exact-head guarded merge and fresh actual-main acceptance. Neither this
decision nor the profile operation approves that source automatically.

No crate upload, tag, repository visibility/history operation, normative
adjudication, lifetime qualification or publication completion is authorized.
