# Trusted engineering amendment registry

This is policy data, not a proof or conformance certificate. The controller reads
only the copy from its trusted default-branch checkout, never a proposed branch.
An empty registry authorizes no protected changes. Records below are proposals
while this file is on a feature branch; only owner adoption on trusted `main`
gives them authority.

## Proposed engineering decisions

| Record | Exact scope | Original evidence |
|---|---|---|
| `proposal-160` | Move the private parser tests to their own file; executable/contract/proof prefix unchanged, same compiled tests, exact inventory bindings | #160, CI40608106332 / Verus40608106341 |
| `proposal-164` | Standalone `image` 0.25.10 consumer, its package-only CI step, source, tests and fixtures; no decoder API or implementation change | #164, CI40665820297 / Verus40665820275 |

This is not approval to merge either source PR. After record adoption, refresh
each source branch from accepted main and require fresh complete checks before
requesting source review. Exact bytes, not a feature name or author's assertion,
control eligibility. Neither record permits normative model/proof Rust or
controller changes, combined amendments, or changed pins/caps.

The package-distribution proposal #163 is deliberately not included. It shares
the package inventory with #160 and needs refreshed identities/evidence after
the first structural move is accepted. Do not silently reuse a stale record.

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
files and non-regular file modes fail closed.

```json
{
  "schema_version": 1,
  "amendments": [
    {
      "id": "proposal-160",
      "profile": "parser-test-extraction",
      "source_pr": 160,
      "source_head": "e929d74ae77ec277e55748012a13cbb48ac80bdd",
      "ci_run": 40608106332,
      "verus_run": 40608106341,
      "rationale": "Mechanical private-test relocation with unchanged executable/contract/proof prefix, compiled test identities, models, pins and resource policies.",
      "paths": {
        "src/codestream.rs": {
          "before": "d8c8066b191da8382ce10c2ca3eb6587d07a9b44",
          "after": "09f0b4369eabb7c05098d2aaa22041bfeadbe698"
        },
        "src/codestream/tests.rs": {
          "before": null,
          "after": "9317d9d81f43fb8d8d382e2a5733d8583789afc2"
        },
        "validation/expected-package-files.txt": {
          "before": "ef00ad4587c92a7c52365e5d6a1a0dec18f12005",
          "after": "026b291e83d7b551c092b5919886bc19b8b7b90e"
        },
        "validation/normative-coverage.json": {
          "before": "8053dd2c9ce66efdb4f39f75aa0e45539ccc7e9c",
          "after": "05237df10a29fd9e430180a0f7250da810db567f"
        },
        "validation/tests/test_framework.py": {
          "before": "28edc2ee48064bbb2693c44656705ab8934086fb",
          "after": "43da2a6f2e292bef0b9d411f772b59993f39403d"
        }
      }
    },
    {
      "id": "proposal-164",
      "profile": "image-consumer-probe",
      "source_pr": 164,
      "source_head": "bb5e67286d5c969ef23acba14e0f810193d74c33",
      "ci_run": 40665820297,
      "verus_run": 40665820275,
      "rationale": "Bounded standalone consumer of the actual package through image traits/hooks; no production decoder/model/API/pin/cap change or normative approval.",
      "paths": {
        ".github/workflows/ci.yml": {
          "before": "c4c373dc1604314d7588de438c33ed7182de7052",
          "after": "eaad74889b89bab5ca025282d4c53f9503d9e18c"
        },
        "validation/image-consumer-probe/.gitignore": {
          "before": null,
          "after": "b83d22266ac8aa2f8df2edef68082c789727841d"
        },
        "validation/image-consumer-probe/Cargo.lock": {
          "before": null,
          "after": "491fa432326017b6eb284354ebc9fbf595c5f56f"
        },
        "validation/image-consumer-probe/Cargo.toml": {
          "before": null,
          "after": "1735ac1955818412f4b4f266d080f0c418d137aa"
        },
        "validation/image-consumer-probe/src/decoder.rs": {
          "before": null,
          "after": "e8c36bb5ac7b5b12612fbc85163f44768e6eaa7c"
        },
        "validation/image-consumer-probe/src/lib.rs": {
          "before": null,
          "after": "40c4eff6e40c9b647ac4ec1e12a295cb956d8060"
        },
        "validation/image-consumer-probe/tests/adapter.rs": {
          "before": null,
          "after": "1eeb94542c3e000b2bc6631198437389eb8c2cea"
        },
        "validation/image-consumer-probe/tests/fixtures/gray.jp2": {
          "before": null,
          "after": "efc957bcd86ffef8f416081a790b1f64d6408bfb"
        },
        "validation/image-consumer-probe/tests/fixtures/rgb.jp2": {
          "before": null,
          "after": "b36f70e6d01316b056e30aa72aa05344f1181b55"
        }
      }
    }
  ]
}
```
