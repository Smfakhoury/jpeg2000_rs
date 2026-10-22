# Verus proofs

## Single executable implementation

Cargo and Verus use the exact [src/lib.rs](src/lib.rs) crate root. Production
algorithms live in [src/](src/); a second proof-only decoder implementation is
not maintained.

Verus-only attributes and modules are enabled with `verus_keep_ghost`:

- [src/refinement.rs](src/refinement.rs) contains executable helpers with
  functional contracts.
- [specs/](specs/) contains mathematical models.
- [proofs/shared/](proofs/shared/) contains representation views.
- [proofs/spec_properties/](proofs/spec_properties/) contains model lemmas.
- [proofs/production_impl/](proofs/production_impl/) contains refinement lemmas.
- [proofs/internal_specs.rs](proofs/internal_specs.rs) contains auxiliary
  prefix-membership witnesses, not algorithm denotations.

Start semantic review with the [specification audit guide](specs/README.md),
which distinguishes selected reference editions, normalization policies and
internal proof support. Essential raster/first-failure semantics remain on
that reading path.

Ordinary Rust builds erase ghost code and retain Rust's borrow checking.
Source and proof changes must be validated together.

## Complete native palette lookup boundary

The actual `jp2::Palette::value` returns exactly `part2_palette_value` on its
private stored entries, complete physical column descriptors and value sequence,
for every native entry/column pair. No parsed-header, rectangular-table,
valid-coordinate or successful-lookup premise is imposed. Actual physical column
count conversion is checked before logical coordinates, followed by checked
row-major address computation and physical value access. Invalid descriptor
precision/sign does not itself make this raw lookup fail.

The stored representation stays private behind its owner view and exact public
getter contracts. Explicit matches retain the original conversion/multiplication/
addition/error order while exposing every real error, rather than relying on
uncontracted callback results. The shared local error producer is verified, not
trusted. Independent tests cover 8,820 complete supplied-state/native-coordinate
contexts, signed extrema, physical stride and first-error priority. The source
index binds the full Result, actual owner view, entry and column, with negative
tests for wrong, partial, unrelated and reordered bindings.

This raw lookup result alone does not establish palette parsing, source-sample
interpretation or composition with selected pixels.

## Complete native component copy and palette mapping

The actual `decode::copy_typed_component` and `map_palette_component` now have
unconditional complete Result relations. Copy preserves every metadata field,
signed/unsigned sample and ordered channel definition, including duplicates and
malformed source geometry. It does not invent validation of its inputs.

Palette mapping checks the actual selected column and precision before sample
access. Native signed/unsigned indices must fit `u16` and the stored entry
count before raw table lookup. Every stored value must fit the selected signed
or unsigned precision. Source geometry, subsampling and registration are
preserved; precision/sign and the sample tag come from the descriptor, and
incoming channel definitions are cleared. Empty input still checks the
descriptor but does not access or validate unvisited table storage. All
arithmetic, logical versus physical table and range errors retain their actual
order/category/offset. No parsed/normalized/rectangular/success premise is used.

The existing static component constructor API delegates to one verified free
production constructor, which the proved callers use directly. This preserves
the API and a single executable field-construction body while avoiding the
conditional frontend's static-impl contract limitation. Full mutable
channel-definition frames preserve component samples and metadata.

An independent wide-integer oracle checks 37,632 complete supplied contexts,
including invalid metadata/sign tags, integer extrema, empty/malformed tables,
descriptor boundaries and error precedence. All 256 native precision values
are checked separately. Five exact result/model/input recipes reject unrelated,
partial, reordered and constant bindings. Existing sparse-magnitude arithmetic
also explicitly instantiates its original final-limb premise to stabilize the
joined proof; no theorem domain, native algorithm, trust or limit changed.

Whole-header direct/palette component selection, CDEF record application,
final typed-image admission, parser associations and selected-pixel composition
remain separate F7 closure obligations.

## Reproduce verification

The [Verus workflow](.github/workflows/verus.yml) pins the verifier archive,
checksum, required Rust toolchain, and command. Use the same versions when
reproducing it locally. Do not substitute a rolling compiler without
revalidating the source and standard-library specifications.

The pinned release includes `cargo-verus` beside `verus`; put that directory
on `PATH`. Use its required Rust toolchain (`1.98.1` for the pinned release),
separately from the ordinary decoder toolchain. The root manifest opts the
same production crate into verification.

The `verus` Cargo feature supplies the exact pinned `vstd` dependency.
It is deliberately not a default feature: ordinary decoder builds retain an
empty dependency graph. The frontend does not enable optional features
automatically, so include `--features verus`. The released lifetime boundary
below still requires `--no-lifetime`; it is not hidden by a wrapper script.

From the repository root:

```sh
RUSTUP_TOOLCHAIN=1.98.1-x86_64-unknown-linux-gnu \
  cargo verus verify --features verus --locked --fwd-verus-args-to roots -- \
  --no-lifetime
```

To collect the same whole-crate evidence as CI:

```sh
mkdir -p target/validation
RUSTUP_TOOLCHAIN=1.98.1-x86_64-unknown-linux-gnu \
  cargo verus verify --features verus --locked --fwd-verus-args-to roots \
  --target-dir target/verus --lib -- --no-lifetime --output-json \
  > target/validation/cargo-verus-output.log
python3 validation/generate-proof-coverage.py \
  target/validation/cargo-verus-output.log
```

The collector extracts exactly one root result, writes normalized
`verus-result.json` and schema-v2 `proof-coverage.json`, and binds both the raw
Cargo output and normalized result by hash. The expected crate namespace is
`jpeg2000_rs`, not the standalone compiler's default `lib`.

Cargo can reuse an already verified artifact without emitting fresh JSON.
For a fresh local report, clean only this crate's verification artifacts with
`cargo clean -p jpeg2000_rs --target-dir target/verus`, then repeat the report
command. CI uses a fresh checkout. Reports are generated evidence, not
committed snapshots; an empty/cached output cannot become a new proof report.
If Cargo refuses to clean because the target lacks a valid `CACHEDIR.TAG`,
respect that safety check. Use a new dedicated `--target-dir` for fresh
verification instead of deleting the unrecognized directory.

The checker rejects missing/zero/dependency-only/wrong-crate/ambiguous/partial
results, any termination opt-out, and forbidden trusted codec escapes.
Cargo's exit code alone is not proof evidence. Keep this gate enabled.

For module debugging, the pinned frontend requires `focus`:

```sh
RUSTUP_TOOLCHAIN=1.98.1-x86_64-unknown-linux-gnu \
  cargo verus focus --features verus --locked --fwd-verus-args-to roots -- \
  --no-lifetime --verify-module refinement
```

Module-only results are not accepted by the whole-crate collector. CI
explicitly exercises that rejection before its complete verification.

## Current lifetime-analysis limitation

The pinned released verifier requires `--no-lifetime` for verified-module
constant initialization because of an upstream THIR/erasure initialization
error. The proposed upstream fix is
[verus-lang/verus#3266](https://github.com/verus-lang/verus/pull/3266).

This flag disables Verus lifetime analysis, not Rust borrow checking.
Documentation must not imply that lifetime analysis ran. Remove the flag
only after a reproducibly pinned fixed verifier passes the exact production
root and the relevant upstream regression suites.

## What the proofs establish

### Full native magnitude arithmetic

`magnitude_value` is the closed natural-value projection of all six
little-endian limbs. Native `checked_add`, `checked_shl`, `shift_right`,
`low_u64` and `is_zero` refine exact sum/capacity, multiplication/floor
division by powers of two, modulo-low-word and full-value zero semantics.
All `u16` shift values are covered, including large-shift zero success and
nonzero overflow. No coefficient-normalization or HT-success premise is
assumed. Public associated factories retain their API while verified free
factories supply complete producer contracts, including exact invalid-bit
errors and identity with `Magnitude::ZERO`.

The existing limb implementations are related to natural arithmetic through
proved one-bit word composition, full-value carry identities and capacity
bounds, without raising verifier limits. This interface supports actual
HT reconstruction but does not by itself prove HT byte meaning or F6's
remaining assembly bridge.

The unchanged native `Magnitude::limb` also has an exact full-`usize`
`Option` contract: each index below six returns its corresponding raw word,
and every other index returns `None`. The pure owner representation proof
`magnitude::representation_proofs::magnitude_limbs_shape` exports the intrinsic
six-word shape without exposing private fields or requiring an executable
getter call. A separate property in `proofs/spec_properties` checks that this
closed-view interface is usable from another module. These are raw
representation facts, not additional Tier-1 coefficient meaning.

`Magnitude::checked_sub` has a complete no-requires full-native Result
contract: exact 384-bit natural difference when ordered, exact arithmetic
underflow otherwise, including equal operands returning zero and full raw
representation/value/shape/capacity binding. The optimized executable
subtraction is unchanged. Its comparison guard is linked to natural prefix
order, and the existing carry-prefix algebra is reused for borrow propagation,
including incoming borrow with an all-ones subtrahend word. Independent bit
borrow tests compare complete results across sparse, boundary and dense pairs.

### Bounded supplied-coefficient reconstruction

`transform::inverse_quantize_values` and `transform::inverse_quantize_plane`
are the real production cores shared by the public integer and internal
signed-magnitude adapters. Their complete-result contracts relate every sample
and all `ReconstructedPlane` metadata to
`part1_inverse_integer_quantization` / `part1_inverse_quantization`, including
parameter, explicit decoded-byte-budget, ROI, arithmetic and final width
rejections in execution order. The domain includes all six-limb coefficients,
signs and `u8` ROI shifts; out-of-range restored magnitudes are rejected rather
than assumed absent. The original `inverse_quantize` and
`inverse_quantize_magnitudes` adapters also have complete Result contracts
using the closed `ResourceLimits` byte-field projection and its verified getter.

`transform::inverse_5_3` and `transform::inverse_9_7` have whole-sequence
contracts covering both parities, empty/singleton handling, mirrored boundaries,
all scaling/lifting passes and checked errors. The horizontal, vertical and
single-level 2D public entry points for both standard kernels additionally
specify every plane sample, all metadata and the actual configured byte limit.
The real level implementation reuses the proved production row/column
coordinators. `inverse_lift`, scaling, the four-stage 9/7 coordinator and column
storage specify complete mutable states on first failure, with untouched
positions retained. Supporting model properties establish complete sequence
lengths and plane sample-count/metadata consistency.

The existing public `convert_samples` API has full Result semantics for its
actual 1..38 precision policy, rounding, level shifting, clipping and checked
failures. Its concrete consumer is an independent API test, not the decoder's
separate image/component conversion path. Independent tests compare complete
outputs and error/mutation boundaries with bit-expanded ROI, rational Q16 and
widened-integer sequence/plane/conversion references.

The actual free `apply_reversible_component_transform` and
`apply_irreversible_component_transform` APIs have complete mutable-image
contracts, not just scalar arithmetic bounds. A closed `DecodedImage` view
contains every image/component metadata field and all sample buffers. The
shared real loop's ordered compatibility, length and arithmetic checks are
modeled together with complete failure states; metadata, lengths and every
extra component remain unchanged. Independent widened-integer tests compare
entire images, including earlier committed pixels on a later error.
No typed/CDEF structure or conversion is duplicated or modified.

Raw closed arbitrary-kernel/step/coefficient views and exact receiver getter
contracts make their consumed representation available without validation or
coefficient-normalization assumptions. They are interfaces for further
arbitrary-kernel work, not a whole-kernel theorem.

The actual quantized-block placement loop in
`decode::prepare_tile_component_with_kernel` is a shared, verified production
block/row/pixel transition. Its complete working-subband view includes both
samples and visited flags with exact first-failure buffers, unchanged metadata
and lengths, monotone visited flags and protected previsited values. Independent
wide-index tests cover full grids, short buffers, overlaps, empty blocks and
coordinate failures. Geometry/quantization checks stay before the same real
call. This is the complete write stage, not a result theorem for its surrounding
placement or multilevel coordinator.

The actual `assemble_image` and active `assemble_selected_image` entry points
now have complete, unconditional Result relations over supplied tiles. Every
output sample, component-info field, dimension, origin and fraction value is
modeled. Shared actual component/tile/rectangle/row/pixel stages preserve complete
partial sample/mask states, with first-error absorption. Registered coordinates,
physical component-info bounds, cumulative decoded-byte charging, fraction
consistency, overlaps and missing coverage retain their native ordering.
Selected assembly keeps its distinct intersection-only sample domain and
selection coverage errors; off-region matching tiles still undergo fraction
and extent checks. Budget checks remain before crop-offset arithmetic.
Independent widened coordinate/image tests compare complete results and exact
error boundaries, including raw fractions and non-normalized metadata.

Raw component-info lookup and reconstruction direction/count selectors have
full-native-domain Result contracts with no valid-header or array-size premise.
The only deleted inactive implementation is the nested selected-assembly
duplicate; active production callers use the shared verified stages.

The existing decoder component/image typed conversions now have complete
Result relations using the actual F7 views, including all signed/unsigned
samples, image/component metadata and the existing empty channel definitions.
The original production loops retain their precision, per-component and
cumulative byte-budget order. Fraction validation happens per accessed sample,
so empty components with valid precision keep their native arbitrary-fraction
behavior. Rounding bias and unsigned level-shift overflow precede clipping;
no geometry, normalized-input or sufficient-budget premise is introduced.
Independent widened references cover every `u8` precision/fraction value,
complete sequences, whole images and exact error precedence.

The narrowly scoped read-only packet decoder accessors also expose their
actual private Vec through a public closed view, exact entire-`usize`
indexed reference Result, count and complete slice contracts. Parallel
packet-state/context shape and entropy meaning are not assumed.

These are input-relative integer/fixed-point stage results, not compressed
parsing/Tier-1 meaning or measured irreversible conformance.

The actual public `prepare_tile_component` now closes the complete standard
None-kernel plan coordinator: raw decoder/plane and six-limb coefficient reads,
signed midpoint handling, initialization and nine-byte cumulative charging,
ROI/quantization, placement/coverage/fraction checks, the entire mixed-direction
multilevel 5/3 or Q16 9/7 fold and every final PreparedTileComponent field/error.
Its original private with-kernel body shares these real stages and exposes the
explicit None-kernel guarantee; no proof-only executable coordinator is added.
All public native malformed-input and resource cases remain modeled rejection,
not header/normalization/coverage/budget/success/trace preconditions.
The prepared-to-tile getter also has exact full-field/sample semantics.
Independent full-plan and ordered-error tests use the previous widened
reference algorithms and completed one-bit input blocks, not subject transforms.

Seven exact inventory recipes bind those complete returned objects to their
actual plans, precincts, styles, planes, coordinates, fractions, selected counts,
detail indices and decoded-byte cap. Negative tests reject incorrect listed
models, unrelated or partial results, constant/reordered native inputs and
success-only substitutions. The private with-kernel implementation remains a
context link, not an unconditional Some-kernel contract: a regression explicitly
rejects that promotion. No generic expression or implication matcher was added.

F6 remains open: Some-kernel/arbitrary profiles, general MCT paths and the actual
whole-decoder composed
coefficient-to-component-sample bridge are not
established here. Whole tile-sample-to-image assembly does not close those gaps.
See the [reconstruction model reading path](specs/README.md#reconstruction-reading-path).

### General scope

Verification checks the properties expressed by contracts and loop invariants,
including bounds, arithmetic checks, termination, and selected functional
relations to specification helpers. The technical inventory
[normative-coverage.json](normative-coverage.json) links implementation,
contract, specification, proof, and test symbols.

That inventory is not an exhaustive normative-clause checklist. A helper
contract is only as strong as its model and its relationship to the production
state/output. Review those relationships independently; symbol links and
successful obligation counts alone do not prove full standards conformance.

### Concrete contract boundaries

Read each contract according to its inputs and postcondition:

| Contract family | Established relation | Not established by that relation alone |
|---|---|---|
| Stream and packet accounting | Consumed offsets, declared lengths, ordering and bounded claims | That accepted syntax has the intended pixel meaning in every standard edition |
| Scalar refinement helpers | The implemented bounded arithmetic/bit operation agrees with its listed model | That callers chose the normative context, stencil, scale or traversal order |
| JPX source-over arithmetic | Exact natural-valued numerators and rounded scalar color/alpha results, proved in the 16-bit range at production conversion sites, with upward ties and transparent zero | Layer ordering, placement, sampling, persistence, or a complete composed-image theorem |
| Complete finite-channel pixel evaluation | One total model for all active colors and alpha, preserved unused slots, and exact invalid-input rejection for up to three normalized color channels | A complete theorem for planar gather/scatter, canvas traversal, layer order or whole-render transactionality |
| JPX planar pixel gathers | The complete optional pair of stack pixels, alpha values and representation flag equals the model of the actual source/destination planes and indices, including grayscale expansion, absent source alpha and all missing-sample rejection cases | That earlier channel-map resolution is normative, or that subsequent scatter, canvas traversal or layer/frame composition is completely correct |
| JPX planar scatter | Exact status and complete final color/alpha sequences, including the successfully written color prefix on a failed raw scatter, preserved other samples and plane lengths | That an arbitrary caller prevalidated every destination, or that an entire render is transactional |
| Complete normalized planar pixel transition | The actual gather/evaluate/scatter status and entire final planar state equal one composed model; rejected pixel inputs leave both color and alpha state unchanged | Earlier channel normalization, complete canvas traversal, work-limit transactionality, layer/frame order or a parser-to-pixel theorem |
| Successful placed-layer composition | On success, the complete final planar canvas state and exact work count equal a raster-prefix fold of normalized pixel transitions at the selected clipped, floor-scaled crop addresses | Complete error-state semantics, sufficient-budget success, earlier channel normalization, layer/frame ordering or a parser-to-pixel theorem |
| All-return placed-layer prefix safety | On every return, all buffers equal a valid raster prefix, and work accounts for that prefix or all but its final stored pixel; inactive geometry leaves the complete initial state unchanged | Which prefix must fail, exact error classification, sufficient-budget success, normalization or layer/frame ordering |
| Deterministic budgeted placed-layer execution | On every return, the exact typed `Result`, complete final buffers and work counter equal a raster fold that stops at the first checked-address, invalid-pixel, work-overflow or actual work-limit failure | Earlier channel normalization, layer/frame ordering, parser-to-pixel correctness or official conformance |
| Supplied normalized layer sequence | `render_decoded_layers` returns exactly `part2_render_layers`: global output-space choice, transparent canvas initialization, ordered cumulative byte checks, carried placed-layer buffers/work, first failure and complete final image metadata/samples | Which compressed layers/instructions are supplied, normalization/registration/opacity/chroma-key correctness, or complete frame/timeline orchestration |
| Supplied frame-table selection | `select_frame_instructions` returns exactly `part2_select_frame`: finite/indefinite modulo selection, exact selection errors, ordered persistent predecessors/current frame, and all instruction fields | Building the frame table, interpreting reuse, or a composed selected-frame-to-image theorem |
| Complete timeline construction | `build_timeline` returns exactly `part2_build_timeline`: repeated instruction sets, new/reused layer references, exhaustion, grouping/persistence, all frame/instruction metadata, checked timing and exact frame/work/arithmetic/conflict errors | Parsing life/tick fields or deriving the supplied composition metadata from compressed/container bytes |
| Complete supplied-layer frame composition | `render_normalized_frame` returns exactly `part2_render_normalized_frame`: actual selection, indexed normalized outcome lookup/consumption, first supplied-layer failure, then complete budgeted rendering and final image | Compressed decoding, earlier normalization/registration/opacity/chroma-key, or a parser-to-pixel theorem |
| Normalized image construction | `from_layer`/`from_composited` return exactly `part2_image_from_planes`: checked dimensions/plane cardinalities, exact errors, 16-bit precision, ordered channel kinds, whole-image alpha association and all buffers | The origin or correctness of supplied normalized samples |
| Complete converted-image layer construction | Exact typed result, dimensions, ordered color planes, optional sequentially combined alpha and representation flag for actual borrowed selection and owned copies | Earlier component/color conversion or association validation, registration/opacity/chroma-key, layer/frame ordering or a parser-to-pixel theorem |
| Complete typed JP2 color conversion | Every typed result, image field and color/alpha sample equals the composed model of the actual typed components, definitions, reference grid, color method and actual byte/work caps; includes complete preflight and fail-fast raster/channel traversal | Earlier compressed decoding, palette/container interpretation, selected-image crop or JPX registration/opacity/chroma-key; ICC methods are explicitly rejected, not executed |
| JPX destination stores | Exact success flag and final slice: the indexed sample is set to the requested value, all other samples and length are preserved; an invalid index leaves the slice unchanged | That the selected plane/index is the normative destination, or that an entire render is transactional |
| JPX placement coordinates | The selected floor-scaled source coordinates remain inside the cropped source, clipped destination coordinates remain inside the canvas, and actual read/write indices equal checked row-major addresses | Complete canvas traversal/channel routing, all resampling profiles, or an edition-level placement/conformance theorem |
| Tier-1 state and reconstruction | Selected pass-state, magnitude-prefix and integer-midpoint operations | A complete mathematical denotation of every code-block and inverse-transform combination |
| Complete raw-bypass reader transitions | Actual reader initialization, byte loading, bit result, every final field, exact input/work/overflow errors and predictable-termination result | Complete MQ decoding, packet/segment accounting, significance/refinement/cleanup traversal, coefficient meaning or end-to-end legacy correctness |
| HT directional reader/substream state | All five actual readers: exact constructors, bytes/bits/positions, MagSgn words and drain, zero/one fill, stuffing, exhaustion, full cursors/counters, shared cleanup byte claims and refinement physical-bit vectors/padding, exact inner/outer budget and overflow effects | Upstream partition/pass accounting, UVLC/quad interpretation, coefficient/block reconstruction, replacement/placeholders, mixed sequencing or complete F5H acceptance |
| HT cleanup multi-bit/event operations | Every `u8` VLC peek/advance count, non-mutating copied-reader peek truncation, fail-fast actual advance, complete MEL event cache/head/suffix/adaptation and full-width event-counter failure state | That callers request the normative symbols, select the right table/context, reconstruct full cleanup blocks or route mixed pass groups correctly |
| HT UVLC/quad-word interpretation | Actual prefix, all native suffix lengths, prefix value, initial/non-initial U pairs and quad words, with exact typed result and complete final reader/event/claim/work state; actual table selection, context-zero MEL suppression, length validation, advance and tick order | Normative codebook endorsement, context selection by the quad traversal, complete metadata/coefficient blocks, placeholders/replacement or mixed sequencing |
| Actual packet-pass placeholder grouping | Every native packet-pass input: exact typed rejection outside 1..=164 and all four cleanup/refinement/placeholder/skipped-plane fields inside it | Actual pass execution/replacement, segment partitioning, coefficient planes or mixed dispatch |
| Complete actual HT cleanup metadata traversal | Exact public/segment result for all native header values and actual byte/work limits, ordered row/pair/member quads, north/west/east contexts, suppression/UVLC/events, sample bounds/counts, collision, work/counter first errors and every metadata field | Numerical magnitude/sign/exponent or coefficient-plane reconstruction, refinement passes, placeholder/replacement execution, mixed sequencing or full F5H |
| Complete actual HT numeric MagSgn reconstruction | Every native parameter/flag and initial reader state: exact full 384-bit numeric magnitude, sign, exponent, typed first error and all final reader fields; actual full-value arithmetic through F6 contracts | Plane placement, raw high-limb storage, exponent stencils, final plane padding/termination, refinement or mixed sequencing |
| Complete actual HT cleanup-plane reconstruction | Every raw output coefficient and all six words/flags, actual constructor and metadata rejection, coordinate-overflow-before-clipping, ordered numeric consumption/placement/counters, both full exponent stencils/reset/swap and final physical padding/stuffing/work termination; no valid-metadata, matching-shape or numeric-success premise | Refinement passes, replacement/placeholder/mixed sequencing, a composed metadata-plus-plane decoder theorem or complete F5H |
| Earlier cleanup-only symbol stage, now subsumed | Actual metadata output feeds numerical reconstruction and complete duplication; decoder state and ordered accounting facts remain reusable native stages | This earlier stage alone did not establish supplied refinement or byte-input preflight; the complete entries below now do. Continuation/replay, placeholders/replacement/mixed sequencing and full F5H remain separate. |
| Complete actual HT significance/refinement plane | Both one/two-pass paths, all native header values and raw six-word states: actual reverse magnitude adjustment, stripe/neighborhood/causality, evolving discovery plane, per-patch delayed signs, every coefficient/flag/statistic, complete readers/claim/stuffing vectors and first-error/final-padding effects | Refinement continuation/replay, placeholder/replacement/mixed sequencing or full F5H |
| Complete substantive HT segment byte entry | Actual style/pass/set admission, real pass/byte/work caps, parser partitions, internal metadata-to-numeric-to-refinement composition, complete decoder state and exact ordered failures, including `None` versus `Some(empty)` | Separate refinement-contribution replay, placeholder skipping, replacement/multiple-group orchestration, mixed legacy dispatch, normative endorsement or whole-decoder/F5H completion |
| Complete retained HT refinement contribution | Actual retained-buffer clone/append and size/cumulative-byte admission, replay from the retained cleanup plane, native pass-group checks, empty-concatenation bypass, full output/data/control state and every transactional/late partial accounting error without normalized-state premises | Staged packet-group clone/commit orchestration, mixed legacy dispatch or full F5H |
| Complete native HT placeholder/replacement step | All native pass/phase/set states and real pass caps, exact cleanup-flag/buffer reset, preserved stored cleanup/output, multiple-set permission/counting and partial segment-overflow effects | Entire staged multiple-group/packet or mixed dispatch; no assumed Clone/legacy outcome is supplied |
| Raw Tier-1 producer interfaces | Actual decoder plane, raw dimensions/sequence/indexed result even for malformed shapes, full stored words/flags, exact legacy-midpoint signed magnitude and native arithmetic rejection; no normalized-coefficient premise | Entropy meaning, coefficient normalization, complete reconstruction coordinator or end-to-end pixel correctness |
| Decoded output shape | Nonempty image/component counts and rectangular sample cardinality | Pixel equality with the JPEG 2000 decoding process |
| Representation views | Mathematical descriptions used by the referenced lemmas | An automatic connection to all production state or end-to-end output |

The earlier cleanup-caller inventory retains three unconditional native stage
contracts: complete preparation, complete decoder store state and complete
plane duplication. Their exact result/model, old/final decoder view, actual
planes, counters and work-cap arguments are checked through the existing
conservative mechanisms. Negative tests reject unrelated, partial, reordered
or constant bindings. The actual symbol caller is now unconditional for both
absent and supplied refinement and is linked as a complete contract by the new
substantive-byte-entry profile, not falsely promoted from its old conditional
theorem. The pass-limit owner projection only specifies the existing getter;
its runtime body and policy are unchanged.

Twenty-seven finite exact result/model/view recipes bind all actual refinement
and byte-entry producers. Every mutable receiver or summary must appear as
complete final state, with the correct initial state, full-value magnitude,
coordinates/stripe/patch arguments, causal flags, pass/bitplane values and real
pass/byte/work caps. Refined statistics bind field three/count one, and merge
binds the complete actual other record. Input patterns are anchored to the
matched returned-result clause: a later correct occurrence of a model cannot
mask incorrect primary arguments. Actual duplicate-clause and first-only
mutation regressions cover that distinction. No generic expression/implication
matcher or mutable-state relaxation was introduced.

`normalize_component` has an unconditional complete result relation for
supplied typed integer components, target dimensions and reference-grid
origins. It covers every output sample, either signed/unsigned representation,
declared precision 1..38, fixed-point registration, subsampling, edge clamping
and the exact first typed error. Invalid precision, sampling, dimensions,
signedness tags, missing/out-of-range samples and checked coordinate/index
overflow are modeled rather than assumed away. Empty target height returns an
empty plane without sampling; zero target width is rejected first, preserving
the actual stage's behavior. This does not prove the earlier palette/channel/
container interpretation, full color-layout/sYCC/alpha pipeline or parser-to-
pixel relation. Public budget checking remains an earlier separate stage.

`channel_layout` now has a complete typed-result relation over the actual
ordered component/CDEF metadata. With no definitions it requires one component
per color and returns identity routing. Explicit definitions preserve ordered
alpha associations and flags, ignore unspecified channels, allow repeated
identical color assignments within one component, reject cross-component
conflicts, and check missing required colors only after the entire definition
scan. Invalid alpha-association errors therefore keep their actual precedence
over a later missing-color error. These contracts do not independently prove
that a container parser authored the normative definitions or close the
subsequent whole color/alpha conversion.

`normalized_at` and `normalized_difference_at` are connected to the actual
typed-image component sequence and origins, including component lookup and
all nominal sampling errors. Chroma differences subtract `2^(p-1)` and scale
by `65535/(2^p-1)`, rounding signed half ties away from zero. `sycc_to_srgb`
returns the exact separately rounded, clamped RGB triple for every native
`u16`/`i32` input, without a restricted-chroma precondition. The checked
`rounded_fixed` helper also retains both `i128` overflow boundaries. These
pixel results do not close the earlier component/output-budget validation or
the complete public color-converter channel/raster traversal.

The separate public `convert_jp2_color` contract now composes these relations
as `color_convert_jp2`. It has no valid-input, canonical-channel, restricted-
chroma or sufficient-budget precondition. Zero image dimensions precede
format selection; unsupported formats precede layout validation; layout
precedes component metadata; component metadata precedes output-byte/work
admission; and admission precedes every sample read. The complete sYCC raster,
ordered grayscale/RGB channels and all alpha channels retain their exact first
failure, including original association/premultiplication metadata. Output
dimensions, color space, precision and every sample are connected to that one
result model.

Component validation covers precision/tags, subsampling, registered spans,
dimensions and sample counts. Output admission retains native checked-count
overflow and exact `DecodedBytes`/`WorkUnits` limit/actual fields, with bytes
checked first. These are this stage's admission quantities, not a theorem
about global cumulative work or peak live allocation. The normalization
allocations retain ordinary Rust allocator behavior; allocation failure is
not invented as a typed decoding result.

The shared private channel/pixel helpers contain the original production
operations and are called by the real converter, not an alternate decoder.
The preflight invariant bundle is internal proof support under
`proofs/internal_specs.rs`; essential format, validation, traversal and error
semantics remain under `specs/`. Independent wide-integer tests compare 1,440
complete supplied-input images and metadata plus 180 native-domain admission
cases. Those direct typed-input tests are not new reference-qualified
compressed profiles. Palette/channel-map interpretation, container selection/
crop, earlier decoding and normative endorsement remain separate F7/F8/N1
obligations.

`ConvertedImage::crop` now has an unconditional complete image-result
relation to `color_crop_image`, including the actual selected-color caller.
Zero requested dimensions precede coordinate overflow, which precedes bounds
checking. The existing exact whole-image identity path returns the supplied
image without allocation or new plane validation; even an internally supplied
short plane is retained unchanged. Other rectangles copy only requested
samples, in channel/row/column order, with exact address and missing-sample
errors. Color space, precision, every channel kind and alpha association/
representation flag are preserved without a canonical-channel premise.

The private plane helper shares the original two-dimensional production
loops rather than replacing them with a slower division-per-pixel raster.
Its only rectangle-order premises are established by the real method's
checked geometry; no valid sample plane is assumed. Essential crop
denotations remain in `specs/`; the immutable preflight bundle alone is in
`proofs/internal_specs.rs`. Source-coordinate-mask tests cover 86,700 bounded
contexts, alongside explicit native-boundary and identity-bypass cases.
This proves cropping supplied converted pixels, not how the decoder chose
the requested region or produced those pixels.

The actual `convert_selected_color` caller now refines
`color_convert_selected` from supplied typed components, color method, target
rectangle and actual limits. It composes the real converter and crop
contracts instead of assuming an unconnected converted image. Format,
metadata, budget and sample errors precede target-coordinate subtraction;
invalid target subtraction retains the existing `InvalidCodeBlock` error,
while valid differences pass to the full crop semantics. Earlier compressed
decoding, palette/channel-map application and selection-target derivation are
not established by this typed-input composition theorem.

`layer_channels` and `owned_layer_from_image` refine `part2_layer_input`
and `part2_owned_layer_input` on every return, without assuming canonical
channel order or valid plane lengths. Their result projections include every
output field and preserve the exact error variant. Channel scanning precedes
area checking, required-color resolution and alpha validation. Red and gray
both select slot zero, and duplicate or out-of-range slots are rejected.
Alpha planes are combined in source order, rounding after **each** product;
one alpha retains its premultiplication flag, while multiple alphas produce
straight representation. Association metadata is already an input to earlier
conversion policy and is not revalidated by this stage.

The private constructor is a free function because the pinned `verus_spec`
attribute cannot infer an associated static method in this conditional-macro
layout. There is still one production implementation and no public API change.
The existing placed-layer theorem uses a fresh solver context
(`spinoff_prover`) to avoid accumulated solver-state sensitivity as the model
grows; its obligations and resource limit are not relaxed.

### F3 input-relative orchestration boundaries

The actual layer renderer now carries a deterministic sequence denotation
across every placement; no validity, budget sufficiency or trace assumption
is required. The render invariant includes every mutable buffer and both
counters; the byte-charge and placed-layer contracts cover **all** returns.
Byte charging precedes placement validation;
placed-layer stores/charges retain their existing error ordering. The final
image constructor is proved rather than treated as an unmodeled adapter.
The constructors are free functions for the same pinned attribute limitation
as the F1 owned-layer constructor; their bodies remain production code.

The internal renderer receives the byte cap explicitly. Each production
caller supplies `options.limits().maximum_decoded_bytes()`; the work cap is
still related directly to `ResourceLimits` by `resource_work_limit`.
This avoids adding an unproved/private-field limit accessor assumption or
changing the shared limits module. The theorem is relative to the supplied
byte cap and the actual work-limit view, not a proof of all caller code.

`build_timeline` now has an unconditional all-return contract, not merely
an instruction-step lemma. Its model starts from the actual empty state,
folds complete instruction sets and their repetitions, stops on layer
exhaustion, and absorbs the first error. The state includes the full pending
reuse schedule, next new-layer index, instruction position, frame table and
time. All instruction fields and frame indices/start/duration/persistence
are projected. The whole function proves its helpers' structural invariants
from initialization; that invariant-only predicate lives in
`proofs/internal_specs.rs`, not in the normative definitions. Helpers'
partially updated private state on error is not exposed by the result-only
timeline theorem. No successful frame state is abstracted away.

`render_normalized_frame` is the complete composition boundary. Its supplied
input is an **indexed outcome bank**, not an assumed execution trace: each
entry contains a layer reference and an optional normalized layer `Result`.
For each selected instruction, the actual collector consumes the first
remaining matching entry; arbitrary bank order, duplicate references,
unrelated entries, empty slots, errors and missing references are all modeled
without preconditions. Duplicate references consume successive outcomes
rather than silently caching one decoded layer. This is necessary to preserve
the existing per-placement decoder-call behavior. Missing references return
`InvalidBoxContents`; supplied failures retain their exact error variants.
All selected outcomes are gathered before rendering, so a later supplied
failure precedes an earlier layer's pixel/resource failure, as in production.
The actual complete selection and layer-render contracts establish the final
image, rather than assuming their outputs as trace inputs.

`render_inspected_frame` still decodes selected references in the previous
order, stopping at the first decoder error. It supplies those outcomes to
the proved normalized stage, which independently performs frame/reference
selection and propagates that first selected error before rendering. The
wrapper does not cache, skip or add compressed decoder calls. The extra
instruction selection is deterministic and has the same existing selection
errors; compressed decoding itself has no newly asserted denotation.

Independent tests compare 11,520 bounded timeline programs with a separate
flattened/BTreeMap scheduling and grouping model, including exact typed
errors. Frame tests cover every four-frame persistence mask, finite/indefinite
looping, extreme requests, indexed bank permutations and repeated references
with distinct normalized outcomes. Full multiframe images are compared with
wide-rational arithmetic, including metadata, alpha representation, complete
buffers, repeated-call determinism and loop reset. Dedicated cases establish
selection-before-supply and gather-before-render failure precedence.

These complete actual functions cover F3 relative to supplied composition
metadata and normalized layer results. They do **not** establish compressed
decoding, container-to-metadata interpretation, normalization, registration,
opacity or chroma-key semantics, nor standards-edition conformance. Owner
integration/review and package acceptance remain separate from these local
theorems. All complete output projections have exact inventory-scanner
mappings and linked `contract_refs`, including `build_timeline` and
`render_normalized_frame`.

The raw-bypass reader has unconditional exact typed result/state relations
for byte loading and bit reads, plus exact termination classification.
Its state model includes the immutable byte sequence, current byte, remaining
bits, consumed-byte index, marker-padding count and actual work counter/cap.
Stuffed data use seven bits; marker-like bytes beyond `0x8f` after `0xff`
remain unconsumed while synthetic `0xff` bits are supplied. Exhausted input
uses the same selected bounded-reader policy. These are input-relative
implementation semantics, not a completed selected-edition audit.

The frozen first F5H subprofile is documented in [specs/README.md](specs/README.md#frozen-f5h-directional-reader-stages).
It comprises `HtMelBitReader` and `HtReverseVlcReader` construction, loading,
single-bit reads, positions and counters, plus the actual shared cleanup
pair's constructor, `can_claim_byte`, `tick`, `read_mel_bit` and `read_vlc_bit`.
The three constructor bodies are shared private free implementations because
the pinned conditional attribute cannot infer associated static context.
The existing constructors remain thin Rust-tested adapters; no additional
executable decoder or trusted codec specification is introduced.

The mathematical transitions consume the actual input sequences. Their
postconditions specify the exact typed result **and every final field** on
both success and failure, without restricting the `u64` counter range.
VLC's terminal nibble starts at bit four; reverse stuffing excludes bit seven
only in the selected later-byte context. VLC charges the bit before eagerly
loading its predecessor, so the first rejected work charge leaves the old
byte and advanced cursor. MEL forces the low terminal nibble to ones before
testing its distinct stuffing rule. Invalid MEL stuffing leaves the entire
reader unchanged. Exhaustion and counter overflow retain their actual partial
state effects rather than being modeled as rollback.

The cleanup wrapper contracts compose those actual transitions, byte-claim
checks and outer accounting with complete nested-reader and MEL-event frames.
A collision precedes consumption; an inner failure does not record a claim or
charge outer work; an outer failure retains the consumed bit and recorded
claim. Mathematical prefix definitions and checked first-error absorption
lemmas describe fail-fast clients; they do not assert that callers retrying
after an error leave state unchanged. Independent fixed bytes and literal
expected bits/full cursors cover terminal stuffing, exhaustion, constructor
capacities, inner/outer budget order, collisions and full-width overflow.

The subsequent complete directional substages also cover the actual MagSgn
constructor, little-endian word loop, physical/fill/work counters and
bounded finish drain. The models include zero-count no-op, oversized-count
rejection, stuffed `0xff` synthetic fill, bad padding consumption and every
ordered overflow/budget effect. Finish checks the native `input.len() * 8`
fuel before testing pending physical data. Its semantic recurrence is not
an invariant-only witness and does not assume termination succeeds.

Both actual refinement readers have exact positioned and public bit-read
contracts. Their pair's full state includes both complete readers (including
an absent reverse reader), the entire claim vector and the entire stuffed-bit
vector. Reverse stuffed-bit recording precedes claim collision, so rejection
can retain both a consumed bit and a new stuffed mask. Work is the checked
sum of physical/fill counters after selected-counter charging. Padding finish
checks all encoded bytes without changing any state. A lexical mutable-borrow
scope and extensional vector frames make the existing claim mutation visible
to the verifier; the two public result projections use equivalent explicit
matches rather than a standard-library `Result::map` contract. No decoding
algorithm is duplicated or changed to suit an inventory expression.

The actual VLC peek/advance loops and MEL event machine refine semantic
recurrences for every native count/cache/counter input. Peek intentionally
returns a partial `u16` on copied-reader failure or result-width exhaustion,
without changing the real reader. Its loop distinguishes break-only results
from normal continuation. MEL cache priority precedes state validation;
event-counter overflow retains cache changes, and suffix-read failure retains
only actual nested reader/claim/work mutations. A literal maximum-exponent
fixture checks the five-bit MSB-first suffix and subsequent cached events.

These are complete **selected directional/multi-bit/event substages**, not
full-block or F5H completion. Upstream segment partition/accounting, UVLC/
quad interpretation, cleanup/significance/refinement block denotations,
placeholder/replacement sequencing and mixed routing remain required.
The inventory uses nineteen additional exact constructor/mutating-view
recipes, with fail-closed tests for each return binding, complete view,
old/final snapshot and function/model pairing. Scope lookup, mutable-parameter
completeness and expression matching are unchanged. Sixteen contracts have
actual internal production callers and are indexed. Three standalone public
APIs (`HtForwardMagSgnReader::read_bits` and both public refinement `read_bit`
adapters) are proved and their recipes tested against real API tests, but have
no internal decoder callers. They are not disguised as internal helper calls
or indexed through a wrong same-named method. The exact-root verifier checks
all these actual contracts independently of the non-exhaustive source index.
The next complete interpretation stage connects the actual UVLC prefix,
suffix, combined value and both U-mode decoders to the complete cleanup
reader state. Every native mode/length is covered, not just successful
prefixes: a true suffix bit above bit seven is rejected after consumption,
whereas zero bits above the result width may continue. Read failures precede
that width check. Initial mode three retains its special long-prefix route,
and both modes retain their exact prefix/suffix read order and checked
arithmetic failure points.

The actual quad-word caller refines lookup into closed views of the unchanged
production tables, then context-zero MEL suppression, codeword validation,
advance and final tick. No table words/notices are copied or modified.
Equivalent explicit boolean/checked-add conversions and one shared checked
byte-conversion helper replace opaque standard-library callback composition;
they do not introduce a second executable interpretation algorithm.
U-pair results retain their native two-element arrays. Extensional content
and exact array equality use the existing pinned `vstd::array` equality fact,
not a new codec axiom or trusted escape. Prefix opacity only keeps caller
composition modular; actual producer bodies explicitly unfold their models.

The authorized nineteen directional and six interpretation registry recipes
are complete. The six interpretation contracts use the same exact whole-state
shape, preserving actual extra arguments and native tuple/array/word results;
their models, true production callers, proofs and fixtures are indexed.
Neither this source index nor successful local gates close full-block or
mixed sequencing. The models and a green solver run are not human endorsement
of T.814.

`HtPassGroup::from_packet_passes` now shares one verified free implementation
that returns the complete actual four-field group, retaining the static
public wrapper and all original native bounds/error behavior. This is an
input-relative grouping theorem, not placeholder/replacement execution.

The complete actual public segment parser now refines `ht_parse_block_segments`
through a public closed outcome projection of every result field. The sole
authorized foreign edit supplies the owner-local `resource_code_block_byte_limit`
view and exact getter postcondition. The parser model therefore uses the actual
selected `ResourceLimits` value, not a chosen or existential budget. Size
rejection precedes the exact CodeBlockBytes failure, which precedes Scup
interpretation and partitioning. All encoded/substream bytes, offsets,
directions, Scup and total bytes are included, retaining the observable
distinction between absent and present empty refinement. No public-parser
wrapper or successful-input assumption is introduced. Its complete outcome
shape is proved by the root. The non-exhaustive source index binds this
function to its exact outcome/model recipe and actual caller; negative tests
reject unbound, partial, swapped or unknown outcomes/models without any
matcher widening.

Existing public F6 limb/value, constructor, zero-test, right-shift, exact
checked-add, full-domain checked-left-shift and low-word contracts are now
available in this combined branch. The actual HT numerical reconstruction
consumers still need their complete result/reader-state relations connected
through these interfaces. No Tier-1-local magnitude implementation or trusted
shim is introduced.

### Complete cleanup metadata boundary

`decode_ht_cleanup_segment_metadata` now refines
`ht_cleanup_segment_metadata` on **every return**. The public
`decode_ht_cleanup_metadata` additionally composes the actual segment parser
with its real code-block byte limit before the metadata header/work stage.
There are no stream-validity, decode-success, sufficient-budget or selected
trace preconditions on either entry point.

The internal producer is indexed through its exact complete outcome/model,
actual segment view, width/height/maximum-U arguments, actual work-limit
snapshot and scoped coefficient-symbol caller. Fail-closed mutations reject
swapped inputs/limits, unbound/partial outcomes, wrong models and wrong
callers. The separately verified public facade currently has only API-test
callers and is not falsely attributed to the internal block path.

The semantic reading path includes complete deterministic word/member,
sample/quad, pair and row folds in `specs/mod.rs`, with full typed first-error
propagation. Checked step/absorption lemmas justify the actual loops. The
source is still one executable traversal: the existing bodies are moved to
word, append, pair and row helpers, not copied into a proof-only decoder.
Their internal geometry and allocated-length facts are established from
the actual header and loop bounds; reader/event/work and significant-count
domains are not restricted to successful states.

The helpers expose complete disjoint mutable frames. A failed word read
retains prior current-pattern/left-context updates and all actual reader
effects. An invalid significant edge sample retains earlier sample counts
but does not store a quad or tick. A counter error precedes the store; a
work error from the final tick retains the stored quad and updated count.
The row's previous-pattern vector is replaced only after the entire row
succeeds. Opaque prefix folds retain their exact first error and state.
Canonical unit-result postconditions are proved from the actual bodies to
compose these folds, not assumed from success.

All returned fields are covered: dimensions, every ordered quad field,
significant-sample count, MEL/VLC bit counts and MEL events, including final
collision and checked 32-bit counter conversion. Allocation requests and
tick/read order are preserved. The unused **private** cleanup constructor
adapter is removed; metadata and its two existing tests use the same proved
free implementation. Public reader constructors are unchanged. Three local
`too_many_arguments` style allowances keep immutable codec inputs and
disjoint mutable frames explicit; no verifier/trust/termination limit is
relaxed.

The four new HT-only state properties use fresh solver contexts
(`spinoff_prover`) so their finite mask/geometry queries do not perturb
unrelated existing incremental solver queries. Their bodies and all limits
remain checked unchanged; no F6/color proof is edited for solver sensitivity.

Independent literal regressions include nine suppressed quads across odd
rows/columns/pairs, exact 24-unit success versus 23-unit last-store failure,
each actual northern/western context input, invalid edge-sample partial
counts, native counter overflow before/after stores, and public
parser/header error priority. Existing nonzero, fused-byte, UVLC and
collision fixtures remain in place.

This complete metadata boundary is independent of numerical magnitude
reconstruction. The F6 owner has now supplied the requested full-domain
interfaces separately in `f7807fe3bc6b2d3ccc622e2d3998eb3a39927535`;
they are not merged or duplicated in this metadata head. A clean joined
baseline is required before consuming them for the next magnitude/plane
stage. Neither metadata nor its positive tests complete coefficient blocks
or mixed sequencing.

### Complete numeric MagSgn boundary

`reconstruct_ht_magnitude` now refines the deterministic
`ht_reconstruct_numeric_magnitude` result/state model on every return. Invalid
completed-U/bitplane parameters precede all reader operations. The actual
payload loop consumes LSB-first physical/stuffed/fill bits with the already
proved complete reader transition; every first read/counter/budget error
retains its actual final reader state. Sign and exponent are accumulated
alongside the full numeric value, not a low-word proxy.

Embedding, odd-center correction, centering, checked left scaling, right
division and zero rejection are one moved executable completion body using
the real F6 factories and full-domain arithmetic contracts. Its private
geometry facts are derived from the original public-entry checks; the
completion body itself admits every native 384-bit magnitude and exponent,
including capacity rejection. A checked modular-arithmetic lemma connects
the real low-word parity test to full-value parity. No arithmetic algorithm,
foreign source shim, assumed success or new limit is introduced.

The source index has a narrowly typed recipe for this one actual producer:
the complete numeric outcome and complete final MagSgn reader must equal the
listed model on the actual old reader and native arguments. The existing
every-mutable-parameter check remains active. Tests reject missing/old/wrong
final states, unbound/partial outcomes, swapped inputs and wrong models; this
does not introduce general expression matching or an API-root exception.

Independent fixtures cover sign/exponent/scaling and inconsistent native
embedded flags, zero-bit reconstruction without touching even over-budget
reader state, all six limbs at bit 365, stuffing and physical/fill budget
errors with complete literal cursor/counter states, native counter overflow
and centering/shift overflow.

The next actual cleanup-plane boundary reaches `CoefficientState::set_magnitude`,
which uses the foreign `Magnitude::limb(index + 1)` to store all five high
words. The owner now supplies its exact full-index
`Some(magnitude_limbs(self)[index])`/`None` result and a public pure six-limb
shape fact while keeping the representation closed. The actual stores and
signed-result/plane consumers still need their complete relations through
those interfaces; no shifted low-word replacement, assumption or duplicate
limb algorithm is used. Plane reconstruction, placement,
stencils and final padding/termination are not claimed by the numeric
producer theorem.

### Complete cleanup plane and raw producer interfaces

`reconstruct_ht_cleanup_plane` now has a complete typed-result relation over
the actual supplied plane dimensions, raw cleanup descriptor, arbitrary
metadata, native maximum-U/bitplane values and actual work cap. Its blank
allocation does not assume or reuse old coefficients. The ordered quad/sample
fold includes checked coordinates before clipping, scan/read/placement/count
ordering, every raw six-word coefficient and flag, both complete exponent
vectors with reset/max/overwrite/swap behavior, and the final real reader
padding/stuffing/work check. Failed private transitions retain their exact
partial plane, reader, statistics and stencil state. This is not refinement,
replacement/placeholder/mixed execution or a metadata-to-plane caller theorem.

The raw coefficient/plane interfaces retain malformed shapes and full native
legacy midpoint tags, including insignificant stale words and significant
negative zero. Six thin ghost denotations express existing field/record
projections in the already supported complete-model contract form; original
numeric, shape and field facts remain available. The source index reuses its
exact known projection/reader recipes and every-mutable-parameter checks,
rather than adding a generic expression fallback.

The old two-field `SignedMagnitude::new` was crate-private and, after the real
signed producer's equivalent literal construction, used only in two tests.
That now-unused constructor is removed and those tests use the same literal
fields. No public API, decoder result, sign policy, arithmetic behavior,
dummy runtime call or lint suppression is introduced.

For raw bypass, error states are not transactional: work-limit failure preserves
the consumed bit and incremented work counter; work overflow preserves the consumed bit
and old counter. Marker-padding overflow preserves the already installed
synthetic byte/eight-bit count, without consuming the bit or charging work.
The constructor's private free helper is shared by the unchanged public
`RawBitReader::new` wrapper and actual legacy decode path; there is no second
executable reader. Its contract proves the initial state and exact input-byte
claim. These stage contracts do not complete the F5L packet/MQ/pass epic.

A separately reference-qualified layered/tiled RGB16 all-style regression
identified three termination corrections. Later completed coding segments
may have zero payload bytes; initial empty blocks, unfinished empty segments
and empty HT inputs remain rejected. Empty raw predictable termination is
valid only before any coded bit is consumed. A complete final raw `0xff`
may retain its exact seven-bit `0x2a` termination byte without that byte being
an entropy decision. The model and exhaustive terminal-pair regressions cover
these corrected outcomes; no oracle expectation, budget or cap was relaxed.

The production placed-layer compositor uses the complete `source_over_pixel`
evaluator. Its executable contract equates the entire optional color/alpha
result with one model: invalid channel counts or active premultiplied samples
above alpha return no pixel; valid inputs produce all specified rounded
channels and alpha, preserving unused array slots. The loop invariant connects
all processed channels and untouched slots, not just one arbitrary scalar.
Checked conversions remain in the evaluator, and its contract rules out a
spurious conversion failure on valid inputs.

Fixed-size stack buffers gather the supported grayscale/RGB pixel before
evaluation and scattering; no per-pixel heap allocation is introduced.
The production `gather_composition_pixel` contract covers the entire optional
input tuple, not just individual successful reads. Its model uses the actual
planar sequence views and indices: grayscale-to-RGB expansion reads source
plane zero for every active destination color, other cases select the matching
plane, absent source alpha supplies opaque straight samples, and inactive
stack slots are zero. Missing active planes, color/alpha samples or excessive
destination channel counts return no tuple; valid reads cannot spuriously fail.
The finite-channel loop preserves each processed read pair and every untouched
slot. The gathered tuple is passed directly to the verified pixel evaluator;
missing reads still become the existing typed invalid-layer error before any
current-pixel store or work charge. This gather relation alone does not
establish earlier layer channel-map resolution or mutable canvas/layer semantics.

The production compositor calls `compose_planar_pixel`, whose contract equates
the status, all final color-plane sample sequences and the final alpha sequence
with `part2_composition_pixel_transition`. That model composes the actual
planar gathers, complete pixel evaluation and planar scatter. Both mutable
outputs participate in the same equality; it is not just a result flag or
one stored sample. The projection uses the real nested vector views.
Its contract also explicitly states that a rejected pixel leaves both entire
outputs unchanged. Destination prevalidation in the gather rules out a
late scatter failure in this complete transition.

The raw scatter helper separately models all outcomes: excess channel counts
write nothing, a missing destination sample leaves the already stored color
prefix intact, and a missing alpha slot leaves all successful color writes
intact. Its model identifies every changed sample and preserves all others.
Its loop relates the complete current nested-plane state to the initial
state plus the processed prefix. Ghost snapshots and pointwise frame
assertions connect nested mutable borrows to that relation; those snapshots
and assertions erase from Cargo builds. No new trusted codec body is added.
The complete transition validates before invoking this raw scatter and has
the stronger no-change rejection property described above.

The independent read/rational/store oracle checks 159,744 shape/representation
cases and the entire resulting buffers, including valid and invalid
premultiplied samples; 864 raw-scatter cases check successful and partial
failure states, unchanged-value stores, missing slots and extreme indices.
The existing placed-layer rational/geometry and work-order tests remain.
The pixel models use pure nested sample sequences. Their production contracts
project actual vector views at the boundary, so the modeled resulting state
can feed the next pixel without reconstructing executable vectors.

The actual `composite_placed_layer` return contract is explicitly guarded by
`result.is_ok()`. On that path it equates all final color/alpha samples and
the final work counter with one complete placed-layer success model. The
row/column loops retain the exact raster-prefix state and the affine charge
`initial_work + completed_pixels * (color_channels + 1)`. The prefix model
selects every source coordinate with the existing crop/scale equation and
every destination with the actual row-major canvas address. Its base and
successor lemmas are proved and invoked from the production loops; the
recursive definition is opaque by default to bound solver expansion, not
trusted or skipped. No verifier resource budget was increased.

The cached alpha-slice/representation pair and grayscale expansion flag are
explicitly connected to the supplied normalized layer. The alpha cache
retains explicit option cases for that proof relation, with
a statement-local `manual_map` style-lint allowance; verification is not
suppressed. Empty source and
zero-target error priority remain unchanged. Empty clipped rectangles now
return before row traversal: a regression with `u32::MAX` unvisited rows
must return within five seconds while preserving even unvisited malformed
planes and an already exhausted work counter. This removes work-free empty
row iteration without adding new accepted pixel data. Work overflow and
work-limit rejection still occur after pixel stores, with their existing
counter behavior; exact tests preserve that ordering. The success theorem alone
does not specify partial buffers on `Err`, prove valid inputs always succeed,
or establish normalization, layer/frame order or standard-edition conformance.
An additional, unconditional placed-layer postcondition now constrains every
return, including `Err`: the complete output buffers equal some valid prefix
of that same raster fold. The work counter is the initial count plus either
all stored pixels' charges or all but the last pixel's charge. The latter
case preserves the existing checked-work-overflow behavior after a store.
Empty source, invalid zero target and empty clipping preserve all initial
buffers and work. A constructive prefix witness and raster bound are proved
and used before rejection, after stores, and after charging, so no error exit
is justified only by the success guard.

This is a prefix-safety theorem, not an exact failure-position or error-cause
model. It does not assert whole-render rollback or sufficient-budget success.
Independent multirow RGB regressions check every work-limit cut in a six-pixel
rectangle, a missing sample on the second row, an uncharged pixel on overflow,
and the exact-budget success case, comparing all stored and untouched samples
and the exact counter/error. Runtime instructions and error ordering are
unchanged; the new witness assertions erase from Cargo builds.
The compositor additionally has an unconditional equality to a deterministic
budgeted transition model. Its state includes the actual typed result, every
color/alpha sample and the work counter. The model checks address fit before
reading pixels, validates the complete pixel before storing, then checks work
overflow and the configured work limit in the existing production order.
It retains the stored pixel with the old counter on overflow, and retains
both stores and the incremented counter on a work-limit error. The recursive
fold stops on the first error; a verified absorption lemma proves later
nominal raster positions leave that exact error/state unchanged.

The work-limit model is a closed projection of the real `ResourceLimits`
field, connected by the executable getter's proved return contract. Local
typed error helpers and the limit checker have exact result contracts; no
new error kind or offset is introduced. Explicit propagation branches keep
their frame proofs before returning, with statement-local `question_mark`
style allowances rather than suppressed verification. The previous complete
success and all-return prefix-safety contracts remain separate postconditions.
An independent 1,260-case replay grid checks color/alpha truncation positions,
work-limit cuts, initial counters near `u64::MAX`, exact typed error precedence,
all buffers and counters using widened arithmetic. This establishes selected
normalized placed-layer execution, not earlier normalization, layer/frame
order, parser-to-pixel semantics or standard-edition conformance.
Malformed premultiplied data is rejected before destination writes for that
pixel, including a malformed later channel. This intentionally strengthens
the previous private intermediate-buffer behavior; public render APIs still
return the same typed error and do not expose a partial image. Earlier pixels
may already be written, and work-limit checks retain their existing position
after successful pixel stores. No whole-render transaction is claimed.
The test-only unplaced compositor remains a separate scalar reference path.

The evaluator and scalar reference share the source-over helpers. Ghost-only proof calls in the executable helpers
instantiate multiplication bounds; ordinary Cargo builds erase those calls.
The paired models establish alpha numerator at most `65535^2` and color
numerator at most `65535 * alpha_numerator` for straight samples or valid
premultiplied samples (`color <= alpha`). The division model establishes the
nearest-integer interval, including upward ties, for every positive denominator.
The executable division widens to `u128`, including for full-width `u64` inputs.
The helper postconditions identify their actual return values as natural
numbers, without truncating the models to machine words. Numerator bounds and
the nearest-integer interval are exposed in these executable contracts; a
proved bounded numerator also yields a rounded result at most `65535`.

The pixel evaluator's loop retains the alpha numerator relation and complete
processed-channel prefix as invariants. Its proof assertions establish that
each computed color and alpha result equals its composed scalar model and fits
`u16`, immediately before the checked conversions. The complete transition
passes those exact color/alpha results to the proved scatter.
Both color and alpha stores use the same executable checked slice-store helper.
Its contract equates the returned flag and complete final slice with a model of
the initial slice and requested update. Successful writes preserve every other
sample, including when the requested value equals the existing value; rejected
indices preserve the entire slice. The composed transition contract connects
the complete stored planar state, through the checked conversions, to the
source-over models. Rejected transitions propagate the usual typed invalid-layer
error; resource/work accounting remains in its original order, after successful
pixel stores.

These relations connect the pixel models and successful placed-layer canvas
traversal to production mutations, not just standalone arithmetic lemmas.
They do not prove earlier channel normalization, all caller index/channel
selections or layer/frame ordering correct. The canvas theorem covers the
selected successful placed-layer transition, not every failing render path.
No-change rejection of one store does not
establish transactional failure semantics for an entire render. Caller
rejection and checked conversions remain in place.
Placement uses the selected edge-aligned nearest-neighbor equation
`floor(position * source_extent / target_extent)`, with the full target extent
retained when the canvas clips the copy. Its executable helper proves the
natural-valued result and source-offset bound; the actual row/column loops
establish crop, canvas and addressing relations before passing the resulting
indices to the complete planar pixel transition.
The row-major helper computes `y * width + x` with proved `u64` intermediates
and a checked conversion to the platform's `usize`. The existing fallible
wrapper preserves its arithmetic-error behavior.

A flattening helper is not itself a coordinate validator: the placement loops
separately establish geometric bounds. Other callers receive the exact address
contract without a new blanket claim about their coordinate selection.
Geometric index bounds also do not prove that every supplied sample plane has
the declared cardinality; checked buffer access remains active. Empty-source,
zero-target and off-canvas behavior, and resource accounting order, are retained.
Independent rational-oracle composition tests are separate numerical evidence,
not a formal theorem about the entire compositor.

The `decode_output_shape_valid` and `component_output_shape_valid` helpers are
structural checks, not a codestream-to-pixel denotation theorem. Neither the
inventory nor the exact-root verification result establishes a complete
standards-level parser-to-pixel theorem. Numerical and metamorphic tests
exercise relations beyond these contracts; their evidence must remain
separate from formal guarantees.

Numerical agreement of irreversible transforms, fixed-point color conversion,
and nonlinear transforms requires separate conformance metrics and reference
outputs. The Rust/Verus toolchains, solver, and reviewed standard-library
contracts remain part of the trusted computing base.

## Changes to proof code

- Do not introduce codec `external_body`, `assume`, or `admit` escapes.
- Use explicit loop progress measures; do not bypass termination obligations.
- Add concrete postconditions/refinement evidence rather than disconnected
  model-only lemmas.
- Run the exact-root verifier and ordinary Rust tests.
- Review [supported-profile limitations](SUPPORTED.md) before strengthening
  public claims.
