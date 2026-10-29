# Exact delegated authorization for the publication-name profile

## Decision and authority

The owner requested Cargo package `jpeg2000-decoder` and explicitly delegated
engineering PR review and merge on 2026-10-28 at 22:57 UTC. Rust imports use
`jpeg2000_decoder`; the repository remains `microsoft/jpeg2000_rs`.

This proposes one new, single-use operational authorization. It is inactive on
a feature branch. The owner or the expressly authorized delegate may adopt this
documentation-only PR on trusted main only after every required check and
prerequisite original-artifact audit passes. A delegated merge must be recorded
as delegated, not as an independent human or normative review.

Earlier #166, #169, #170 and #173 forward permissions are spent. They cannot
authorize this different operation. Nothing here authorizes registry publication,
visibility/tag changes, history rewriting, skipped gates or protection overrides.

## Exact permitted operation

Only these two regular files may be replaced, preserving mode `100644`:

| Path | Required original Git blob | Authorized replacement Git blob |
|---|---|---|
| `validation/check-model-review-hold.py` | `12bf19c1f9c803e51121aceff8f6fd993f23199f` | `62c64100a3cf1c4fee3d5e5afc920f09b6c231e7` |
| `validation/tests/test_model_review_hold.py` | `7b2195bc548b52e3e2cb8286652bd4f86bc0b482` | `0c7aa72b1123c6a78e3077db1607106a4670d9fe` |

Replacement bytes are retained in maintenance candidate
`324618503552056333be1ae2c478c9e3254508fb`.

The controller adds only `crate-publication-rename`, with this entire fixed set:

- `Cargo.lock`, `Cargo.toml`, `examples/validation_decoder.rs`
- `fuzz/Cargo.lock`, `fuzz/Cargo.toml`, `fuzz/src/lib.rs`
- `fuzz/fuzz_targets/arbitrary_codestream.rs`
- `fuzz/fuzz_targets/arbitrary_jp2.rs`
- `fuzz/fuzz_targets/arbitrary_jpip.rs`
- `fuzz/fuzz_targets/arbitrary_mq.rs`
- `fuzz/fuzz_targets/arbitrary_packet.rs`
- `fuzz/fuzz_targets/arbitrary_tier1.rs`
- `fuzz/fuzz_targets/structured_framing.rs`
- `validation/check-packaged-source.py`
- `validation/generate-proof-coverage.py`
- `validation/image-consumer-probe/Cargo.lock`
- `validation/image-consumer-probe/Cargo.toml`
- `validation/image-consumer-probe/src/decoder.rs`
- `validation/image-consumer-probe/src/lib.rs`
- `validation/image-consumer-probe/tests/adapter.rs`
- `validation/proof-coverage.schema.json`
- `validation/tests/test_packaged_source.py`
- `validation/tests/test_proof_coverage.py`

These are exactly the 23 protected paths in the rename prototype, not a wildcard.
The profile cannot name codec `src/`, normative models/proofs, the controller,
another module or a combined amendment. It still requires a separate adopted
record binding every original/proposed regular-file identity and the complete
path set. Adding the profile alone approves no rename or source change.

All existing rejection cases remain. Four new tests cover trusted-only adoption,
every changed protected identity, extra codec/model/controller changes and
forbidden profile paths. All 61 focused controller/package tests pass.

## Required qualification

Before review and adoption, independently accept the maintenance candidate's
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40789823929) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40789826129),
and the separate rename prototype
`5d99c8bc42f3ab932c54213519ab49d54880e8ee`:
[full CI](https://github.com/microsoft/jpeg2000_rs/actions/runs/40789824278) and
[repository/archive Verus](https://github.com/microsoft/jpeg2000_rs/actions/runs/40789825409).
Pending, missing, skipped or failed gates are not passes.

The source prototype changes no decoder/model/proof Rust byte. It updates package
and Rust import identities, real consumer/fuzz manifests and locks, exact archive
binding and the strict proof namespace. Old-name/dependency-only evidence remains
rejected. All third-party versions/checksums/pins stay unchanged. Actual repository
URLs, historical source-bound evidence and stable validation protocol identifiers
are preserved; namespace comparisons allow only the exact requested crate-prefix
change. Publication remains disabled.

Require actual archive formatting/binding, MSRV, real consumer, native/platform/
resource gates, unchanged obligations and exact observed function identities
modulo only that crate prefix. These are engineering observations, not exhaustive
coverage, lifetime verification or independent normative correspondence.

## Execution and rollback

After actual adoption on trusted main:

1. Read accepted current main and verify both required-original identities.
2. Prepare a NEW commit whose sole parent is that main and whose only changes
   are the two exact replacements above. Preserve every registry entry and all
   other source, documentation, manifests, locks, workflows, pins and caps.
3. Run existing full CI and released repository/archive Verus on that exact
   result tree. Independently audit original artifacts and thresholds. Changed
   base trees require fresh result qualification, not an earlier candidate's run.
4. Recheck unchanged main and use only a normal fast-forward update. No force,
   admin/status/settings override, disabled/skipped gate or expanded scope.
5. Accept fresh actual-main full CI/Verus before using the profile. The rename
   still needs a separate green exact registry record, adopted on trusted main,
   normal source refresh and fresh all-green source qualification before merge.

If post-deployment validation fails, restore only the two required-original
blobs in a new non-force commit, provided those paths still have the exact
authorized replacements. Preserve all other files and validate actual main
again. Concurrent changes require stopping, not broadening this authorization.

This forward permission is single-use; it cannot authorize later controller
versions or integrate the rename itself.
