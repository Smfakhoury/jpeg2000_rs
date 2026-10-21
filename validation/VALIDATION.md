# Decoder validation

This is a reusable decoder validation framework, not an ISO/IEC 15444-4
conformance claim or a publication approval system. It contains no official,
restricted, or standards-derived vectors.

## Running the checks

Use Python with the pinned `jsonschema` dependency in `requirements.txt`.
For example, from the repository root (installation is needed only once):

```sh
python3 -m venv .venv
.venv/bin/python -m pip install -r validation/requirements.txt
PYTHONPATH=validation .venv/bin/python -m unittest discover -s validation/tests -v
.venv/bin/python validation/run-validation.py generate --check
.venv/bin/python validation/run-validation.py verify
.venv/bin/python validation/run-validation.py inventory
.venv/bin/python validation/run-validation.py schemas
.venv/bin/python validation/run-validation.py all --no-external
```

`all` checks corpus reproducibility, schemas, technical inventory links,
generated decoder cases, independent numerical cases, and budgets. It builds the example harness with
`cargo build --example validation_decoder --locked`. It does not require
internal approvals, old results, or committed source snapshots. `--subject`
selects a different harness; `--no-build` uses the existing default binary.

All reports default to ignored `target/validation/`. The `run`, `budget`,
`official`, `numerical`, `htj2k`, `schemas`, and `all` commands accept `--output-dir`. Existing
tracked report paths are refused. Reports bind the Git commit, dirty flag,
current source and validation-tool hashes, and subject binary hash when run.
These describe the current execution, not the validity of historical evidence.
No command updates a committed source snapshot or creates an attestation.
The commands above leave tracked source unchanged.

CI initializes the crate's pinned Rust toolchain in a job-local `RUSTUP_HOME`
before building the native harness. This keeps validation independent of
preinstalled runner toolchain/component state.

### Native numerical platform gates

The separate native numerical matrix builds the same pinned OpenJPEG and
scalar OpenJPH source revisions on Ubuntu, macOS and Windows, then runs the
legacy/HT numerical commands and numerical/selection/container regressions
using each platform's native executables. This is independent numerical
execution, not an inference from portable Rust unit tests. Reports are retained
in platform-named artifacts, with source/tool/executable hashes and explicit
execution-platform and requested-limit metadata.

```sh
bash validation/build-openjpeg.sh
bash validation/build-openjph.sh
```

The builders require Git, CMake and a C/C++ toolchain, select Release for both
single- and multi-configuration generators, and reject modified/wrong-revision
source checkouts. OpenJPEG is pinned at
`a5891555eb49ed7cc26b2901ea680acda136d811` (2.5.0); OpenJPH retains its existing
0.33.0 pin. The OpenJPEG build enables the required PNM/PGX/RAW paths without
optional PNG/TIFF/ICC-engine libraries. Windows generators may place OpenJPH `.exe` files in a `Release`
subdirectory; CI requires exactly one matching executable before exporting
its configured tool path.

Windows capture uses bounded pipe-reader queues rather than POSIX pipe
selection. Timeout, combined stdout/stderr capture bounds and nonzero exit
status remain explicit; POSIX setup callbacks cannot be silently discarded.
Address-space limits are requested on Linux only; macOS rejects that setup.
File-size limits are requested where the POSIX resource API is available.
Unavailable limits are reported as `null`, including both on Windows.
These numerical checks do not claim identical
OS enforcement or measured memory guarantees: the separate GNU Time/RSS
resource gate remains Linux-only.

Per-component quantization snapshots use bounded heap storage sized to the
actual component count, avoiding large fixed-array copies on the default
Windows main-thread stack. `ComponentQuantizations` is `Clone`, not `Copy`;
borrow it for reads or explicitly clone an owned snapshot. A one-MiB-stack
decoder regression checks this on Linux, while Windows numerical execution
uses the native default stack rather than a larger configured stack.

## Reproducible corpus

`framework.py` deterministically generates 17 minimal raw codestream, JP2,
JPH, JPX, and JPIP fixtures, including negative, malformed-prefix, and
metamorphic cases. `corpus.json` retains byte lengths, hashes, recipes,
redistribution policy, expected outcomes, and metamorphic parents. `verify`
checks these bindings, path containment, identifiers, and unexpected files.
Intentional recipe changes can be regenerated with `generate` (without
`--check`); this maintenance operation updates the tracked corpus.

## Differential execution

The package-excluded example harness emits stable image/error observations.
Images normalize dimensions, component precision/sign, and integer samples;
errors normalize typed categories. Metamorphic cases compare normalized hashes.

`oracles.json` defines optional OpenJPEG, Grok, and JasPer command templates.
Use `--oracle-config` for another configuration. The framework neither downloads
nor bundles those programs. It checks `OPENJPEG_DECODER`, `GROK_DECODER`, and
`JASPER_DECODER`, then `PATH`. An absent unconfigured oracle is explicitly
`skipped`, not passed. An explicitly configured missing executable, crash,
timeout, or invalid output is a failure, not a skip or successful rejection.
External PGM/PPM images normalize to the same planar integer representation.

```sh
.venv/bin/python validation/run-validation.py run
.venv/bin/python validation/run-validation.py run --no-external --output-dir target/validation/local
```

`validation-results.json` records observations and oracle errors;
`disagreements.json` preserves acceptance/output differences. Stable
`oracle-disagreement-v1` IDs bind case, oracle, kind, and normalized hashes.
Unresolved differences make the run `blocked` (nonzero exit). An optional
external `--adjudication-file` can supply human decisions using
`oracle-adjudication.schema.json`; it is validated and never rewritten.
The generated `oracle-adjudications.json` is only a derived view. Without
external observations its status is `skipped`, not passed.

## Independent numerical regression checks

Install OpenJPEG's `opj_compress` and `opj_decompress` CLI tools (for example,
`libopenjp2-tools` on Debian/Ubuntu). The suite creates its own deterministic
nonzero 8/16-bit PGM/PPM images, including channel extrema, plus monochrome PGX
inputs retaining signedness and non-byte-aligned precision. It encodes and
decodes them independently and checks the Rust decoder's typed sample planes.
It also exercises 256-by-256 grayscale/RGB images with multiple code-blocks.

```sh
.venv/bin/python validation/run-validation.py numerical
.venv/bin/python validation/run-validation.py numerical --output-dir target/validation/numerical
```

Cases cover grayscale/RGB, raw/JP2, tiles and nonzero origins, multiple quality
layers, progression orders, every legacy coding-style flag, SOP/EPH/PLT, and
reversible/irreversible transforms.
PGX observations add signed 1/2/4/7/8/9/12/16/17/24-bit and unsigned
1/2/4/7/12/20/24-bit samples,
including signed JP2, odd geometry, tiled nonzero origins, vertical causality,
and layered raw-bypass coding. PGX storage uses one byte through 8 bits, two
through 16 bits, and four through 32 bits; nominal precision and signedness
remain separate metadata and are checked exactly. Both PGX byte orders,
payload lengths, declared ranges and the decoder's indexed component filenames
are validated.
For the selected sub-8-bit cases, PGX remains the original/reference format,
but the encoder receives unsigned RAW sample bytes with explicit precision
through `-F`. The pinned PGX/PNM input converters promote those inputs to
8 bits, so their direct output is not substituted for the intended low-bit
image. RAW inputs preserve the actual sample words and SIZ precision.
Reports identify `encoder_input_format`, actual input hashes and encoder
options, separately from the original source format.
Signed fixtures encode the offset unsigned source, whose centered transform
coefficients are identical, then author the signed SIZ flag and matching JP2
bit-depth flag. This avoids PGX input converters that recompute precision from
positive samples and discard the sign bit. Reports explicitly distinguish the
original signed source from the actual unsigned encoder input and identify
authored headers. No coefficient or packet-body bytes are changed. The
independent decoder must still reproduce the original signed metadata and
samples exactly; a rewritten header alone cannot qualify a fixture.
Selected signed/unsigned 25-bit profiles qualify at zero DWT decomposition,
including layered raw-bypass, odd tiled geometry and signed JP2. Main-header
segments are traversed by their lengths, including inside JP2 wrappers.
The actual COD decomposition level must match the case; declared quality-layer
and coding-style options are also checked against COD. Marker-like bytes
inside comments do not satisfy required RGN/NLT checks.
Lossless cases must reproduce the original and reference samples bit for bit.
The generated stream must first independently decode to the original geometry,
precision, signedness and samples; a changed reference is a failure, not a new
expected image. PGX origin cases use equal nonzero offsets; asymmetric origins
remain covered by the PNM cases. The generated matrix does not qualify
25-bit DWT transforms, precisions above 25, every intermediate precision/style,
or high-precision multicomponent images. Wider full-range probes that failed
the independent original/reference roundtrip were not adopted as expected
outputs or successful cases.
The layered bypass case also covers a zero-byte contribution completing a
buffered MQ segment. Such a continuation must retain and replay its bounded
prefix; it does not make an empty initial legacy segment valid.
Lossy cases record per-component maximum error, MSE, RMSE and PSNR, plus error
against the original image. PSNR is `null` when MSE is zero, rather than a
nonfinite JSON value.

These explicit **engineering regression budgets** compare against OpenJPEG;
they are not official Part 4 numerical tolerances:

| Irreversible case | Maximum sample error | Maximum per-component RMSE |
|---|---:|---:|
| 8-bit grayscale | 1 | 0.25 |
| 8-bit RGB | 2 | 0.5 |
| 16-bit grayscale | 8 | 2.0 |

`numerical-results.json` retains every failure and measured disagreement with
source, input, encoded-stream, output and executable hash bindings. Successful generated images and encoded files are temporary; recipes and
runner source are tracked. Failed inputs, encoded streams and reference images
are retained under the ignored output's `numerical-failures/` directory and
included in CI artifacts for reproduction and minimization.
Native execution has time, output and POSIX address-space/file-size bounds.
Reports include encoder/reference/subject timings and total case time. Each
native stage defaults to ten seconds; the aggregate case budget is three
times that timeout, including normalization. Output capture is limited to one
MiB and files to two MiB, with 512 MiB address space where available.

### Nonuniform component grids and selected color

Container relation tests generate their own planar 8/16-bit 4:2:0 and 4:2:2
sources, including nonzero origins and odd tile boundaries. Independent
component-selective PGX decoding must reproduce every original plane exactly
before the subject runs. Native discard levels are separately checked against
independent component decoding, not simple sample decimation.

sRGB resampling is checked exactly, with independent component upsampling at
aligned origins. Aligned-origin sYCC output has explicit engineering maximum
errors of 2 samples at 8 bits and 12 at 16 bits after common-depth normalization:
the reference converter uses rounded matrix coefficients and integer
truncation. These are not official color-conformance tolerances. Odd-origin
edge filling is checked against the decoder's documented clamping policy, not
claimed equivalent to the reference's different boundary policy.

Full and region/tile/tile-set color outputs must agree exactly after cropping
at the same native DWT resolution, including one-pixel and chroma-unaligned
regions. Half-grid CRG fixtures separately exercise registration phase.
The tests cover JP2/JPX and authored HTDECLARED all-legacy JPH wrappers with
unchanged independently qualified legacy coefficient/packet bytes; they do not
claim independent nonuniform HT encoding. The existing live HT suite remains
separate.

Color-selected paths retain required sampling context before conversion and
crop afterward. Invalid original selections are still rejected before context
expansion, and ordinary resource limits cover any additional decoded context.
An empty-resolution precinct consumes its packet header even when its present
flag is set and it contains no code blocks. Input/tool/source hashes and failing
fixtures are retained under the existing `container-failures/` artifacts.

### Component Maxshift and binary-complement NLT

Additional legacy cases exercise component-wide `RGN` upshifts for grayscale,
16-bit data, different RGB component indices, JP2, nonzero tiled origins,
quality layers and selected signed data. The requested RGN component/style/
shift header must actually be present. This CLI mechanism upshifts a whole
component; it is not evidence for arbitrary spatial ROI masks. Native
resolution, region, tile and tile-set relations are checked separately.

Live OpenJPH cases encode finite signed binary32 patterns with NLT type 3 at
zero, one and two DWT decompositions, including a signed JPH container. The
required NLT header must be present; signed-header authoring is not substituted
for this live transform. The independent PFM output and the subject's typed
32-bit signed words must preserve every original bit, including distinct
positive and negative zero. Reports label the representation
`ieee754-binary32-bits`; integer-word error metrics here are bit-exactness
diagnostics, not floating-point intensity or PSNR claims.

The PFM helper reads words in stored scanline order and only supports unit
scale and binary32 fixture payloads. Repeated rows isolate the nonlinear
sample transform; this is not general PFM orientation, NaN/infinity, color
management, other NLT-type or full floating-point codec qualification.
Failures still retain original/encoded/reference inputs and source/tool hashes.

HT cleanup U/MagSgn precision includes the sign-coding bit in addition to the
coefficient magnitude bound. The paired implementation/specification and
boundary regressions keep these limits distinct: up to 74 coefficient
magnitude bits and 75 MagSgn bits, with out-of-range values still rejected.
Missing default tools are explicitly `blocked`; configured missing tools,
process failures, invalid output and exceeded thresholds cannot pass.
`--encoder`, `--decoder`, `OPENJPEG_ENCODER` and `OPENJPEG_DECODER` select trusted
local tools. `all` includes this gate unless `--no-external` is selected.

### Independent HTJ2K checks

The separate `htj2k` gate uses OpenJPH's `ojph_compress` and `ojph_expand`.
Build the immutable revision listed in [REFERENCES.md](../REFERENCES.md)
with Git, CMake, and a C++ compiler available:

```sh
bash validation/build-openjph.sh
export OPENJPH_ENCODER="$PWD/target/validation/oracles/openjph-build/src/apps/ojph_compress/ojph_compress"
export OPENJPH_DECODER="$PWD/target/validation/oracles/openjph-build/src/apps/ojph_expand/ojph_expand"
.venv/bin/python validation/run-validation.py htj2k
```

The builder keeps the public reference checkout and build under ignored
`target/validation/oracles/`, disables SIMD for scalar reproducibility, and
rejects a modified or different-revision cached checkout. No upstream source
or binaries are included in the source package.

Generated nonzero cases exercise grayscale/RGB, 8/16-bit samples, sparse
coefficients, odd dimensions, tiles, nonzero origins, progression orders,
raw/JPH wrapping, and reversible/irreversible transforms. First-party fixed
codestream recipes cover positive/negative significance propagation and
magnitude refinement, combined refinement, and a minimum-value boundary.
Both the reference decoder and the subject must reproduce each fixed case's
known samples. Preencoded cases report `encoder_invoked=false`, empty encoder
options, and a `null` encoder timing; the encoder is not run for those cases.

For live lossless encoding, the reference's decoded samples must match the
original source before the subject runs. A reference self-roundtrip failure
is retained as a failure, not accepted as an alternative expected image.
The pinned reference encoder does not preserve the minimum-valued one-pixel
input with zero decompositions. Live encoder cases therefore use at least
one decomposition; an independently decoded fixed stream covers that
zero-decomposition boundary.

`htj2k-results.json` uses the same numerical schema, per-case engineering
thresholds, process bounds, hash bindings, and failure retention as the legacy
numerical gate. These comparisons do not establish official Part 4
conformance or exhaustive Part 15 coverage. Missing unconfigured tools block
the gate; an explicitly configured missing tool is an error.
`all` does not implicitly run `htj2k`; CI runs both commands explicitly.

## Nonzero selection relations

The Python regression suite also exercises the production `*_selected` APIs
on independently encoded nonzero images. It compares native full-resolution
and reduced-resolution output with OpenJPEG/OpenJPH, then compares region,
single-tile, rectangular tile-set, and tile-contained region output with exact
crops of the corresponding native full decode. Independent reference crops
must also meet the declared per-case numerical budgets.

```sh
PYTHONPATH=validation .venv/bin/python -m unittest discover \
  -s validation/tests -p test_selection.py -v
```

These tests cover raw, JP2, JPH, and JPX component decoding; grayscale/RGB;
8/16-bit samples; odd dimensions and nonzero origins; uniformly subsampled
components; and reversible/irreversible transforms. Reference resolution
reduction uses `opj_decompress -r` or `ojph_expand -skip_res`, not image
resizing. Nonzero fixtures additionally distinguish native DWT output from
simple post-decode decimation. Rectangles and reported output origins use the
image reference grid; PNM component dimensions are kept separate from the
subsampled image-grid extent.

Invalid nonrectangular tile sets and regions extending outside selected tiles
must retain typed selection errors. The tests do not relax those API
constraints. Tests needing unavailable native tools are explicitly skipped;
the CI setup installs both reference implementations before running them.
Disagreements retain source/encoded/reference files and a hash-bound
reproduction record under ignored `selection-failures/`, included in CI
artifacts. These component-selection tests do not replace JPX composition,
JPIP reconstruction, color-resampling, or official conformance checks.

The example harness exposes the same selection controls without changing its
ordinary full-decode path:

```sh
cargo run --example validation_decoder -- select raw image.j2k 1 all 4,6,32,24
cargo run --example validation_decoder -- select jp2 image.jp2 0 0,1 all
```

The arguments after the input are discard levels, `all` or comma-separated
tile indices, and `all` or `x,y,width,height`. Selected output begins with a
`FRAME` line containing the actual output origin, followed by the existing
image observation.

## Nonzero JPX and JPIP relations

The Python suite also builds first-party JPX and JPIP structures around
independently encoded nonzero samples:

```sh
PYTHONPATH=validation .venv/bin/python -m unittest discover \
  -s validation/tests -p test_container_relations.py -v
```

JPX relations cover 8-bit grayscale and 16-bit RGB layers, chroma-key
transparency, overlapping placements, source crops, frame persistence and
finite frame bounds. Expected canvases are calculated from the original
samples and authored placement rectangles, not from another subject decode.
The original cases retain opaque/binary alpha and their separate strict oracle.
Additional RGBA fixtures independently encode/decode every 8/16-bit color and
opacity plane before authoring CDEF channel roles or JPX OPCT declarations.
They cover fractional alpha including near-zero/near-opaque endpoints,
straight/premultiplied representation, reordered channels, 2-by-2 alpha
subsampling, and explicit opacity overrides.

A separate oracle uses Python `Fraction` arithmetic for source-over in
normalized sample space, rounding each output color/alpha to 16 bits after
each layer. It does not copy the production integer expression or consume
subject outputs as expectations. Static composition, placed/scaled/clipped
crops, persistent/nonpersistent frames, and equivalent straight/premultiplied
encodings must match exactly. Invalid premultiplied samples are rejected by
composition. Single-layer rendering preserves its samples and alpha
representation flag rather than pretending premultiplied samples are straight;
composited transparent pixels are transparent black.

This qualifies selected whole-image/global-alpha sRGB fixtures, not all
per-color/multiple-opacity associations, higher precision, linear-light
colorimetric blending, a third-party JPX compositor, or official alpha
conformance. Authored container metadata and the independently qualified
coefficient payloads remain distinct evidence. Nested fixture files, tools,
test sources and subject hashes are included in retained failure records.

JPIP relations check byte-exact reconstruction from reordered, duplicated,
explicit/inherited-context fragments, followed by sample-exact legacy and
HT decoding. Independently decoded OpenJPEG quality prefixes provide expected
8/16-bit progressive images. Incomplete packets and repeated updates must not
emit new images; complete layers must advance monotonically and reproduce the
corresponding independent output. Stored legacy magnitude bits remain a
prefix while integer coefficient materialization adds the coefficient-local
truncation midpoint. The last fully decoded bitplane needs no integer offset.

Automatic progressive cases use the original packet-length metadata.
Explicit-plan cases use authored inline headers without obsolete PLT
declarations, and separately retain the rejection of unsupported headers;
tests do not broaden the manual API's supported profile.
No network transport or service is involved.

Failures retain generated inputs, reference images, tool hashes/options and
reproduction records under ignored `container-failures/`. Distinct input,
subject and test hashes select distinct failure directories, so a later
disagreement does not overwrite an earlier one. CI uploads those directories.

## Budgets

```sh
.venv/bin/python validation/run-validation.py budget
```

Defaults are five seconds per case, 30 seconds total, one-MiB input/output
caps, and 512 MiB address space where POSIX `RLIMIT_AS` is available. Execution
enforces bounded output capture and reports unavailable memory enforcement.
Decoder `ResourceLimits` remain active on every platform.

### Representative process measurements

On Linux, install GNU Time alongside the OpenJPEG tools, build the pinned
scalar OpenJPH tools as above, and run:

```sh
.venv/bin/python validation/run-validation.py resources
```

This builds the release-mode harness and measures 1024-by-1024 grayscale and
512-by-512 legacy RGB8/RGB16 and HT RGB16 lossless inputs, each tiled at 128-by-128. Full decode, tile-zero
selection, and native resolution with two discarded levels must reproduce
independent reference output exactly. Declared-pixel-limit and truncated-header
cases must produce their expected typed errors, not merely exit unsuccessfully.
HT cases require exact original/OpenJPH agreement and actual SIZ/CAP/COD
geometry and HT coding signals. Their tile-zero expectation is a checked crop
of the full independent lossless decode; native reduction uses independent
`-skip_res` synthesis, not decimation. The shared crop helper also serves the
selection tests and rejects inconsistent frame/component geometry.

`--ht-encoder` and `--ht-decoder`, or `OPENJPH_ENCODER` and
`OPENJPH_DECODER`, select trusted HT tools. Missing unconfigured tools block
the full gate; an explicitly configured missing tool is an error. Legacy and
HT reference executable hashes are recorded separately.

The gate also independently qualifies 32-by-32 unsigned24 grayscale and RGB16
fixtures. The latter has three quality layers, 16-by-16 tiles and an asymmetric
origin; the required SIZ/COD fields are checked, not inferred from encoder flags.
For each fixture, a bounded search finds an observed adjacent decoded-byte
success/rejection pair and an observed adjacent work-unit pair. The test-only
`decode-limited` operation tightens exactly one `ResourceLimits` value; other
limits retain their hardened defaults. The search first requires typed rejection
at zero and exact independently decoded output at its ceiling (16 MiB decoded
bytes or 10,000,000 work units). It uses at most 32 probes per fixture/resource.
Each accepted frontier and the budget one unit below it are then rerun three
times. The lower budget must report the exact configured resource/limit and an
excessive claim; process failure or a different typed error cannot qualify.

Each of the twenty-two final cases is measured three times. Every search probe
also retains its budget, wall time, peak RSS, return code and exact output hash
or typed limit claim. GNU Time reports actual peak
process RSS; the bounded runner measures wall time including process startup
and image-observation serialization. The engineering thresholds are:

| Case | Maximum time per process | Maximum peak RSS |
|---|---:|---:|
| Full image | 5 seconds | 512 MiB |
| Tile/native-resolution selection | 2.5 seconds | 256 MiB |
| Early typed rejection | 1 second | 128 MiB |

Aggregate measured native time, including search probes, is limited to 30 seconds.
Search probes obey the same per-process thresholds as their accepted-frontier
case. Tile-zero peak RSS
must be at least 25 percent lower than full decode for each generated image.
Each process also has a one-GiB address-space limit and bounded output capture.
These measurements are not universal worst-case guarantees. Decoded sample
storage, address-space enforcement, and measured total-process RSS are distinct
quantities; the report keeps them separate.
An observed adjacent budget pair is not a theorem that all lower values fail
for every decoder path, nor a universal minimum budget. A decoded-byte limit
controls accounted decoder claims, not total process RSS. Work units are an
implementation accounting threshold, not a CPU-time or instruction count.
These finite fixtures do not characterize all production-default or worst-case
inputs, high-component layouts, broader HT/container paths or every supported platform.

Resource report schema version 2 records configured budgets, finite search
evidence and the separate repeated frontier observations.
`resource-results.json` retains source/tool/input/output hashes, repetitions,
individual observations, median times, peak RSS, thresholds and explicit
failures. Unavailable default tooling/platform support blocks the gate;
explicitly configured missing tools and absent measurements cannot pass.
Failed inputs are preserved under ignored `resource-failures/` and uploaded
by CI. `all` does not implicitly invoke this Linux-specific gate; CI calls it
separately. `--subject` selects a trusted prebuilt binary; `--no-build` uses
the existing release harness.

## Technical reference inventory

`normative-coverage.json` is a static inventory of topics and implementation,
specification, executable contract, proof/view, and test references. Its
validator checks every present item, including contracts that ensure a listed
spec and are called by the claimed implementation. Shadow-only links fail.
For scalar contracts it recognizes both `ensures returns (...)` and a named
return value whose typed or natural-number equality directly names a listed
model, including contracts with a `requires` clause.
For checked slice stores it also recognizes an equality between
`(named_result, final(slice)@)` and a listed state-transition model. Unrelated
return binders, pre-state-only tuples, missing final-state relations and mere
model mentions do not qualify. Functions taking mutable references must still
provide the final-state relation; a bare return equality cannot replace it.
This is a conservative source-link check, not a
replacement for Verus.
It does not assert exhaustive standards-clause coverage, exact progress
counts, phase completion, or conformance.

## External official vectors

Assets remain in an authorized, externally managed directory and are never
copied into the repository or report artifacts. Use `--vector-root` or
`JPEG2000_OFFICIAL_VECTOR_ROOT`, and `--manifest` or
`JPEG2000_OFFICIAL_VECTOR_MANIFEST`. The default external manifest name is
`official-vector-manifest.json`.

The manifest schema requires source/acquisition metadata, authorized-use and
license evidence, redistribution disabled, archive and input byte/hash
bindings, explicit selection, and independently established normalized hashes
for accepted cases. Paths are confined to the external root. A manifest's
source URL identifies its suite; no private standards-profile file is needed.
These declarations must reflect the operator's actual rights and evidence.

```sh
JPEG2000_OFFICIAL_VECTOR_ROOT=/authorized/vector-root \
  .venv/bin/python validation/run-validation.py official
```

Missing access/configuration produces an explicit `blocked` report and nonzero
exit. Invalid manifests fail before execution. Reports contain hashes,
permitted public identifiers, counts, timing, normalized hashes, and geometry,
not private paths, identifiers, samples, or asset bytes. Archive-integrity,
execution, and resource failures cannot pass. This optional command is not
part of clean-clone `all`.

## Proof report

After an actual pinned whole-crate Verus run:

```sh
.venv/bin/python validation/generate-proof-coverage.py target/validation/verus-result.json
.venv/bin/python validation/run-validation.py schemas
```

The default report is `target/validation/proof-coverage.json`; `--output-dir`
selects another directory. The checker requires the pinned commit,
whole-crate success, zero errors, nonempty project results, and no unmapped
termination bypasses or trusted proof escapes. It does not compare against
committed progress output or infer coverage for absent functions. CI retains
the stable release's `--no-lifetime` boundary explicitly; lifetime verification
is not claimed.
