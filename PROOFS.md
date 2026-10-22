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

These are input-relative integer/fixed-point stage results, not compressed
parsing/Tier-1 meaning or measured irreversible conformance. F6 remains open:
coefficient placement, actual multilevel/resolution scheduling, arbitrary
kernels, component transforms, decoder image/component conversion, component/
tile assembly and composed component-sample relations are not established here.
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
| Complete converted-image layer construction | Exact typed result, dimensions, ordered color planes, optional sequentially combined alpha and representation flag for actual borrowed selection and owned copies | Earlier component/color conversion or association validation, registration/opacity/chroma-key, layer/frame ordering or a parser-to-pixel theorem |
| JPX destination stores | Exact success flag and final slice: the indexed sample is set to the requested value, all other samples and length are preserved; an invalid index leaves the slice unchanged | That the selected plane/index is the normative destination, or that an entire render is transactional |
| JPX placement coordinates | The selected floor-scaled source coordinates remain inside the cropped source, clipped destination coordinates remain inside the canvas, and actual read/write indices equal checked row-major addresses | Complete canvas traversal/channel routing, all resampling profiles, or an edition-level placement/conformance theorem |
| Tier-1 state and reconstruction | Selected pass-state, magnitude-prefix and integer-midpoint operations | A complete mathematical denotation of every code-block and inverse-transform combination |
| Decoded output shape | Nonempty image/component counts and rectangular sample cardinality | Pixel equality with the JPEG 2000 decoding process |
| Representation views | Mathematical descriptions used by the referenced lemmas | An automatic connection to all production state or end-to-end output |

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
