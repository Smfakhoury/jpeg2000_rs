# Trusted engineering amendment registry

This is policy data, not a proof or conformance certificate. The controller reads
only the copy from its trusted default-branch checkout, never a proposed branch.
An empty registry authorizes no protected changes. New or changed records are
proposals while this file is on a feature branch; only owner adoption on trusted
`main` gives them authority. Unchanged adopted records retain their history.

## Engineering decisions and proposed addition

| Record | Status and exact scope | Original evidence |
|---|---|---|
| `proposal-160` | Adopted through #167; the private parser-test relocation is now owner-merged in #160 | CI40608106332 / Verus40608106341; refreshed source qualification is retained in #160 |
| `proposal-164` | Adopted through #167; the standalone package-only `image` consumer is now owner-merged in #164 | CI40665820297 / Verus40665820275; refreshed source qualification is retained in #164 |
| `proposal-163-b4c6d67` | Adopted through #168; the source-complete package is now owner-merged in #163 | CI40713472934 / Verus40713472868; refreshed source and actual-main qualification are retained in #163 |
| `proposal-171-ghost-support` | Adopted through #172; the mechanical proof/view relocation is now owner-merged in #171 | CI40730904780 / Verus40729319509; refreshed source and actual-main qualification are retained in #171 |
| `proposal-174-tier1-tests` | Proposed exact private Tier-1 test relocation; complete production/contract/proof prefix unchanged | #174, CI40772805265 / Verus40772806908; refreshed source checks remain mandatory |

Record adoption is gate eligibility, not permission to merge a source PR.
For every future record, refresh the source branch from accepted main after
adoption and require fresh complete checks before requesting source review.
Exact bytes, not a feature name or author's assertion, control eligibility.
No record permits normative model/proof Rust or controller changes, combined
amendments, or changed verifier/dependency pins and resource caps.

### Package-distribution choice

The adopted #163 record chose a source-complete archive rather than its
lean predecessor. Include the existing verification Rust sources and
technical reference so the actual unpacked crate supports the project's
pinned formatter and same-source Verus replay. Do not use formatter skips,
dummy specifications or a second decoder.

The archive accepted for #163 had 51 files, including all 35 runtime/model/proof
Rust files byte-identical to its source. Its compressed size was about 901 KiB
instead of the predecessor's 659 KiB. Ordinary builds still have zero default runtime
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

### Adopted decode-support layout

The adopted #171 record covers only `src/decode.rs`,
`src/decode/proof_support.rs`, the package inventory and the technical reference
inventory. It moves the existing 132-line reconstruction proof/view block through
a cfg-guarded include into the same logical `decode` module. Body bytes,
visibility and open/closed declarations are unchanged; recombination reproduces
the original decoder file exactly. Five proof-reference paths follow the move.
Attached contracts, loop invariants and executable bodies remain in place.

The recorded original source `233574e0392a7b460c6732d597008eca039f3971`
passed complete CI and repository/archive Verus with 3,518/0 each and all 4,554
observed project function identities retained, including the original lemma
namespace. The bound archive has 52 files and 36 Rust files; pinned formatting,
compiled native test identities, MSRV, resource and actual consumer gates passed.
These are engineering observations, not exhaustive source proof coverage,
lifetime verification or independent normative correspondence.

Source #171 was refreshed from accepted recovery main
`0e1158a1dc563474081cf52487295e00a684a0eb`, retaining all four exact proposed
blobs. After owner adoption through #172, it was refreshed again to
`884e2063c8a5054a17040ce47a59910769481e1d`; every source check, including fresh
policy gates, passed before source review. The owner merged #171 at
`24dd11ed3f32ae899802ce019076cda5e61fbe48`. Fresh actual-main CI and both
repository/archive proofs were independently accepted: 288 Python tests without
skips, 3,518/0 per proof replay, unchanged observed function identities and a
52-file/36-Rust source-bound archive. The earlier failed policy hold was not
waived or overridden.
This record cannot authorize Tier-1 cleanup, another module, a controller
amendment or a combined change.

### Proposed private Tier-1 test layout

The new #174 record covers only `src/tier1.rs`, `src/tier1/tests.rs`, the
technical reference and package inventories, and the binding-regression test
file. It moves the existing private `cfg(test)` module body without changing
the production/contract/proof prefix or its `tier1::tests` identity and private
access. The runtime-facing file decreases from 15,501 to 8,250 lines; the
standalone test module has 7,167 lines after pinned formatting.

Exactly 119 test references follow the move. The accepted ghost-support sources
and five proof-reference moves remain unchanged. The actual archive has 53 files
and 37 Rust files; all 666 compiled native test identities match the accepted
baseline. All 147 actual Tier-1 tests, missing/stale binding regressions, actual
archive MSRV 660+3 and real image consumer 37+1 checks passed.

The recorded original source `79c6f9aecd316e70fbcf323eda7fdbfd58c8c036` passed
fresh full CI and repository/archive Verus with 289 Python tests without skips,
3,518/0 per proof replay and all 4,554 observed project function identities
retained. Actual pinned formatting, source binding and native/MSRV/resource/
consumer gates passed. These are engineering observations, not exhaustive
source proof coverage, lifetime verification or normative correspondence.

Draft #174 was normally refreshed from accepted profile main
`452d07bbc3eb059324dc2e6583b5c0097b50c881`, preserving all five exact proposed
blobs. Its policy hold is real, not a check to waive. This separate record must
first be owner-adopted with every check green; then accept fresh main, refresh
the source branch again and require fresh complete checks before source review.
This record cannot authorize another module, a controller/model change or a
combined amendment.

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
image consumer and source-inclusive packaging. The #169
[decode-support profile operation](decode-support-profile-authorization.md)
was rolled back after required actual-main validation failed; its forward
permission is spent. The owner-adopted #170
[recovery authorization](decode-support-profile-recovery-authorization.md)
was deployed at `0e1158a1dc563474081cf52487295e00a684a0eb` and accepted from
fresh actual-main CI and repository/archive proof artifacts. It installed only
the fixed decode-support profile and added no source record. This source
record did not activate itself by changing a feature branch.
Normative model/proof targets and controller amendments remain
outside the source-record profiles.
Unknown fields/profiles, missing history, malformed trusted data, extra protected
files and non-regular file modes fail closed.

The owner-adopted #173
[Tier-1 test-profile authorization](tier1-test-profile-authorization.md) was
deployed at `452d07bbc3eb059324dc2e6583b5c0097b50c881` and accepted from fresh
actual-main CI and repository/archive proof artifacts. It installed only the
fixed five-path Tier-1 profile and added no source record. This proposed record
does not activate itself by changing a feature branch; the existing
decode-support record cannot authorize Tier-1 cleanup.

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
    },
    {
      "id": "proposal-171-ghost-support",
      "profile": "decode-proof-support",
      "source_pr": 171,
      "source_head": "233574e0392a7b460c6732d597008eca039f3971",
      "ci_run": 40730904780,
      "verus_run": 40729319509,
      "rationale": "Exact ghost proof/view relocation preserves the executable/contract/proof bodies and original decode namespace; original-file recombination and every observed function identity match. Not normative approval or source-PR merge permission.",
      "paths": {
        "src/decode.rs": {
          "before": "d2f27aff98662153229e6bb85be644150937c92d",
          "after": "afd81f4a1d9d17a79f952248e602798e544db53e"
        },
        "src/decode/proof_support.rs": {
          "before": null,
          "after": "848e266a5715993d226b5905fe112f035b183c87"
        },
        "validation/normative-coverage.json": {
          "before": "05237df10a29fd9e430180a0f7250da810db567f",
          "after": "26369b786df8431b476a3a67f83a454e8968375c"
        },
        "validation/expected-package-files.txt": {
          "before": "208d745650df83c5531c01f5ec7632f065da72e2",
          "after": "1afd84378f402a66b4a59b83fb17cdd5cbdaa454"
        }
      }
    },
    {
      "id": "proposal-174-tier1-tests",
      "profile": "tier1-test-extraction",
      "source_pr": 174,
      "source_head": "79c6f9aecd316e70fbcf323eda7fdbfd58c8c036",
      "ci_run": 40772805265,
      "verus_run": 40772806908,
      "rationale": "Mechanical private-test relocation preserves the complete production/contract/proof prefix, test-module identity and accepted ghost support. Exact evidence/package bindings and current compiled native identities are retained; not normative approval or source-PR merge permission.",
      "paths": {
        "src/tier1.rs": {
          "before": "97eb35539d40f4059a8bf735e2b8ff82baf26b32",
          "after": "be2b059c31a5b0db4571d8fc61e506c01fa69a47"
        },
        "src/tier1/tests.rs": {
          "before": null,
          "after": "24b8b6ed98ffbe15af1af8dcc216f9edbcf82767"
        },
        "validation/normative-coverage.json": {
          "before": "26369b786df8431b476a3a67f83a454e8968375c",
          "after": "c7436555216afc20d9cd86dfa7d0291deb2fd179"
        },
        "validation/expected-package-files.txt": {
          "before": "1afd84378f402a66b4a59b83fb17cdd5cbdaa454",
          "after": "a2d0538bf1db8f48537bb303bd55666df142ff16"
        },
        "validation/tests/test_framework.py": {
          "before": "43da2a6f2e292bef0b9d411f772b59993f39403d",
          "after": "bc1541157dfcb818f70546589e524dfb52d2afff"
        }
      }
    }
  ]
}
```
