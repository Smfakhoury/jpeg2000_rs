# Trusted engineering amendment registry

This is policy data, not a proof or conformance certificate. The controller reads
only the copy from its trusted default-branch checkout, never a proposed branch.
The initial empty registry authorizes no protected changes.

An exception must first be proposed in a separate documentation-only PR. All
required checks on that PR must pass before review is requested. Owner adoption
of an exact record on `main` authorizes only those original/proposed regular-file
Git blob identities and that entire protected path set. Refresh the source PR
from the accepted main, run every required check again, and request source review
only when all checks genuinely pass. Combining a proposed record with its source
change does not authorize either change.

Records have `id`, `profile`, `source_pr`, `source_head`, `ci_run`, `verus_run`,
`rationale`, and `paths`. Each path maps to `before` (a blob ID or `null` for an
absent original) and `after` (a different blob ID). Evidence identifiers provide
an audit path; their presence does not certify the evidence. Owner review must
check the original source-bound results and the mechanical/behavioral argument.

Only the three known engineering profiles in the controller are supported:
parser-test extraction, the standalone image consumer, and source-inclusive
packaging. None permits normative model/proof Rust or controller amendments.
Unknown fields/profiles, missing history, malformed trusted data, extra protected
files and non-regular file modes fail closed. No record is currently adopted.

```json
{
  "schema_version": 1,
  "amendments": []
}
```
