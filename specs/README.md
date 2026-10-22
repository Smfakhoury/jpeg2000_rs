# Specification audit guide

## F6 reconstruction profiles (bounded stage contracts)

The first F6 stage starts with supplied signed 384-bit magnitudes (six
little-endian `u64` limbs), not compressed bytes or Tier-1 meaning. It covers
every ROI shift in `u8`, both quantization modes, the full coefficient sequence,
and the returned one-row plane's samples, width, height and fraction bits.
The integer-input adapter must reject `i64::MIN` before validating quantization
parameters or the decoded-byte budget, as the production call order requires.
The magnitude path validates parameters, then the eight-byte-per-sample budget,
then samples in order, and finally the `u32` width. Restored magnitudes must fit
`i64::MAX`, including for negative inputs; this is not a normalization premise.
Irreversible results describe the implementation's integer Q16 arithmetic,
including zero-sized steps after truncation, not real-valued 9/7 conformance.

The second bounded profile is the entire `inverse_5_3` sequence: geometry
validation and interleaving, symmetric boundary extension, both lifting
passes, either low-pass parity, empty input, and the high-only singleton's
arithmetic right shift. Every `i64` input is admitted to the relation; checked
neighbor-sum, update-bias and sample arithmetic determine typed rejection,
not an assumed coefficient bound. The shared lifting contract also describes
the complete partially modified buffer on failure, retaining all untouched
positions. Its fixed-product branch is an integer implementation model used
by 9/7, not a whole-9/7 theorem.

Later F6 acceptance still requires inverse-transform plane and remaining
sequence profiles, component transforms, component/tile assembly and their
composition into component-sample relations. These completed quantization
and 5/3 sequence profiles do not complete F6. Arbitrary Part-2 kernels and
unavailable-reference adjudication remain separate, as do parsing and
entropy decoding.

### Reconstruction reading path

1. `part1_magnitude_*` defines six little-endian limbs, lexicographic magnitude
   comparison and cross-limb logical shifting. These are finite bit-vector
   denotations on all 384 bits, not an assumption that coefficients already fit
   a machine sample.
2. `part1_roi_coefficient`, `part1_quantization_step` and
   `part1_dequantized_sample` specify restoration before narrowing, Q16 step
   truncation, centered scaling and checked signed output.
3. `part1_quantized_prefix` fixes sample order and first-error absorption.
   `part1_inverse_quantization` adds parameter validation, budget checks and
   complete plane metadata. `part1_inverse_integer_quantization` adds the
   integer adapter's earlier `i64::MIN` rejection.
4. `part1_interleaved`, `part1_lifting_sample`, `part1_lifting_prefix` and
   `part1_inverse_five_three` specify full sequences and mutable lifting states.
   Prefixes here are algorithm denotations, not invariant-only witnesses.

The executable theorem entry points `inverse_quantize_values` and
`inverse_quantize_plane` take an explicit decoded-byte budget. Both actual
`ResourceLimits` adapters call these cores with `maximum_decoded_bytes()`;
the adapters' getter-to-private-field linkage is not a separately contracted
resource-projection theorem. No width, pixel or work-unit policy is silently
added to this stage.

This directory contains the semantic models used by production contracts.
It is an audit entry point, not a claim that every selected standard clause
has been reconciled or proved.

## Reading path and reference status

1. Read the selected editions and acquisition boundaries in
   [REFERENCES.md](../REFERENCES.md).
2. Use [normative-coverage.json](../normative-coverage.json) to locate the
   implementation, model, contract, proof and independent test for a topic.
   This is a technical index, not an exhaustive clause checklist.
3. Compare the semantic definitions in [mod.rs](mod.rs) with the authorized
   reference, then inspect their actual production contracts under [src/](../src/).
4. Read representation/induction details under [proofs/](../proofs/) as needed.
   The established and unestablished relations are listed in
   [PROOFS.md](../PROOFS.md#concrete-contract-boundaries).

| Reference | Model entry points | Audit distinction |
|---|---|---|
| T.800 (2024-07), core coding/JP2 | `part1_*`, marker/packet, geometry, MQ and transform models | Check encoded field widths, sample precision, signedness, coordinate grids and arithmetic domains against the selected reference. |
| T.801 (2023-08), extensions/JPX | `part2_*`, ADS/ATK/NLT/CBD and composition models | Some syntax interpretation used acquired T.801 (2021). The selected 2023 reconciliation still requires authorized text; no missing clause numbers are invented here. |
| Selected normalized composition profile | `part2_layer_input`, `part2_owned_layer_input`, `part2_source_over_pixel`, `part2_composition_geometry`, `part2_composition_layer_transition` | Normalization to 16 bits, channel/alpha construction, floor-scaled sampling, checked addresses, typed errors and work limits are explicit selected implementation policies, not blanket claims about all JPEG 2000 profiles. |
| T.808 (2026-06), JPIP | `part9_*` and databin/range models | The cited catalog record is not an acquired normative text or a completed edition audit. |
| T.814 (2019-06), HTJ2K | `part15_*`, `valid_ht_*` and HT cleanup/refinement models | Separate coding semantics from implementation capacity bounds and generated-image qualification. |

For a clause-level review, record the authorized edition/clause, its variables,
units and ranges beside the corresponding model and evidence. The unresolved
edition boundaries above must remain unresolved until the authorized source
is available; a successful solver run does not fill them in.

## Normalized source-over vocabulary

These symbols describe the selected composition stage, not the original
codestream component precision:

| Symbol | Definition and range |
|---|---|
| `p` | `PART2_NORMALIZED_SAMPLE_BITS = 16` |
| `M` | `PART2_NORMALIZED_SAMPLE_MAX = 2^p - 1`, represented by `u16::MAX` |
| `h` | `PART2_NORMALIZED_SAMPLE_HALF = floor(M / 2)` |
| `Cs`, `Cd` | Source and destination color samples, in `[0, M]` |
| `As`, `Ad` | Source and destination alpha samples, in `[0, M]` |
| Premultiplied source | Every active `Cs <= As`; absence of source alpha means opaque straight samples |
| `A` | Unbounded natural alpha numerator: `As*M + Ad*(M-As)` |
| `C` | Unbounded natural color numerator: `Cs*As*M + Cd*Ad*(M-As)` for straight source, or `Cs*M*M + Cd*Ad*(M-As)` for premultiplied source |

Output alpha is `floor((A + h) / M)`. Output color is zero when `A = 0`;
otherwise it is `floor((C + floor(A/2)) / A)`. Positive-denominator rounding
is nearest-integer with upward ties. Arithmetic models use natural numbers;
production checked conversions and their bounds are proved separately.

The domain and presentation equivalences are checked by
`normalized_composition_domain_matches_binary_width` and
`symbolic_source_over_matches_word_normalization` in
[proofs/spec_properties/mod.rs](../proofs/spec_properties/mod.rs). These relate
the named mathematical quantities to the prior word constants; they do not
replace an independent standard-edition audit.

## Semantic definitions versus internal proof support

`part2_layer_input` describes selection from supplied converted channels:
scan all channel kinds first, then check the area and required color planes,
then validate and combine alpha planes. Required colors are returned in slot
order, regardless of input order; red and gray share slot zero. Missing,
duplicate, out-of-range or incorrectly sized planes have explicit typed
rejections. Optional alpha is an **ordered fold** of
`floor((a*b + h)/M)` with a rounding step after each plane. It is not a single
product rounded once. One plane preserves its representation flag; two or
more produce straight alpha. Association validation and earlier normalization
are outside this stage's input-relative theorem.

The actual raster and first-failure definitions remain here. In particular,
`part2_composition_budgeted_prefix` determines execution order and error/state
propagation and must be audited as part of the layer denotation. Its solver
opacity does not remove that obligation; the body and checked base/step/
absorption proofs remain available.

Prefix-membership witnesses used only to establish an additional safety
property live in [proofs/internal_specs.rs](../proofs/internal_specs.rs).
Compatibility re-exports in `mod.rs` do not make those induction details part
of the normative reading path. Representation views and mathematical/
production refinement lemmas remain in their existing proof modules.

Resource policies are audited separately from coding semantics: each stored
pixel charges one unit per active output color plus one alpha unit. Stores
precede charging; overflow preserves the stores and old counter, and a work
limit error preserves stores and the incremented counter. These are selected
bounded-decoder behaviors, not newly asserted normative JPEG 2000 equations.
