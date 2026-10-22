# Specification audit guide

## Complete actual mixed native body and segment dispatch

`part1_code_block_body_staged` consumes the sole full native contribution view,
actual body sequence, raw decoder state and four real owner caps. It composes
the single `part1_code_block_body_prepare` producer and then the actual
`high_throughput` branch: `ht_packet_body_segments` or
`part1_legacy_code_block_body`. No route result is an input.
`part1_code_block_body_transaction` models the real whole-decoder clone/commit
operation. Both actual staged/public functions have unconditional complete
Result/state posts; their earlier selected posts remain available too.

`part1_tier1_segment` likewise joins the actual Tier-1 public segment dispatcher:
zero-pass rejection, then `ht_group_route` using the stored cleanup **option**,
then actual HT group or legacy segment execution. It does not replace the option
with a flag, normalize private state or introduce a decoder-validity premise.
The old conditional HT relation remains a proved consequence.

All native primitive/helper bodies and resource policies are preserved. In
particular, a zero-count empty legacy contribution can succeed while the HT
route rejects it; the legacy replay/input limit is not invented for HT decoding.
Selector-only loop invariants used for the former disjoint proofs are gone.
The actual shared header parser and complete precinct/packet caller remain
separate composition obligations, not assumed successful traces.

## Frozen actual HT packet contribution-body route

This stage is the actual HT branch of
`packet::decode_code_block_contribution_body_staged` and its outer transactional
clone/commit entry. The sole F5L contribution projection/getters supply every
native header field and both complete physical segment arrays; no competing
header view or parser is introduced. All native stored counts, malformed header
combinations, old decoder states and supplied body bytes remain admitted.

The sole actual common prefix refines `part1_code_block_body_prepare`; both
selected branches reuse that producer and its model. It checks exact body length and route, then sets zero
bitplanes before HT segment-count admission. HT traversal checks each actual
segment Result, pass count, native length/address arithmetic and exact borrowed
body subrange, then selects cleanup/placeholder/refinement using the current
decoder phase and cleanup flag. Decoder effects precede decoded-pass accounting;
offset/index updates follow. Final exact consumption and declared-pass mismatch
errors retain the staged partial state, while the outer real clone/commit
wrapper preserves the complete original decoder on every failure. The private
loop is unconditional; only the staged/public postconditions select the actual
HT header flag. Their profiles are not restricted to a normalized header or a
decoder whose coding style already guarantees successful HT execution.

The complete models are `ht_packet_segment_route`, `ht_packet_segment_body`,
`ht_packet_body_run`, `ht_packet_body_segments`, `ht_packet_body_staged` and
`ht_packet_body`. `HtPacketBodyProgress` represents every local progress field;
the native private record has one closed owner view. These relations include
the current cleanup flag, exact `None`/stored cleanup distinctions, original
decoder vectors/counters and all actual typed failure effects through the
existing Tier-1 producers. The optional public byte-slice view is open because
it contains no private fields; no private decoder representation is opened.

No supplied decoder/legacy outcome, header/shape normalization, success, trace
or adequate-budget premise is used. Empty slices route differently according to
actual state; `None` and retained empty data are not conflated. Legacy branch and
shared header parser are untouched. Shared contribution/zero-bitplane/route facts
are owned by F5L and consumed from their real implementations. Full header
parsing, mixed legacy/HT union and precinct orchestration remain separate
producer obligations; this selected body boundary is not full F5H completion.

## Frozen whole-decoder clone and actual selected HT group profile

The next boundary is the actual staged HT branch of `decode_segment`, selected
by HighThroughput coding or Mixed coding with a retained cleanup-plane option.
The selector is not normalized to the current-cleanup flag. Its whole-decoder
raw clone preserves every field, vector, malformed shape and retained state
through the existing sole F5L logical bitplane/MQ/legacy projections and three
actual data clone contracts. Original field/vector allocation order is retained.

The real packet-pass grouping, repeated skip-one/skip-two placeholder loop,
substantive byte entry and final clone/commit are modeled directly. Any failure
after staging preserves the entire original decoder, even if the staged copy
has partial counter/reset effects. Success commits the entire actual staged
state and reports the original packet pass count. No supplied clone/replay/
legacy outcome, normalized old state, valid stream/shape or sufficient budget
premise is introduced. The legacy dispatcher else remains untouched and outside
this selected-branch theorem until actual legacy composition is available.

## Frozen retained-refinement and placeholder orchestration profile

The next native boundary is the complete actual
`decode_high_throughput_refinement`, including arbitrary retained byte buffers,
cleanup planes, pass counters and flags. Its actual admission does **not** check
the configured pass limit or `maximum_passes`; those checks must not be invented.
The cumulative-byte check precedes cloning/appending the retained buffer, and
the 2046-byte check remains after those real operations. Nonempty input is
replayed from the retained cleanup plane through the actual complete refinement
producer, not from a supplied trace or the current output plane.

An empty concatenated buffer records refinement passes and segment accounting
without replaying or replacing the output plane. Empty input with nonempty
retained bytes instead replays those bytes. Replay rejection preserves the
entire decoder; successful replay stores plane/data/passes before checked
pass/segment accounting, so late errors retain prior writes. No normalized old
state, valid stream/plane, success or adequate-budget premise is admitted.

The accompanying complete `skip_high_throughput_segment` profile retains actual
phase/pass/set admission, placeholder/replacement flags and first-error writes.
A phase-zero placeholder clears retained refinement bytes/passes and the
current-cleanup flag, but preserves the old cleanup-plane option and output.
Set-count updates precede those resets; pass/segment updates follow. Byte/work
limits absent from this operation are not added. Staged multiple-group/mixed
dispatch remains a separate boundary requiring actual clone/legacy relations.

The existing two-statement retained-buffer reset is now shared by the real
symbol caller and phase-zero skip operation. Its unconditional whole-decoder
state contract isolates that frame join without changing operation order,
capacity, proof budgets or solver context flags. The void helper is referenced
as a proved implementation fact, not mislabeled as a named-Result contract.
Independent integration regressions also execute the existing real staged
packet-group loop, rollback and successive continuation/replacement calls.
Those regressions are not a substitute for a full clone/dispatch theorem.
The pinned non-Copy derived `Clone` implementations expose no functional
specification; full group closure must use actual raw cloning relations,
including the shared logical bitplane-state view, rather than opaque native
identity or a supplied legacy outcome.

## Actual legacy MQ/bypass three-pass boundary

[mq.rs](mq.rs) supplies a complete declarative MQ state/Result denotation.
The stored nineteen contexts are pairs of probability-state index and MPS bit.
The forty-seven probability transitions are an independent ghost sequence joined
to the production table, not an assumed transition trace. Initialization,
context mutation/save/restore, byte loading, conditional exchanges,
renormalization, exact work admission, consumed-byte reporting and the existing
predictable-termination test retain all native error priorities and mutations.
In particular, byte advancement can survive addition failure; code/count loading
can survive padding-counter overflow; a work-limit error retains its increment.

[legacy.rs](legacy.rs) composes those real MQ effects and the complete raw-reader
effects with actual significance, refinement and cleanup operations. The whole
bitplane state observes geometry, bitplane, both bool vectors and all three
completion flags. The plane observes all six words, sign, significance,
refinement and optional legacy bitplane of every coefficient. Context selection
uses the real complete ordered-neighborhood producer, including causal
suppression. Stripe, column and row folds model the actual optimized traversal,
not a normalized or reordered coefficient list. Cleanup includes run-mode
eligibility/interruption, the two uniform run bits, sign reads, remaining rows
and optional segmentation-symbol reads.

Native invalid states are not erased by constructor invariants: the pass
preflight checks geometry/vector lengths and completion order, while actual
coefficient operations retain their own different bitplane bounds. For example,
marking accepts the magnitude owner's native 0..383 domain, whereas refinement
first rejects bitplanes at or above 292. Both behaviors remain in the models.
Symbol/visit/plane/statistic changes before a later failure are retained.
No public pass assumes valid bytes, successful decoding, normalized words,
valid headers, adequate budgets or a supplied trace.

Opaque model phase boundaries control proof unfolding only. The concrete
production stages prove each phase against its body; both private reader-adapter
implementations delegate to proved actual readers. There is no second executable
decoder or uninterpreted successful entropy outcome.

The same module now composes actual segment scheduling and contribution replay.
`part1_legacy_segment` checks the real pass/byte/empty-segment admission before
reader construction, restores or initializes real contexts, runs the actual
MQ/raw pass loop, checks existing termination policy and commits accounting in
its real order. `part1_legacy_contribution` preserves exact replay-work charging,
retained-record take/append/restore, the real segment's result and final record
retention. Its starting numerical state and input bytes come from the actual
retained record, never from a supplied successful decoder outcome.

Complete logical bitplane and nineteen-context projections appear in both
whole-decoder and retained-record frames. The actual faithful raw plane,
bitplane and legacy-record Clone implementations preserve malformed shapes and
all words/flags and retain original field/allocation order. Private arithmetic
lemmas receive their six-word facts from those real native representations, not
from a normalized-coefficient premise. Native record restoration and data
append are verified operations, not assumed state transformers.

The actual route/pass/accounting getters and zero-bitplane setter supply native
owner facts for the next packet caller. The setter checks a previously stored
value before its phase/counter checks and does not invent a 292-bitplane bound.
Its existing checked multiply/add is one verified production helper over every
native `u16`, with unchanged field commits. Header validity is not a premise.

The internal contribution entry is not transactional: work charging can precede
a later arithmetic error, and a taken retained record need not be restored on
replay failure. These Tier-1 results alone do not establish outer clone/commit,
body/pass-total joins, packet/header associations or the public mixed dispatcher;
the next actual packet stage closes the selected code-block caller boundary.

The next actual packet stage is now in [packet.rs](packet.rs). Its sole raw
contribution projection includes every public field, the stored count and both
complete 164-entry arrays. Real descriptor lookup, checked body subranges,
actual legacy contribution execution and ordered pass/offset/index commits
form the complete legacy packet-body model. Final consumption/pass-total
errors occur after decoder effects, not before them.

Shared preparation models actual length/route/zero-setting admission. On the
actual legacy selector `!contribution.high_throughput`, the staged and public
packet-body callers now have complete Result/full-state refinements through the
actual faithful whole-decoder Clone. Staged errors can expose partial work;
public errors roll back every decoder field. This profile predicate selects a
real format branch only: it adds no header validity, geometry, normalization,
success, budget or trace premise. The HT packet branch and whole packet/header
association remain separate integration obligations, not guessed outcomes.

## Frozen full F5H refinement-plane profile

The continuing stage is the entire actual `decode_ht_refinement_plane`, for
both one and two refinement passes and every native pass/bitplane/cap input.
Initial rejection and complete duplication precede reader construction and the
cleanup-significance snapshot. Two passes execute reverse magnitude refinement
first, then forward significance propagation. All six raw words and flags,
both physical readers, complete claimed/stuffed vectors, every statistic and
the separate evolving discovery plane/pending sign coordinates are observed.

Significance discovery follows actual four-row stripes and four-column patches,
including vertical-causal neighbor suppression. Discovery writes precede delayed
per-patch sign reads and output stores; discovery signs are not silently replaced
with output signs. Final claim/padding/stuffing/collision/work rejection follows
all stores. Each first-error partial state and transactional boundary must match
the real producer. No valid-plane, normalized-coefficient, valid-stream,
successful-decode or adequate-budget premise is introduced. Private geometry
bounds must follow from the real duplicate checks. An additive-only, one-pass or
reader-only result is not completion of this full S+R profile.

The completed caller boundary includes both the unconditional inner symbol
caller and actual `decode_high_throughput_segments` byte entry. All native style,
pass counts, retained state and three real owner caps are admitted. Public pass
and significant-bitplane preflight occurs before the real byte parser; byte and
work admission then precede the inner caller. Inside that caller, actual metadata
still precedes pass conversion. Neither order is transplanted onto the other.
`None` and `Some(empty)` retain different pass/segment/refinement-state effects.
Inner failures can clear retained refinement data before rejecting a refinement
segment, while later accounting overflows retain prior committed fields.

The private reader constructor wrapper was removed after the production entry
began calling its existing shared verified free implementation directly; its one
test uses the same implementation. No public API or second executable decoder
was introduced. Pending sign vectors are still allocated once per actual patch
and dropped at the original patch-iteration boundary. Magnitude operations use
the real owner contracts, including complete native subtraction and zero.
Continuation/empty replay, placeholder skipping, replacement orchestration and
mixed HT/legacy routing remain outside this completed entry profile.

## Earlier cleanup-only symbol-caller subprofile

The earlier bounded caller profile was the **cleanup-only** branch of the actual
`Tier1CodeBlockDecoder::decode_ht_coefficient_symbols`: the supplied parsed
segment has `refinement == None`. This is an explicit format-profile selection,
not a valid-stream, normalized-coefficient, successful-decode or budget premise.
The real metadata producer's result feeds the real numerical reconstruction;
metadata is not a theorem input. Metadata decoding precedes pass-to-bitplane
conversion and subtraction. Reconstruction and the complete cleanup-plane
duplicate precede clearing retained refinement bytes/passes. Plane/stencil/
reader rejection therefore leaves the entire decoder unchanged. The final
assignments precede checked set/pass/segment/byte accounting, so a later overflow
can expose earlier committed fields. Every decoder field, including untouched
legacy/MQ/bitplane state, must be framed.

The shared cleanup preparation and final state-store stages remain unconditional
over their native inputs. The earlier caller theorem selected absent refinement
only; it did not assume a refinement trace. The complete current refinement and
substantive public-byte profiles above now subsume this subset and provide their
own unconditional production relations. Continuation/replay, placeholder
skipping, replacement/mixed sequencing and whole-decoder composition remain
separate closure obligations.

## Actual raw palette lookup

`part2_palette_value` uses the stored entry count, actual physical column
descriptor sequence and actual physical signed-value sequence. It first checks
whether the physical column count fits `u8`, then the logical entry/column
coordinates, then the checked `usize` row-major address, then actual value
presence. A physical value outside the logical table is not a valid lookup;
truncation inside it returns `InvalidBoxContents`. Column-count/address overflow
returns `ArithmeticOverflow`, with offset zero. The common existing
`part2_composition_error` denotes these same two error values; it adds no
composition premise.

This deliberately models the raw lookup, not the parser's palette-shape rules
or the mapper's precision/sign/sample-index rules. The raw operation returns the
stored `i64` unchanged, including both extrema, and ignores descriptor
precision/sign. Its complete actual production relation is unconditional on
stored shape and native coordinates. Palette parsing, component mapping and
their composition remain unproved by this raw lookup result alone.

### Complete component copy and palette interpretation

`part2_typed_component_copy` preserves all input fields and the ordered CDEF
sequence without adding geometry, precision or sample-tag validation.
`part2_palette_component` instead changes precision/sign from the actual
selected descriptor and starts with empty CDEF. It preserves the source's
width/height, subsampling and registration even when those fields are malformed.

The precision variable is the descriptor's bit count, restricted by the actual
body to 1..38. `part2_palette_modulus` denotes the same power of two; explicit
bit-vector facts relate its original multiply-by-two loop to the existing
`part1_output_minimum`/`part1_output_maximum` signed/unsigned range functions.
Source indices are interpreted independently of source precision/sign metadata.
They must fit the native 16-bit palette index and stored logical entry count
before `part2_palette_value` checks physical storage.

`part2_palette_samples` is the ordered whole-sequence denotation, including the
first failing sample. Its prefix is essential result semantics, not a separate
execution trace premise. Empty samples still reject an invalid descriptor but
do not trigger physical-column-count/table/entry failures absent from the
executed body. The actual full result also contains the output sample tag and
all metadata. Whole-header cmap selection, CDEF routing, typed-image admission,
parsing and selected-pixel joins remain outside these component results.

## Frozen F5H cleanup-plane reconstruction profile

The next full numerical boundary is the actual `reconstruct_ht_cleanup_plane`,
not a validated-metadata wrapper. It admits arbitrary stored quads, dimensions,
initial-row flags, masks, coordinates, U values and native bitplane/work limits.
Only metadata width is checked before creating the blank output using the input
plane's dimensions; old input coefficients are ignored. Coordinate addition
precedes metadata clipping. Scanning precedes MagSgn consumption, which precedes
placement and both success counters. Duplicate or out-of-output placements can
therefore fail after consuming a magnitude. All six stored words, flags,
statistics, both exponent stencils and every reader field are observed by the
shared production stages, including their first-error partial states.

Column-zero reset, sample-one maximum, sample-three overwrite and end-of-row
stencil swap follow actual metadata vector order, not assumed geometric order.
Noninitial kappa retains its conservative base-plus-exponent overflow check.
No-significance, empty or fully clipped metadata does not impose numeric-domain
checks absent from the body. Final physical padding, stuffing and work errors
remain after all placements. This profile does not establish refinement,
placeholder/replacement/mixed routing or complete entropy conformance.

The raw `Tier1CodeBlockDecoder::plane` postcondition is an existing API bridge
consumed by `decode::prepare_tile_component_with_kernel`. It remains outside the
same-file internal-caller index: there is no invented Tier-1 caller or generic
API-root exemption. Its exact view is listed as a proof reference.
The raw and signed `Result` projections are open over their public payloads and
closed owner views, so outside-module consumers can connect actual returned
fields to the mathematical result without revealing private stored fields.

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

The decoder's existing `DecodedComponent::typed_samples` and
`DecodedImage::typed` now have their own complete Result relations. They use
the actual F7 typed-sample, component, geometry and channel-definition views:
the result includes every signed/unsigned sample, all image/component metadata
and the existing empty output channel-definition lists. No typed/CDEF class,
getter, metadata definition or color conversion is changed or duplicated.
The original loops use the same actual scalar rounding/clipping stages.

Their domains and order differ from the separate transform conversion API.
Component conversion rejects precision outside 1..38 before its sample-length
byte budget. Image conversion instead charges the cumulative eight-byte count
before calling each component, so that budget error precedes that component's
precision error. Fraction values above 62 reject only when a sample is read;
empty valid-precision components therefore retain success even with arbitrary
fraction bits. Checked rounding bias and unsigned level shift may overflow
before clipping. Dimensions, subsampling and registration are copied as raw
metadata, not assumed valid or newly validated. Prefix models preserve the
first error through the entire sample/component sequence.

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

The actual read-only `PacketPrecinctDecoder` decoder/count/slice accessors
expose a public closed projection of the complete private decoder Vec.
The indexed Result covers every `usize`, returns the exact initialized entry
or `MalformedInput { offset: 0, kind: InvalidPacketHeader }`, and makes no
assumption that the parallel packet-state/context vectors have matching shapes.
This raw representation interface does not assign entropy or packet meaning.

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

Both actual image assembly entry points now have unconditional complete Result
relations: `assemble_image` and the active `assemble_selected_image`. They fold
the supplied tile samples into every component and preserve all image and
component metadata. Registered geometry uses the actual Q16 registration
coordinates. Cumulative decoded-byte accounting, physical component-info bounds,
raw fraction consistency, overlap, missing coverage and arithmetic errors retain
their original order; neither valid headers nor sufficient budgets are premises.
The real production code shares component, tile, rectangle, row and pixel stages,
whose mutable views include every sample and visited flag on failure.

Base and selected assembly intentionally have different domains. Base assembly
rejects zero tile axes and requires the exact tile area. Selected assembly checks
matching tiles' fractions and overflowing extents even off-region, but copies
only their intersection: samples outside that intersection need not exist.
Wrong-component tiles are ignored, and selected missing coverage has its distinct
selection error. Budget checking remains before selected crop-offset arithmetic.
Zero axes/counts and arbitrary fraction values retain their native behavior.
Only the inactive nested selected-assembly duplicate was removed; the theorem
links the actual active caller, not that dead copy.

`ImageSize::component` and the three reconstruction direction/count selectors
also expose complete Results over their full native `u16`/`u8` domains. Raw
component counts and decomposition levels are not assumed to fit the physical
64/32-entry arrays. These interfaces do not establish parser validity.

The actual public `prepare_tile_component` now has a complete unconditional
Result relation for its standard **None-kernel** call path. The same private
`prepare_tile_component_with_kernel` body has the corresponding explicit
`kernel.is_none()` result guarantee; its Some-kernel behavior is not inferred.
Every actual raw plan/precinct/decoder/plane/coefficient field, style selector
and cap is admitted. An Arbitrary transform with no kernel rejects exactly.
No valid header, normalized coefficient, covered band, successful callee,
adequate budget or supplied execution trace is an entry premise.

The production coordinator shares its real initialization, collection,
placement, coverage and resolution stages. It checks the selected band count
before initializing bands, charges nine bytes per sample cumulatively before
allocation, checks block geometry before collecting coefficients, reads the
actual row-major raw coefficients with full 384-bit signed/midpoint semantics,
then invokes the actual ROI/quantization and checked sample/mask writer.
Unselected placements remain ignored. Coverage precedes reconstruction.
The entire fold supports arbitrary native resolution selectors, None/H/V/2D
directions, per-stage parity, the actual integer 5/3 or fixed-Q16 9/7 algorithms,
ordered detail indices, output budgets and cursor errors. Final dimension,
sample-count, component, origin and fraction fields are related exactly.
The real prepared-to-tile getter also preserves every field/sample.

Only derived private append-loop structure bounds are used to discharge a
Vec-push precondition; they are proved from initialization and never imposed
on the public plan input. Complete working-state views include all bands'
metadata/samples/visited flags and the byte counter on failure. Independent
whole-plan tests reuse the existing widened integer/rational transform oracles,
not the production transforms, with nonzero completed one-bit input blocks,
mixed/multilevel directions, parity, reordered and ignored placements and
ordered malformed/resource cases.

F6 remains open: the Some-kernel/arbitrary coordinator profiles, general MCT
paths and actual whole-decoder composition from
coefficients through reconstruction, assembly and typed conversion
into component-sample
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
9. `part1_assembly_pixel`, the row/rectangle/tile/component prefixes and
   `part1_assembly_append_plane` describe the shared real assembly stages,
   including complete write masks and ordered byte accounting.
   `part1_assemble_image` and `part1_assemble_selected_image` close the actual
   full image entry points relative to supplied tiles, not supplied coefficients.
10. `part1_decoded_signed_prefix`, `part1_decoded_unsigned_prefix`,
    `part1_decoded_component_typed_samples`, `part1_decoded_typed_components`
    and `part1_decoded_image_typed` give the existing decoder conversion's
    complete sequence, metadata, empty-CDEF and first-error relation.
11. `part1_prepare_init_prefix`, coefficient row/plane and placement prefixes,
    `part1_prepare_coverage`, `part1_prepare_resolution` and `part1_prepare_fold`
    compose the real standard coordinator. `part1_prepare_standard_component`
    and `part1_prepare_finish` close the actual public Result, not a surrogate
    coordinator or a theorem assuming the callees succeeded.

All prefixes here are algorithm denotations, not invariant-only witnesses.

### Full native magnitude interface

`magnitude::magnitude_value` is a public **closed** natural-number view of all
six little-endian `u64` limbs, defined by `part1_magnitude_value`; it is not a
low-word surrogate. Capacity is `part1_magnitude_capacity() = 2^384`.
The existing raw `magnitude_limbs` view and ROI contracts remain available.

The original receiver operations have full native-domain contracts:

- `checked_add`: exact natural sum below capacity; otherwise precisely
  `MalformedInput { offset: 0, kind: ArithmeticOverflow }`.
- `checked_sub`: exact full-384-bit natural difference iff the first operand
  is at least the second, including successful zero for equality. Every
  underflow returns the same exact arithmetic error. Raw/value/shape/capacity
  agreement is exported with no no-underflow or positive-result premise.
  The original native borrow algorithm, including an incoming borrow across
  a `u64::MAX` subtrahend word, is unchanged. A prefix-order proof connects the
  existing lexicographic guard to natural order, and subtraction reuses the
  proved carry algebra with the operands rearranged.
- `checked_shl`: exact multiplication by `2^shift` when representable, for
  every `u16` shift. Zero succeeds even for shifts at least 384; nonzero
  values reject there. No HT-only success premise is imposed.
- `shift_right`: floor division by `2^shift`, including zero at shifts
  at least 384, with the complete raw-buffer relation retained.
- `low_u64`: full-value modulo `2^64`; `is_zero`: true iff the full value is
  zero. The numeric/raw representation agreement and intrinsic capacity
  bounds are exported by these contracts.
- `limb`: for every native `usize` index, exactly the corresponding raw word
  at indices 0..6 and `None` otherwise, with no index precondition.

The public pure owner fact
`magnitude::representation_proofs::magnitude_limbs_shape(value)` establishes
`magnitude_limbs(value).len() == 6` without an executable getter call.
Its small representation proof submodule stays under the owning module so
the existing public closed projection and private fields remain closed;
the indexed mathematical denotation is `part1_magnitude_limb` in `specs/mod.rs`.

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

## Frozen F5H directional-reader stages

The raw Tier-1 observation substage retains every stored coefficient limb,
legacy midpoint tag and sign/significance/refinement flag. Plane dimensions
and the actual coefficient sequence are not normalized to a rectangle:
coordinate bounds, native checked address arithmetic and actual sequence
length determine the exact indexed result/error. Signed observation ignores
stored bits/tags when insignificant, otherwise applies the actual full-native
legacy midpoint and checked overflow branch without entropy or normalization
premises. It is a raw producer interface, not coefficient reconstruction.

The frozen numerical MagSgn substage is the actual complete
`reconstruct_ht_magnitude` operation for every native completed-U,
embedded-bit/value flag and decoded-bitplane input, with every initial
directional-reader state admitted. Original parameter rejection precedes
any read. The exact result includes the full 384-bit numeric value, sign,
exponent and every final reader field on success or first error. Accumulation,
embedding, odd-center correction, centering, checked left scaling, right
division and zero rejection use the real published F6 contracts. No stream
validity, sufficient budget or successful arithmetic/read is assumed.
This substage is numerical reconstruction, not complete plane placement.

The frozen cleanup-metadata stage is the complete actual segment traversal,
not coefficient reconstruction. Inputs retain all native width, height and
maximum-U values, the actual supplied segment views and actual work limit.
Header checks, reader construction, ordered rows/pairs/members, north/west/
east context selection, context-zero suppression, UVLC/event decisions,
sample bounds, significant-sample counting, quad stores, ticks, final collision
check and counter conversion are part of one deterministic result model.
Every output metadata field and typed first error is included. Internal
geometry/allocated-length facts are proved from the real header checks;
neither valid streams, successful reads nor sufficient budgets are assumed.
Helper extraction moves the existing traversal bodies, without another
executable decoder, changed allocation capacities or reordered stream reads.

The packet-pass grouping substage has the native `u8` input domain: zero and
165..=255 reject, while 1..=164 produce the exact cleanup-pass count,
refinement-pass count, placeholder count and skipped-bitplane count.
All four actual result fields and the typed rejection are modeled. This is
the actual `HtPassGroup::from_packet_passes` implementation, not a theorem
that a block caller executes, replaces or routes those passes correctly.

The frozen byte-partition contract uses the actual selected code-block
byte limit, not a chosen or existential budget. Its inputs are both
actual supplied slices (including `Some(empty)` versus `None`), all native
lengths and the actual `ResourceLimits` object; its output includes every
encoded/substream slice, offset and direction. The current foreign limits
getter now has the specifically authorized owner-local limit view and exact
postcondition. Cleanup/refinement size rejection precedes the code-block
budget check, which precedes Scup interpretation. Every encoded/substream
slice, offset, direction and total byte count is included. Invalid Scup
bounds retain their typed rejection. No arbitrary-budget parser wrapper,
successful-input assumption or narrowed native object domain is substituted.

The next frozen interpretation substage is the actual cleanup UVLC/quad-word
caller: prefix, every native `u8` suffix length, combined prefix value,
initial/non-initial U pairs for every `u8` mode, and quad-word lookup for every
`u8` context. It composes the actual reader/event/advance/tick states on every
return. Invalid modes/contexts reject without consuming; a set suffix bit
above bit seven rejects **after** that bit is consumed, while zero bits above
the result width may continue. Initial mode three preserves its special
long-prefix route and read order; bias one/three and checked byte conversion
retain their actual rejection points. Quad lookup uses closed views of the
unchanged production tables, then context-zero MEL suppression, length
validation, actual advance and final tick in that order. This stage does not
assume valid codewords, available bits, adequate budget or valid counters.
It is not a quad traversal or coefficient-reconstruction theorem.

The next frozen substage is the complete actual MagSgn reader API: construction
from 0..=65534 bytes; LSB-first words of 0..=39 bits and unchanged-state rejection
of larger counts; physical and synthetic `0xff` bits with forward stuffing;
all physical/fill/work counters and cursor fields; and `finish`'s exact
bounded drain, padding-one rejection and first typed failure. All native
counter values remain admitted. Cursor advancement precedes selected-counter
charging, which precedes work charging and the work-limit check. The finish
fuel is the checked native-width product `input.len() * 8`, including its
overflow error before testing whether physical bits remain. Neither a valid
stream nor enough budget is assumed. The word and drain recurrences belong
to the semantic reading path, not to invariant-only support.

The following frozen substage is both actual refinement directional readers
and their shared claim/padding wrapper. Each input accepts 0..=2046 bytes.
Forward reads supply zero fill after physical data; reverse reads begin with
unstuffing enabled and supply zero fill after exhaustion. Positioned reads
report the actual native-width byte/bit position, then advance the cursor,
charge the selected physical/fill counter, check their sum for overflow and
check the work cap. Counter errors and budget errors preserve those ordered
partial mutations. The wrapper records successful physical claims, rejects
repeat claims, records reverse stuffed-bit masks at the actual loaded byte,
and checks all unclaimed/unstuffed padding bits at finish. Models include both
complete readers, optional reverse-reader absence and both complete vectors.
No successful decoding, sufficient budget, disjoint claims or valid padding
is assumed.

The next frozen cleanup-caller substage includes the complete actual VLC
`peek_vlc` and `advance_vlc` APIs for every `u8` count, and `mel_event` for every
native cache/state/counter value. Peek is non-mutating and returns the word
accumulated before copied-reader exhaustion, collision, budget/overflow
failure or a set bit above its 16-bit result capacity. Advance stops at the
first actual read failure with its full nested state retained. MEL cache
priority, 0..=12 state validation, head/suffix consumption, state adaptation,
termination caching and event-counter overflow are explicit, including
partial suffix-reader mutations on failure. Event suffixes are MSB-first.

The first finite subprofile is the actual cleanup MEL/reverse-VLC reader pair:
MEL accepts 1..=4078 bytes, reverse VLC 2..=4079. Contracts cover constructor
rejection, exact loaded bytes and returned bits, terminal-nibble handling,
directional stuffing, exhaustion, positions, all final fields, counter overflow,
and the first work-limit failure. Counter domains retain the full `u64` range;
failure is not assumed absent. Reverse VLC eagerly loads the previous byte only
after charging a successful bit; budget rejection therefore preserves its old
loaded byte with an advanced bit cursor. MEL exhaustion sets its cursor to -1.
The pair's actual `read_mel_bit`/`read_vlc_bit` wrapper includes shared-byte
claim/collision rejection and both inner and outer work counters. All nested
reader fields and MEL event fields are framed, including failed reads after
loading and failures after recording a successful byte claim. Constructor
order is MEL, then VLC, then terminal-byte setup. The model accepts the actual
supplied slices; it does not assume they were partitioned correctly upstream.
These are selected implementation-state relations, not a human normative
endorsement of T.814.

The directional substage now includes all five actual readers, MagSgn words
and finish, refinement physical-bit claims/padding, VLC peek/advance and MEL
events. The cleanup pair's final shared-byte collision check is included.
The subsequent UVLC/quad-word interpretation stage above is also complete.
Actual upstream partitioning and the complete metadata traversal above are
now proved. The remaining stages are pass execution/sequencing, numerical
cleanup magnitude/coefficient reconstruction, significance/refinement
block transitions, replacement/placeholder sequencing and mixed HT/legacy
routing. Completing directional stages does not close F5H or full-block
acceptance. In particular, matching the byte-reader state machine does not
prove that the block caller requests the right symbols or reconstructs the
right coefficients.

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

`ht_cleanup_plane` and its sample/quad/finish models describe complete actual
cleanup coefficient reconstruction from supplied metadata and bytes, not
merely a scalar magnitude. Read counter/read/store ordering and both exponent
stencils alongside the actual final reader termination. Raw Tier-1 models
preserve all six limbs, significance/sign/refinement and legacy midpoint
behavior without assuming a well-shaped plane or normalized coefficient.
The small `tier1_raw_plane_*`, `tier1_coefficient_*` and `tier1_set_magnitude`
denotations are complete field/record projections used by real producer
contracts; they do not introduce an alternate executable implementation.

`color_crop_image` describes the actual selected-output crop on a supplied
converted image. Its ordered checks preserve the zero-size, coordinate-
overflow and bounds error precedence. The whole-image identity shortcut is
explicit and does not silently validate existing channel lengths. Otherwise
the complete ordered channel/row/column denotations stop at the first checked
address or missing requested sample. All source metadata other than output
dimensions is retained. These definitions do not assume canonical color
channels or prove earlier region selection, palette interpretation or pixels.
The `color_crop_ready` invariant bundle is internal proof support.

`color_convert_selected` connects conversion and cropping at the actual
selected-output caller. Its input is the supplied typed image and requested
reference-grid target, not a precomputed converted plane. Conversion completes
first; only then are target-origin/extent subtractions checked with the
existing target error, followed by cropping. It does not model the earlier
algorithm that derived the requested target or decoded the typed components.

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
