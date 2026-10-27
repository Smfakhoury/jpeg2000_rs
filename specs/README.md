# Specification audit guide

Use this guide to follow decoder behavior from executable Rust to mathematical
models, contracts, and supporting evidence. Start with one component or either
worked reading example below; the [detailed semantic reference](AUDIT_REFERENCE.md)
retains the longer explanations and local contract boundaries.

## What is established, and what is not

Production contracts relate actual executable definitions to explicit models:
values, typed errors, check order, mutable state, and preserved fields where
specified. Cargo and Verus share [src/lib.rs](../src/lib.rs) and the executable
definitions; there is no second proof-only decoder. Each theorem's inputs and
preconditions determine its scope. A supplied-coefficient theorem, for example,
does not establish the preceding compressed-byte interpretation.

These relations do **not** by themselves establish correspondence to every
selected standards clause, complete public-decoder correctness, standards-level
numerical tolerances, official conformance, or release authorization. Read
[the concrete boundaries](../PROOFS.md#concrete-contract-boundaries) before
composing claims across stages. Generated vectors and independent implementation
comparisons are supporting evidence, not official certification.

The released verifier uses `--no-lifetime`: Verus lifetime analysis is disabled;
Rust borrow checking is not. See the [tool boundary](../PROOFS.md#current-lifetime-analysis-limitation)
and [reproduction instructions](../PROOFS.md#reproduce-verification).

## Reading path and reference status

1. Check the selected edition and acquisition status below and in
   [REFERENCES.md](../REFERENCES.md).
2. Locate a component in the map below. The maintained
   [technical inventory](../validation/normative-coverage.json) supplies named
   implementation, model, contract, proof, and independent-test pointers.
3. Read the actual Rust body and its adjacent `verus_spec` contract. Compare the
   model's domains, units, rounding, failures, and state updates with the
   authorized reference; then follow the proof and test pointers.
4. Consult [PROOFS.md](../PROOFS.md) for composition and tool boundaries and the
   [detailed reference](AUDIT_REFERENCE.md) for semantic explanations. Neither
   an inventory entry nor a solver success completes a normative walkthrough.

| Selected reference | Status and audit boundary |
|---|---|
| T.800 (2024-07), core coding / JP2 | Selected edition; model-to-normative correspondence still needs clause-level review. |
| T.801 (2023-08), extensions / JPX | Selected edition, but acquired T.801 (2021) informed ADS/ATK/NLT/CBD interpretation. The editions remain unreconciled; authorized 2023 full text is needed. The five Part 1 progression orders and POC sequencing do not imply support for the additional Part 2 orders. |
| T.808 (2026-06), JPIP | Catalog-only reference, not acquired normative text or a completed edition audit. |
| T.814 (2019-06), HTJ2K | Acquired reference; contracts and generated-image checks are not a completed normative audit. |
| T.803 (2024-02), conformance | Conformance target, not a decoder feature or certification. Official assets have separate access/use conditions. |

[REFERENCES.md](../REFERENCES.md#standards) records official source links and
acquisition distinctions. Availability does not imply permission for agent
processing or redistribution. This guide reproduces no standards text and
supplies no inferred clause/page citations.

## Three kinds of material

- **Normative semantics to audit:** coding/format denotations in [mod.rs](mod.rs)
  and the component model files below. Their names are navigation aids, not
  evidence that every selected edition's rules have been checked.
- **Implementation, numerical, and resource policy:** capacity, fixed-point
  rounding, normalization, supported profiles, typed rejections, and limits
  must be read as explicit choices, not silently promoted to normative rules.
  See [validation guidance](../validation/VALIDATION.md) for independent
  numerical comparisons and their limits.
- **Supporting lemmas and invariants:** [proofs/](../proofs/) contains
  representation, induction, and algebraic support.
  [internal_specs.rs](../proofs/internal_specs.rs) holds invariant-only witnesses;
  it does not replace the semantic models or authorize assumptions.

The inventory is a technical reference index, not an exhaustive clause checklist.
The [proof-coverage schema](../validation/proof-coverage.schema.json), version 3,
requires `source_function_coverage: "not-measured"`. Observed functions are
neither a complete source-function denominator nor functional-coverage evidence.

## Component map

Pointers below are starting points; use the inventory for individual symbols
and the linked boundaries before inferring a larger composition.

| Component | Actual source → model | Current boundary to keep in view |
|---|---|---|
| Bytes / containers | [codestream.rs](../src/codestream.rs), [jp2.rs](../src/jp2.rs), [jpx.rs](../src/jpx.rs), [jpip.rs](../src/jpip.rs) → [mod.rs](mod.rs): `part1_codestream_read_segment`, `part9_*`, `part2_*` | Raw reader, metadata, container and selection contracts have distinct inputs; a local marker theorem is not the whole public decoder. See [container composition](../PROOFS.md#complete-native-container-selection-and-typed-output-composition). |
| Packet / entropy | [packet.rs](../src/packet.rs), [mq.rs](../src/mq.rs), [tier1.rs](../src/tier1.rs) → [packet_input.rs](packet_input.rs), [header.rs](header.rs), [tag_tree.rs](tag_tree.rs), [packet.rs](packet.rs), [mq.rs](mq.rs), [legacy.rs](legacy.rs), [mod.rs](mod.rs): `part15_*` | Reader, traversal, retained-contribution and coefficient-state relations must be composed explicitly. See [mixed dispatch](../PROOFS.md#complete-unconditional-mixed-contribution-body-and-tier-1-dispatch). |
| Coefficients / arithmetic | [magnitude.rs](../src/magnitude.rs), [tier1.rs](../src/tier1.rs), [refinement.rs](../src/refinement.rs) → [mod.rs](mod.rs): `part1_magnitude_value`, `ht_cleanup_plane` | Full 384-bit arithmetic support is not HT byte meaning; supplied metadata/bytes and raw coefficient frames remain explicit. See [magnitude interface](AUDIT_REFERENCE.md#full-native-magnitude-interface). |
| Transforms | [transform.rs](../src/transform.rs), [decode.rs](../src/decode.rs) → [mod.rs](mod.rs): `part1_inverse_quantization`, `part1_inverse_level`, `part1_apply_component_transform` | Stage contracts distinguish supplied coefficients, prepared components and real owner buffers. Deterministic fixed-point behavior alone establishes no normative tolerance. See [reconstruction](#reconstruction-reading-path). |
| Channels / color | [decode.rs](../src/decode.rs), [jp2.rs](../src/jp2.rs), [color.rs](../src/color.rs) → [mod.rs](mod.rs): `part2_palette_component`, `color_convert_jp2`, `color_convert_selected` | Palette/CDEF routing, registration, conversion and crop have separate admissions. Supported enumerations and ICC rejection are explicit. See [semantic detail](AUDIT_REFERENCE.md#semantic-definitions-versus-internal-proof-support). |
| Layers / frames | [jpx.rs](../src/jpx.rs) → [mod.rs](mod.rs): `part2_layer_input`, `part2_build_timeline`, `part2_render_normalized_frame` | Normalized 16-bit source-over and floor-scaled sampling are selected policy. Timeline/frame results are input-relative, not proofs of compressed decoding or container normalization. See [composition vocabulary](AUDIT_REFERENCE.md#normalized-source-over-vocabulary). |

## Example 1: full-width magnitude addition

**Code → model.** Open `Magnitude::checked_add` in
[src/magnitude.rs](../src/magnitude.rs). It adds six little-endian `u64` limbs
with a carry. Its closed `magnitude_value` projection is defined by
`part1_magnitude_value` in [mod.rs](mod.rs): the natural value of all 384 bits.
`magnitude_addition` is natural addition; `part1_magnitude_capacity()` is `2^384`.
No normalized-coefficient or HT-success premise is required.

**Contract.** The adjacent `verus_spec` binds both operands to that full-value
view. If their sum is below capacity, the result is `Ok` with exactly that sum
and the raw/value representation relation. Otherwise the result is exactly
`MalformedInput { offset: 0, kind: ArithmeticOverflow }`, expressed through
`part1_transform_error(true)`. This is a finite-capacity arithmetic theorem,
not a theorem that compressed bytes denote a particular coefficient.

**Proof / test.** Follow the loop's prefix-sum/carry invariant and
`production_magnitude_add_prefix_step`, `production_magnitude_add_limb`, and
`production_magnitude_prefix_update` in
[proofs/production_impl/mod.rs](../proofs/production_impl/mod.rs).
`exact_full_capacity_addition_matches_bit_carry_reference` in
[src/magnitude.rs](../src/magnitude.rs) independently compares bit-array addition,
including complete carry propagation and exact overflow errors. The
[arithmetic boundary](../PROOFS.md#full-native-magnitude-arithmetic) explains
how this supports reconstruction without proving HT byte semantics or a
completed ITU-clause audit.

## Example 2: PLM packet-length admission

**Code → model.** Open `Parser::parse_plm` and its actual callee
`Parser::validate_packet_lengths` in [src/codestream.rs](../src/codestream.rs).
Their models in [mod.rs](mod.rs) are `part1_parse_plm`,
`part1_plm_groups_from`, `part1_validate_packet_lengths`, and
`part1_packet_length_admission_from`. The raw variable-length decoder is modeled
by `part1_packet_length_decode` in [packet_input.rs](packet_input.rs).
Group lengths and borrowed byte slices derive from the real segment payload,
not supplied decoded lengths or a bank of decoder results.

**Contract.** Each adjacent `verus_spec` equates the actual result and final
state with the model from the initial raw state. Admission binds the complete
parser view; PLM also binds the next index and all group views, including
arbitrary preexisting groups. Read the ordering, not just the successful count:

- Minimum payload length and index checks precede the PLM index increment.
  A successful index admission, including 255 → 256, survives later failure.
- Packet-header-byte addition commits before its cap check. Genuine work
  charging precedes variable-length traversal; malformed varints retain these
  charges but do not commit a packet count.
- Completed traversal commits the global packet count before its cap check.
  A successful group is appended before the tile-part cap check, so that
  failure retains the appended group.
- Work errors use the parser position; byte/count overflow and malformed
  varints use the marker offset. Empty encoded input still checks counters.

The relations preserve typed first failures, partial commits and untouched
parser/group fields; they do not prove PLT/final metadata or the outer
parser/decoder joins.

**Proof / test.** Follow `parser_view`, `segment_view`,
`packet_length_groups_view`, the source-derived traversal invariants, and
`decode_packet_length` / `Parser::charge_work` in the same source file.
Independent wide-arithmetic tests there include
`actual_packet_length_admission_preserves_byte_work_packet_overflow_and_limit_commits`
and `actual_plm_groups_preserve_charge_errors_append_caps_and_real_segment_reader_join`.
The [PLM boundary](../PROOFS.md#actual-packet-length-admission-and-plm-group-production)
and inventory enumerate the remaining evidence. This reading example is not a
completed normative clause walkthrough or official conformance evidence.

## Reconstruction reading path

Start with [the magnitude example](#example-1-full-width-magnitude-addition),
then follow ROI restoration, quantization, lifting, plane geometry, component
transforms and assembly in the
[detailed reconstruction path](AUDIT_REFERENCE.md#reconstruction-reading-path).
Keep supplied-coefficient, supplied-tile and raw-input boundaries separate;
see [bounded reconstruction](../PROOFS.md#bounded-supplied-coefficient-reconstruction)
and [real owner-buffer composition](../PROOFS.md#complete-wavelet-stage-and-real-owner-buffer-mct-composition).

## Frozen F5H directional-reader stages

For the directional-reader boundary, start with the actual raw-reader fields,
stuffing, exhaustion and termination behavior in the
[HT directional-reader contracts](AUDIT_REFERENCE.md#ht-directional-reader-contracts).
Then follow the [full refinement-plane profile](AUDIT_REFERENCE.md#frozen-full-f5h-refinement-plane-profile)
and [cleanup-plane profile](AUDIT_REFERENCE.md#frozen-f5h-cleanup-plane-reconstruction-profile).
Reader contracts alone do not establish the complete Tier-1 caller. “F5H” is an
internal profile label, not a standards clause or conformance designation.
