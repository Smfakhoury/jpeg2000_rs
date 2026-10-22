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
positions. The entire `inverse_9_7` sequence is also contracted: interleaving,
the two parity scaling passes and all four ordered lifting passes, including
their actual Q16 constants, checked intermediates and unchanged singletons.
Scaling and the four-stage coordinator retain complete failure buffers.
This is an integer implementation model, not real-valued 9/7 conformance.

The horizontal, vertical and single-level two-dimensional 5/3 and Q16 9/7
entry points have complete plane Result contracts. Their production row and
column coordinators are shared with the actual level implementation; there
is no second executable transform. No successful-result, coefficient-bound or
budget premise is imposed on these public contracts. Internal conditional
contracts select the standard-kernel profile (`kernel == None`); arbitrary
kernel meaning is not inferred from those conditions.

The existing geometry/error distinctions are intentional. Standalone
horizontal/vertical stages check dimensions and the output budget before
accessing band samples, and ignore unaccessed trailing samples. Horizontal
zero-width planes and vertical zero-height planes retain their existing
behavior. A full level instead rejects either zero output axis and validates
all four exact band sizes before its output budget. Zero-sized intermediate
low/high bands remain valid where the selected parity requires them.
Column storage models preserve every earlier write on a later failure.

The existing public `convert_samples` API also has complete ordered rounding,
level-shift, clipping and typed-error semantics. Its actual precision domain
is 1..38 with 0..62 fraction bits; rejected parameters are not preconditions.
It uses sample length, not plane dimensions, for its budget and deliberately
does not validate plane geometry. Checked rounding/level-shift overflow occurs
before clipping. This API is exercised by concrete consumer tests; it is not
the decoder's separate image/component conversion path.

The actual free reversible and fixed-Q16 irreversible component transforms
have complete mutable `DecodedImage` state relations. Their shared production
loop checks the existing first-three-component compatibility and sample
lengths, then transforms samples in order. All `i64` triples are admitted;
checked sum/product/bias/addition failures determine rejection. Every image
and component metadata field, sample length and component beyond the first
three is preserved, including on failure. The previously completed sample
prefix remains committed when a later pixel fails. These functions do not
add a plane-area consistency or precision/registration policy absent from
their existing compatibility checks. A closed `View` covers all decoded
fields; it does not reuse or alter F7's typed/CDEF structures.

Raw kernel/step/coefficient views and exact getter contracts expose only the
actual consumed representation fields, including all coefficient bytes and
the recorded width. They impose no parser validation or normalization facts
and do not by themselves prove an arbitrary-kernel algorithm.

The real quantized-block write loop in
`decode::prepare_tile_component_with_kernel` now calls one shared verified
production block/row/pixel implementation. Its complete working-subband state
model includes width, height, fraction bits, samples and the entire visited
mask. Source/destination/mask length mismatches, previsited cells and checked
coordinate/index errors retain all prior writes. Metadata and buffer lengths
remain unchanged; marked cells never become unmarked and previsited sample
values are protected. No rectangular-shape, prevalidated-buffer or sufficient
length premise is imposed on this transition. The caller's earlier geometry,
coefficient extraction, quantization and fraction checks remain in their
original order; this theorem does not silently replace those checks.

F6 remains open: the entire coefficient-placement coordinator, actual multilevel
and resolution-plan execution, multiple-component transforms, decoder typed
conversion, component/tile assembly and their composition into component-sample
relations still require their own complete actual-stage theorems. Arbitrary
Part-2 kernels and unavailable-reference adjudication remain separate, as do
parsing and entropy decoding.

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
   `part1_scaling_prefix`, `part1_nine_seven_lifting_prefix` and
   `part1_inverse_nine_seven` add the complete fixed-point sequence.
5. `part1_band_row`, `part1_column_prefix`, `part1_store_column_prefix`,
   `part1_horizontal_prefix` and `part1_vertical_prefix` fix extraction,
   concatenation, row-major storage, first-error order and complete buffers.
   `part1_inverse_horizontal`, `part1_inverse_vertical` and
   `part1_inverse_level` add actual geometry, budgets and plane metadata.
6. `part1_round_fixed`, `part1_conversion_prefix` and `part1_convert_samples`
   describe the existing public final-sample conversion API.
7. `part1_component_values`, `part1_component_pixel_state`,
   `part1_component_transform_prefix` and `part1_apply_component_transform`
   give the actual complete decoded-image component-transform relation.
   `part1_component_transform_frame` states its complete metadata/extra-plane
   preservation property, not a substitute for sample semantics.
8. `part1_quantized_block_pixel`, `part1_quantized_block_row` and
   `part1_quantized_block_rows` describe the actual coefficient write stage,
   including samples and visited flags. They do not establish the entire
   `prepare_tile_component_with_kernel` result simply because its callees
   now have contracts.

All prefixes here are algorithm denotations, not invariant-only witnesses.

### Full native magnitude interface

`magnitude::magnitude_value` is a public **closed** natural-number view of all
six little-endian `u64` limbs, defined by `part1_magnitude_value`; it is not a
low-word surrogate. Capacity is `part1_magnitude_capacity() = 2^384`.
The existing raw `magnitude_limbs` view and ROI contracts remain available.

The original receiver operations have full native-domain contracts:

- `checked_add`: exact natural sum below capacity; otherwise precisely
  `MalformedInput { offset: 0, kind: ArithmeticOverflow }`.
- `checked_shl`: exact multiplication by `2^shift` when representable, for
  every `u16` shift. Zero succeeds even for shifts at least 384; nonzero
  values reject there. No HT-only success premise is imposed.
- `shift_right`: floor division by `2^shift`, including zero at shifts
  at least 384, with the complete raw-buffer relation retained.
- `low_u64`: full-value modulo `2^64`; `is_zero`: true iff the full value is
  zero. The numeric/raw representation agreement and intrinsic capacity
  bounds are exported by these contracts.

Verified native producers are `magnitude_zero`, `magnitude_from_u64`,
`magnitude_from_limbs` and `magnitude_single_bit`. The zero helper returns
exactly the existing `Magnitude::ZERO`; the bit helper accepts the entire
`u16` domain and returns the exact arithmetic error at bit indices at least
384. Existing public associated factories retain their API and delegate to
these same implementations. Verified internal callers should use the free
factories because the pinned conditional-attribute frontend cannot infer
associated static-function contexts; no trusted specification compensates
for that limitation.

Only the partial native left-shift buffer witness is in
`proofs/internal_specs.rs`. Full value, shift and overflow denotations remain
on this reading path; their word-composition, carry and natural arithmetic
lemmas are proved in the production proof module. Native optimized limb
algorithms remain in `src/magnitude.rs`. Independent bit-array tests cover
every `u16` shift/bit index and complete 384-bit carry propagation.

The executable cores `inverse_quantize_values` and `inverse_quantize_plane`
take an explicit decoded-byte budget. The original `inverse_quantize` and
`inverse_quantize_magnitudes` entry points additionally have complete Result
contracts using `resource_decoded_bytes_limit`: the actual getter is proved
equal to that closed private-field projection. No width, pixel or work-unit
policy is silently added to this stage.

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
| Supplied composition metadata and normalized layer results | `part2_build_timeline`, `part2_select_frame`, `part2_render_normalized_frame`, `part2_render_layers`, `part2_image_from_planes` | Whole timeline construction and complete indexed-layer-result-to-frame rendering are input-relative theorems. Compressed decoding and container interpretation/normalization are not proved by these models. |
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

`color_convert_jp2` is the complete result denotation for the actual public
typed-component converter. Read `color_output_format`, `color_channel_layout`,
`color_validate_components`, `color_validate_output_size`, then
`color_convert_colors` and `color_append_alphas_prefix` in that order. Named
JP2 enumerations select grayscale, sRGB or sYCC; other enumerations and both
ICC methods have explicit typed rejection. Component validation and budget
admission happen before sample conversion. The three essential prefix models
specify complete ordered channels, sYCC planes and appended alpha; each
absorbs the first actual error. The output includes every image/channel field.
The input is supplied typed components and color metadata, not an assumed
compressed-decoder trace or a claim about how a container authored them.

`color_conversion_ready` lives in
[internal_specs.rs](../proofs/internal_specs.rs) because it only bundles
already established immutable facts across the production loops. It is not
an additional algorithmic assumption or normative rule. The byte cap view is
the actual private `ResourceLimits` field, not a default or chosen proxy.

`color_sycc_to_srgb` specifies the actual fixed-point pixel conversion over
every native `u16` luminance and `i32` difference pair. Its coefficients are
91881, 22554, 46802 and 116130 with denominator `COLOR_FIXED_SCALE = 2^16`.
Each delta is rounded separately, with exact half ties away from zero,
before the three output sums are clamped to `[0, 65535]`. The pure
`color_fixed_round` uses unbounded integers; `color_rounded_fixed` separately
retains the checked pre-division overflow boundary for arbitrary `i128`.
`color_image_difference` separately connects real component lookup and nominal
sampling to centered chroma scaling by `65535/(2^p-1)`, with the same signed
nearest/tie policy. Neither pixel theorem proves component/output-budget
validation or the entire color-conversion loop.

`color_channel_layout` describes actual typed-image/CDEF routing before sample
conversion. Its default path is used only when every component has no
definitions. The explicit ordered fold retains alpha kind/association,
permits a repeated identical component/color assignment, ignores unspecified
channels, and rejects conflicting components or invalid associations at their
first occurrence. Required-color completeness is checked last. The native
`usize::MAX` unset marker is explicit; it is not a valid assigned index from a
representable component sequence.

`color_normalize_component` is a full fail-fast plane denotation over supplied
typed integer components and target reference grids. `color_component_index`
uses fixed-point registration in units of 1/65536, registered ceil origins,
registered floor sampling and explicit edge clamping. Sample precision
selects nominal maximum `2^p-1`; signed samples are checked and translated
from `[-2^(p-1), 2^(p-1)-1]` before exact nearest/upward-tie 16-bit scaling.
The word-width facts are checked by `color_precision_binary_bounds`, not
implicitly assumed. All geometry/range/tag/missing-sample errors remain
part of the model. The prefix definition describes the first actual sampling
failure; it is not a sufficient-budget or valid-input-only theorem.

`part1_raw_reader_initial`, `part1_raw_reader_load`, `part1_raw_reader_read`
and `part1_raw_reader_termination` describe the complete selected raw-bypass
reader state and typed outcomes. Byte/bit order, stuffing, synthetic marker
padding, consumed-byte position and counter/error ordering belong on the
semantic reading path. Native fields retain their stated widths; the initial
sequence is constrained to representable slice lengths. These models do not
claim that callers choose the correct pass/segment or normative context.
The termination model distinguishes an empty, zero-work raw pass from
synthetic bits consumed from missing input, and recognizes the unconsumed
`0xff`/`0x2a` full-byte/stuffed-padding boundary.

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

`part2_render_layers` denotes rendering the **supplied ordered placements**,
not an execution trace supplied as an assumption. Its initial canvas is
transparent black, its output space is sRGB if any supplied layer is sRGB
(even an inactive layer), and its layer fold stops at the first failure.
Each step first charges that layer's complete nominal decoded size, checks
the supplied byte cap, then invokes the budgeted placed-layer transition with
the carried canvas and work counter. Canvas initialization contributes its
pixel count to work and its planar storage to decoded bytes. As in the actual
implementation, the initial byte charge is checked together with the first
layer, not separately. The complete image result includes dimensions,
precision, channel kinds/association, and every color and alpha sample.
`part2_image_from_planes` also specifies the production constructor's exact
shape errors, including the accepted zero-area case.

`part2_select_frame` selects a modulo frame for indefinite playback or a
bounded repeated sequence, with exact empty/out-of-range/overflow errors.
`part2_frame_instruction_sequence` concatenates persistent predecessors and
the current frame in table order, preserving every instruction field. Frame
table positions, not stored frame indices or timestamps, determine selection.
The current frame is included regardless of its persistence flag; a transient
predecessor is skipped without erasing older persistent frames. Each render
starts afresh, and looping does not retain the preceding loop's canvas.
Instruction-level persistence and `next_reuse` are preserved as input metadata;
their interpretation by `build_timeline` is not established by selection.

`part2_build_timeline` starts with empty frames/assignments/time and processes
each instruction set `repeat + 1` times. Its deterministic transition assigns
the next new layer unless an earlier instruction reserved this position for
reuse. It models exhaustion, zero-life grouping, persistence overwrite,
start/duration accumulation, future reuse extension/conflicts, instruction
and frame budgets and exact checked-error precedence. Nested sequence folds
define expansion order and absorb exhaustion/errors; their inputs are the
composition metadata, not a supplied successful trace.

`part2_render_normalized_frame` composes actual selection, indexed layer-result
resolution and the complete renderer. A supplied outcome bank contains layer
references paired with optional normalized results. Lookup consumes the
first remaining matching result, allowing arbitrary ordering, repeated keys,
unrelated/error entries and empty slots. There is no canonical-bank or
trace-validity precondition. Missing referenced results reject with
`InvalidBoxContents`. Repeated references consume successive supplied
outcomes, preserving per-placement semantics without a cache assumption.
All selected normalized results are gathered before any placement runs;
therefore their first failure precedes any pixel/work/byte failure in earlier
successful supplied layers. The complete final image model is then the
ordered supplied-layer renderer above.

These are whole actual-function contracts, not only helper properties.
Independent tests exercise multilayer/multiframe rational buffers, all
four-frame persistence masks, finite/indefinite selection, reference-bank
permutations, distinct repeated outcomes, full timeline expansion/reuse/
timing and typed failures. Parsing, compressed-data correctness,
normalization, registration, opacity and chroma-key policies remain separate
packages. Integration and owner acceptance are not implied by local proof
success.

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
