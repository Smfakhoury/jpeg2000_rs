# Trusted engineering amendment registry

This is policy data, not a proof or conformance certificate. The controller reads
only the copy from its trusted default-branch checkout, never a proposed branch.
An empty registry authorizes no protected changes. Records below are proposals
while this file is on a feature branch; only owner adoption on trusted `main`
gives them authority.

## Engineering decisions and proposed addition

| Record | Status and exact scope | Original evidence |
|---|---|---|
| `proposal-160` | Adopted through #167; the private parser-test relocation is now owner-merged in #160 | CI40608106332 / Verus40608106341; refreshed source qualification is retained in #160 |
| `proposal-164` | Adopted through #167; the standalone package-only `image` consumer is now owner-merged in #164 | CI40665820297 / Verus40665820275; refreshed source qualification is retained in #164 |
| `proposal-163-b4c6d67` | Proposed exact source-inclusive package manifest/inventory, archive-binding checker/tests and Verus workflow; no Rust body change | #163, CI40713472934 / Verus40713472868 |

Record adoption is gate eligibility, not permission to merge a source PR.
For the new #163 record, refresh the source branch from accepted main after
adoption and require fresh complete checks before requesting source review.
Exact bytes, not a feature name or author's assertion, control eligibility.
No record permits normative model/proof Rust or controller changes, combined
amendments, or changed verifier/dependency pins and resource caps.

### Package-distribution choice

The proposed #163 record chooses a source-complete archive rather than the
current lean package. Include the existing verification Rust sources and
technical reference so the actual unpacked crate supports the project's
pinned formatter and same-source Verus replay. Do not use formatter skips,
dummy specifications or a second decoder.

The refreshed archive has 51 files, including all 35 runtime/model/proof Rust
files byte-identical to accepted main. Its compressed size is about 901 KiB
instead of the current 659 KiB. Ordinary builds still have zero default runtime
dependencies and do not enable ghost modules. No PDF, validation tooling,
default feature, decoder API or Rust implementation changes are included;
`publish = false` remains in force.

The source candidate was refreshed after both #160 and #164 landed, including
the new private-test file in Cargo traversal order. The record below therefore
uses the current original/proposed inventory identities, not the stale
pre-relocation proposal. Original evidence includes actual archive formatting
with Rust 1.93.1, repository and archive Verus 3,518/0 each, unchanged input
bindings, MSRV 660+3 from the retained archive and the real consumer's 37 tests
plus doctest. These checks are engineering evidence, not normative certification.

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

Only bounded engineering path sets declared by the trusted controller are
supported. The existing records cover parser-test extraction, the standalone
image consumer and source-inclusive packaging. A proposed
[decode-support profile authorization](decode-support-profile-authorization.md)
does not activate its new scope or add a source exception by changing this
documentation. Normative model/proof targets and controller amendments remain
outside the source-record profiles.
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
    },
    {
      "id": "proposal-163-b4c6d67",
      "profile": "source-complete-package",
      "source_pr": 163,
      "source_head": "b4c6d67af550c1ad69274f5d0f3db57b9d4b4cdc",
      "ci_run": 40713472934,
      "verus_run": 40713472868,
      "rationale": "Source-inclusive package preserves every Rust body and default dependency while requiring actual unpacked formatting, input binding and whole-crate proof replay; not normative approval or publication.",
      "paths": {
        ".github/workflows/verus.yml": {
          "before": "5f9cf1c43fae6163438514052162a743169d1621",
          "after": "11772c4f5f30a1f39dec8214a5a04741cd14782c"
        },
        "Cargo.toml": {
          "before": "b23f0d048482d5c3ab6e2767e0ca45a971ee2d95",
          "after": "d03202f88351757c86268f1f24102e21d0c90410"
        },
        "validation/check-packaged-source.py": {
          "before": null,
          "after": "80ea00883050923503738b845087122c6f6b4822"
        },
        "validation/expected-package-files.txt": {
          "before": "026b291e83d7b551c092b5919886bc19b8b7b90e",
          "after": "208d745650df83c5531c01f5ec7632f065da72e2"
        },
        "validation/tests/test_packaged_source.py": {
          "before": null,
          "after": "e48ca2c8f14499be0c40c00c31b5aa189544053c"
        }
      }
    }
  ]
}
```
