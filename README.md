# jpeg2000_rs

`jpeg2000_rs` is a proposed memory-safe Rust decoder for selected JPEG 2000
standards, with Verus verification attached to the production implementation.

## Current status

**The first-release profile was approved on 2026-10-13. Implementation is in
progress and the crate is not releasable.**

The proposed scope is machine-readable in
[`support-matrix.json`](support-matrix.json), and the selected normative
editions are recorded in
[`standards-profile.json`](standards-profile.json). The profile intentionally
does not claim to be a complete JPEG 2000 codec: the first release is
decoder-only, has explicit resource limits, and places irreversible 9/7
numerical conformance outside the Verus functional-correctness claim.

No standard text or restricted conformance vector may be committed to this
repository. Importing or redistributing standards-derived assets and
publishing the crate require CELA/legal approval.

### Implemented checkpoint

Phase M formal refinement (`phase-m-formal-refinement`) links every selected
executable Parts 1, 2, 9, and 15 requirement group to concrete implementation
functions, executable Verus contracts, normative specs, proofs, and tests.
The machine-validated ledger reports 54 functional groups, 160 contracted
functions, and zero unlinked selected groups. This is selected-profile closure,
not a claim of complete JPEG 2000 coverage. Irreversible 9/7 numerical
conformance remains a validation boundary; official Part 4 conformance vectors
are still pending. Encoding, network/TLS/auth/request/server behavior, and ICC
execution remain out of scope. Phase L
selected-spec closure (`phase-l-selected-spec-closure`) closed on 2026-10-17.
Its final JPX checkpoint closes general multi-codestream layers,
channel associations, opacity, composition, nested metadata structure, and
non-JP2-subset codestream-header overrides in addition to signed and
subsampled conversion for pinned spaces 16, 17, and 18. A final source audit
also closes implicit JPX layers, inherited color headers, unassociated alpha,
tile-local mixed component transforms, reserved-marker classification,
contradictory JP2 branding, and malformed JPIP layouts. The complete
machine-readable residual boundary inventory is
`unsupported-boundary-audit.json`; it records zero remaining selected-profile
class-C paths and zero support-matrix contradictions. Phase H decoder-validation
hardening (`phase-h-encoding-validation`) closed on 2026-10-16. Encoding
remains outside the approved first-release scope. Release conformance still
requires approved official vectors, legal approval for their use, immutable
release inputs, and release owner approval. Phase G selective decode
(`phase-g-selective-decode`) closed on 2026-10-16, and Phase F JPIP
(`phase-f-jpip`) closed on 2026-10-15 as a bounded proof-of-concept profile.
The Phase E HTJ2K profile-detection, block-segment, cleanup reconstruction,
significance-propagation, and magnitude-refinement checkpoints were completed
on 2026-10-14; Phase L now closes the remaining legal Part 15 pass-set,
mixed-routing, and constrained-capability combinations.
The earlier JP2
palette, component mapping, channel-definition output, bounded
color-conversion, and Part 2 multiple-component-transform checkpoints
remain complete, as do the Phase C11 POC, PPM/PPT, PLM/PLT, tile-header
default QCD, COC/QCC component-override, and Maxshift ROI checkpoints. The
implemented safe profile currently provides:

- hardened typed resource limits and error categories;
- JPIP binary message-header parsing for inherited, explicit-class, and
  explicit-class-and-codestream Bin-ID forms, optional auxiliary VBAS fields,
  and end-of-response messages; overlong, overflowing, prohibited, truncated,
  non-canonical, and body-length-mismatched headers are rejected;
- typed JPIP precinct, tile-header, tile-data, main-header, and metadata
  data-bin identities for standard class identifiers 0 through 8, with checked
  64-bit offsets, lengths, final lengths, input bytes, accepted fragment
  operations, and interval storage;
- deterministic JPIP cache semantics: identical duplicates are idempotent,
  byte-identical overlaps merge, conflicting bytes or final lengths reject
  without mutation, fragments beyond a known final length reject, and
  out-of-order fragments become complete only after contiguous coverage from
  byte zero reaches the announced final length;
- complete-bin status and owned `JpipByteView` APIs; bounded codestream
  reconstruction accepts one complete main-header bin followed by matching
  tile-header/tile-data pairs from one codestream and requires SOC/EOC, while
  container reconstruction concatenates caller-ordered complete metadata
  bins; both views implement `AsRef<[u8]>` for the existing decode APIs;
- deterministic `JpipDatabinRange` availability reports over incomplete bins,
  including exact closed missing intervals and an explicit open-ended header
  tail when no final length has been announced;
- retained caller-supplied packet-range reconstruction plus automatic class
  0/1 precinct mapping from complete SIZ/COD/COC geometry. The mapping is
  `tile + tile_count × (component + component_count ×
  (prior-resolution precincts + row-major precinct))`; class 0/1 and other
  auxiliary-bit aliases share one transactional logical bin;
- automatic bounded transcoding covers multiple tiles, header-only
  multi-tile-part bins, all five progression orders and retained POC windows,
  SOP/EPH, and source PLM/PLT/PPM/PPT by reframing packets inline and removing
  stale packet-length/packed-header markers. Changing later tile-part
  COD/COC/QCD/QCC/RGN/POC state is retained and executed by the raw packet
  state machine. Boundaries use PLT counts when present and otherwise advance
  packet headers and contribution lengths transactionally to the exact
  non-zero `Psot` consumption; boundary status remains unavailable only when
  the required precinct-bin prefix is incomplete or no standard boundary is
  encoded;
- semantic metadata assembly starts from metadata-bin 0 and recursively
  replaces standard `phld` metadata and incremental-codestream placeholders,
  preserves 32/64-bit original box headers with recomputed lengths, detects
  cycles/depth/size violations, resolves nested superboxes and leaf payloads,
  and validates the resulting JP2/JPX/JPH container with the existing parser;
- `tile_part_transitions` retains every SOT-through-SOD header and derives
  both PLT packet counts and non-zero-`Psot` data consumption. Functional later
  COD/COC/QCD/QCC/RGN/POC state therefore has an exact transition record
  without requiring PLT; the standard zero-`Psot` form is accepted only for
  the final tile-part that extends to EOC;
- transactional manual `JpipProgressiveDecoder` and mapped
  `JpipAutomaticProgressiveDecoder` full/selected updates decode the
  greatest contiguous available quality-layer prefix, report the next missing
  ranges, and advance state only after transcoding and decode succeed; precinct
  bins need not be final when every packet required by that prefix is present.
  Selected updates reuse tile/region/reduced-resolution decode and additionally
  accept a component packet mask; omitted packets are explicit legal empty
  packets, while the returned image retains the codestream component schema;
- explicit JPIP exclusions: cache-model signaling syntax is outside the
  selected bounded profile and lacks pinned public field-level provenance;
  network transport, request generation, authentication, TLS, and server
  behavior remain absent;
- zero-copy JP2 top-level box framing for 32-bit, extended, and to-EOF lengths;
- JP2/JPX signature, primary/compatibility-brand, header-before-codestream,
  duplicate compatibility-brand, and profile-specific codestream-count
  validation, with hardened 64-brand compatibility-list and 64-codestream
  defaults;
- canonical JPH `jph ` primary/compatible-brand recognition (minor version
  zero and exactly one `jph ` compatibility entry), explicit `inspect_jph`,
  `decode_jph`, and `decode_jph_color` routing, and rejection of ambiguous or
  contradictory JPH/JP2/JPX branding;
- retained bounded HT profile metadata: SIZ `Rsiz`, CAP `Pcap`/ordered `Ccap`
  values, decoded Part 15 HTONLY/HTDECLARED/MIXED, SINGLEHT/MULTIHT,
  RGNFREE/RGN, HOMOGENEOUS/HETEROGENEOUS, HTREV/HTIRV, and magnitude-bound
  capabilities, PRF/CPF profile words, and aggregate legacy/HT-only/mixed
  code-block classification across effective main, component, and tile styles;
- cross-validated Part 15 signaling: `Rsiz` bit 14 and CAP `Pcap^15` agree;
  style prefixes `00`, `01`, and `11` identify legacy, HT-only, and mixed
  tile-components while reserved `10`, reserved `Ccap15` mode `01`, reserved
  capability bits, and capability/style/ROI/transform/tile-header
  contradictions are malformed;
- raw codestream inspection and JPH inspection complete without entering the
  block decoder; HT-only and mixed decoder construction succeeds, one cleanup
  and up to two refinement passes are routed across packet/layer
  contributions, cleanup `Scup` termination is validated, and refined blocks
  route through coefficient reconstruction and top-level JPH output;
- checked HT cleanup physical partitioning into non-overlapping MagSgn and
  combined MEL/VLC regions, bounded forward MEL/reverse VLC views, and retained
  forward SigProp/reverse MagRef refinement views, with per-block pass, byte,
  sample, and work limits;
- HT cleanup forward-MEL and reverse-VLC bit readers with directional stuffing,
  bit-granular cursor-collision rejection, bounded MEL run-state decoding,
  pinned VLC lookup words, initial/non-initial UVLC parsing, and checked
  per-quad significance/embedded-magnitude/base-`u` metadata; significance
  outside odd code-block edges, malformed stuffing, truncated symbols, and
  work overruns are rejected;
- a bounded forward LSB-first MagSgn reader with HT stuffing, physical-region
  collision prevention, one-valued termination fill, and physical/fill work
  accounting; exponent-dependent non-initial kappa completion, embedded
  magnitude/sign reconstruction, checked coefficient writes, and cleanup
  termination validation;
- bounded forward SigProp and reverse MagRef readers with directional
  stuffing, shared-byte claim collision checks, zero termination fill, and
  physical/fill work accounting; four-row, four-column-patch SigProp traversal
  dynamically propagates significance before sign decoding, while MagRef
  updates only cleanup-significant samples and all plane replacement is
  transactional;
- all legal HT pass-set shapes are routed: initial groups of three zero-byte
  placeholder passes, one or more cleanup/SigProp/MagRef HT sets, partial last
  sets, zero-length skipped sets, and packet/layer continuation; SINGLEHT
  rejects a second set while MULTIHT permits it, and each non-empty cleanup
  atomically replaces the prior precision reconstruction;
- mixed style code-blocks defer routing across placeholder-only contributions
  and use the normative first non-zero segment-length discriminator
  (`Lblock > 3` and a zero first length bit), retaining the HT or legacy
  decision across later packets and layers; legacy BYPASS and RESTART
  segmentation remain available where legal, while mixed-mode prohibited
  BYPASS/RESTART combinations are malformed;
- retained, input-bounded JPX `jp2c` codestream boxes with explicit indexed
  inspection, ordinal `jpch` associations, per-associated-header
  dimension/component/depth checks when the JP2 metadata subset applies, and
  explicit association bounds;
- explicit `decode_jpx_codestream(input, index, options)` typed output with
  checked indices and no default codestream selection;
- bounded JPX `jplh` parsing with the selected-edition six-byte `creg` records,
  checked grid denominators, byte offsets, codestream references, scaled
  nearest-grid registration, `cgrp` color overrides, general concatenated
  multi-codestream `cmap`/`cdef` channel associations, and `opct` straight,
  premultiplied, or per-component-precision chroma-key opacity;
- explicit `inspect_jpx_layers`, `render_jpx_layer`, and
  `render_jpx_composition` APIs covering scaled multi-codestream layers,
  mixed grayscale/sRGB conversion, and multiple whole-image or per-colour
  alpha associations combined with bounded fixed-point arithmetic;
- bounded JPX `comp`/`copt`/`inst` parsing for target position/size,
  life/persistence, next-use layer reuse, source regions, instruction-set
  repetition, and composition loops; inspection exposes deterministic frame
  timing and `render_jpx_frame(input, index, options)` renders a selected
  bounded frame without clocks, threads, or network access;
- partial `jpch` overrides execute inherited `ihdr`/`bpcc`/`colr` plus
  `pclr`/`cmap`/`cdef`/`res` replacements, and nested JPX association metadata
  is structurally validated with bounded box count and depth;
- bounded SOC, SIZ, SOT, SOD, and EOC codestream framing;
- retained, bounded Part 2 `MCT`, `MCC`, and `MCO` main- and tile-header data
  for multiple indexed arrays, disjoint collections, and uniquely ordered
  stages; segmented arrays and records require contiguous indexes, one
  conflict-free initial/final declaration, and complete reassembly;
- reversible custom transforms with signed 16- or 32-bit integer arrays and
  checked integer matrix arithmetic, and irreversible custom transforms with
  signed integer or finite IEEE binary32/binary64 arrays converted without host
  floating-point operations to deterministic, ties-to-even Q32 fixed point;
  matrix/offset dimensions, references, unique stage ordering, parallel
  collection independence, component geometry, precision growth, decoded
  scratch bytes, work, and arithmetic are checked;
- normative dependency collections execute reversible triangular prediction
  or irreversible lower-triangular prediction, while wavelet collections
  synthesize one-dimensional component-axis 5/3 or 9/7 levels with signed
  canvas-origin parity; coefficient cardinality, subband partitioning,
  geometry, precision, scratch, arithmetic, and cumulative work are checked;
- raw, JP2, JPX, full, and selected paths apply inverse stages in reverse `MCO`
  order before any combined Part 1 inverse component transform; tile-header
  arrays/records/order replace inherited indexes for that tile;
- reserved MCC transform type 2, reserved transform bits, invalid references,
  malformed fragment series, and incomplete records are rejected as malformed
  rather than being widened into an `UnsupportedTransform` boundary;
- typed COD/QCD progression, transform, layer, code-block, precinct, and
  quantization validation;
- nested JP2 image header, component bit depth, color, palette, component
  mapping, channel definition, and resolution validation;
- JP2-to-codestream dimension, component, precision, and signedness
  consistency checks;
- bounded packet-header bit reading with JPEG 2000 `0xff` bit-stuffing
  validation;
- resource-bounded tag-tree topology and monotonic incremental decoding;
- lazy LRCP, RLCP, RPCL, PCRL, and CPRL packet-coordinate iteration with
  packet, precinct, candidate-work, and resource bounds;
- code-block packet inclusion, zero-bitplane, coding-pass, `Lblock`, and
  contribution-length decoding with failure-atomic code-block state;
- a bounded 47-state MQ arithmetic decoder with context transitions,
  marker-aware byte input, renormalization, and work accounting;
- raw-bypass bit decoding, normative Tier-1 pass and forced-segment scheduling,
  bounded per-segment packet length decoding, and failure-atomic code-block
  segment byte/pass accounting;
- bounded coefficient-state storage with safe eight-neighbor significance and
  sign summaries plus monotonic significance/refinement updates;
- orientation-specific zero-coding, sign-prediction, and magnitude-refinement
  context formation;
- standalone MQ and selective raw-bypass Tier-1 significance-propagation,
  magnitude-refinement, and cleanup traversal in normative four-row stripe
  order, including typed code-block styles, vertical-causal contexts, context
  reset support, cleanup run mode, segmentation-symbol validation, and
  per-bitplane eligibility tracking;
- bounded code-block contribution body consumption across packet layers with
  persistent MQ probability states, predictable MQ/raw termination checks,
  support for 38 coefficient bit-planes, and row-major signed coefficient
  access for later inverse quantization;
- bounded multi-code-block packet-prefix execution directly over retained
  tile-part payloads, with persistent tag trees, packet states, and Tier-1
  decoders; checked SOP/EPH handling; and exact header/body byte accounting;
- progression-order-driven execution of configured precinct packet sets across
  contiguous retained tile parts, with canonical precinct lookup, persistent
  packet and Tier-1 state across layers, validated tile-part/COD continuity,
  and rejection of truncated or trailing payload bytes;
- independent LL or HL/LH/HH packet subband state, inclusion trees, and
  zero-bitplane trees within each progression precinct, with header-first
  contribution parsing and subband-order body routing;
- retained main- and tile-header QCD defaults and QCC exponent/mantissa steps;
  validated main- and tile-header COD/COC and QCD/QCC default/component
  precedence with duplicate, component index, marker-length, compatibility,
  and subband-count checks; validated RGN
  defaults/overrides per component; per-component coding geometry, packet
  progression, Tier-1 style, inverse quantization, and inverse wavelet
  transform; Maxshift-adjusted Tier-1 depth; bounded reversible 5/3 and
  deterministic fixed-point irreversible 9/7 lifting, two-dimensional subband
  synthesis, and signed/unsigned sample conversion with decoded-byte limits;
- retained per-component SIZ precision, signedness, and subsampling; exact
  non-overlapping tile-component assembly; reversible and fixed-point
  irreversible three-component transforms; and bounded typed signed/unsigned
  component outputs;
- bounded parsed-contribution orchestration from packet body segments through
  cross-layer Tier-1 reconstruction and inverse quantization into shaped
  code-block sample planes;
- zero-copy retention of validated fixed-length and to-EOC tile-part payloads,
  including tile identity, ordering metadata, and effective tile coding style;
- bounded PLM and PLT parsing with base-128 variable-length packet lengths,
  strict marker-index continuity, scope and duplicate checks, exact tile-part
  payload coverage, retained zero-copy marker data, and progression-driver
  packet slicing against declared boundaries;
- bounded PPM and PPT fragment retention with strict marker-index continuity,
  duplicate and scope rejection, cross-fragment `Nppm` framing, deterministic
  packet-header stream reassembly, and exact per-tile-part ownership;
- separate packed-header and retained tile-body cursors with PLM/PLT logical
  packet-length enforcement, tile-body SOP sequence validation, packed EPH
  validation, and exact exhaustion accounting;
- retained main- and first-tile-part POC records with `Csiz`-selected component
  index widths, typed range/order/coverage/duplicate/scope validation,
  persistent tile overrides across tile parts, and successive progression
  windows that suppress already emitted packets while requiring exact packet
  coverage; POC traversal shares PLM/PLT boundaries and PPM/PPT header streams;
- exact non-overlapping code-block placement into complete subbands plus a
  prepared-image orchestration API that invokes tile-component assembly and
  optional reversible or irreversible component transforms;
- bounded component-resolution and code-block grid derivation from retained
  SIZ subsampling and COD decomposition/code-block geometry;
- origin-aware multi-tile, multi-resolution packet planning that derives LL/HL/LH/HH
  dimensions and orientations, scalar-derived or per-band QCD step selection,
  Tier-1 bitplane depth, precinct-local code-block shapes, and exact decoder
  placements;
- post-progression routing of retained Tier-1 coefficient planes into complete
  inverse-quantized subbands, iterative 5/3 or fixed-point 9/7 synthesis,
  `TileComponent` construction, and invocation of `decode_prepared_image`;
- retained SIZ image/tile origins and extents, exact clipped tile and
  tile-component bounds, origin-aware subband/code-block geometry, and
  non-overlapping multi-tile assembly;
- public `decode_codestream` and `decode_jp2` entry points returning
  `TypedImage`, whose per-component signed/unsigned samples retain precision,
  signedness, subsampling, dimensions, and placement;
- explicit `DecodeSelection` plus `decode_codestream_selected`,
  `decode_jp2_selected`, `decode_jph_selected`, and indexed
  `decode_jpx_codestream_selected` APIs (with matching color variants);
  selection accepts all tiles, one tile, or a rectangular tile set, an
  optional half-open image-reference-grid `ImageRegion`, and a count of
  highest DWT levels to discard. Region coordinates are absolute SIZ
  coordinates, include a nonzero image origin, and are mapped to subsampled
  component grids with ceiling division. Explicit tile sets must be unique,
  in range, rectangular, and fully cover the requested region;
- selected tile traversal omits nonintersecting tile parts entirely, and
  reduced-resolution packet traversal parses required packet framing while
  skipping unneeded packet bodies before Tier-1, inverse quantization,
  high-resolution subband allocation, and DWT synthesis. Packet, precinct,
  code-block, work, and decoded-byte accounting uses selected execution
  where work can be omitted; unavoidable sequential packet-header framing
  remains input/work accounted;
- retained, metadata-bounded `pclr`, `cmap`, and `cdef` records; checked direct
  and palette lookup mappings; palette-derived precision and signedness;
  decoded-output component/byte bounds; and per-output-channel typed color,
  opacity, premultiplied-opacity, unspecified, whole-image, color-associated,
  and unassociated metadata;
- explicit `convert_jp2_color`, `decode_jp2_color`, and indexed
  `decode_jpx_codestream_color` APIs returning bounded 16-bit `ConvertedImage`
  output; deterministic integer conversion for pinned
  enumerated sRGB (16), grayscale (17), and sYCC (18); channel-association
  ordering; preserved straight or premultiplied alpha samples and association;
  checked signed-to-nominal conversion for 1-38 bit samples; deterministic
  nearest component-grid resampling with nonzero-origin, subsampling,
  selected-region, and reduced-resolution placement; and checked component
  count, sample range, output-byte, and conversion-work limits;
- explicit JPX layer/frame inspection and rendering with checked
  layer/codestream/frame counts, registration scaling, target/source regions,
  canvas pixels, cumulative converted bytes, and per-pixel work; composition
  output is planar 16-bit sRGB or grayscale plus whole-image straight alpha;
- byte-exact, metadata-bounded ICC profile preservation through
  `ColorSpecification::icc_profile`; ICC execution and unpinned enumerated
  spaces return `UnsupportedColorConversion` rather than selecting an implicit
  color engine;
- a strict typed marker profile that parses, validates, bounds, and retains
  `TLM`, `CRG`, `DFS`, `ADS`, `NLT`, `CBD`, and `ATK` in addition to
  `MCT`/`MCC`/`MCO`; main-header values are inherited by tile parts and
  same-index tile-header values replace them, while TLM lengths are checked
  against every SOT and all extension bytes consume metadata and work budgets;
- deterministic NLT inverse execution for identity (type 0), fixed-point
  gamma-style transfer curves (type 1), LUT interpolation (type 2),
  binary-complement/sign-magnitude conversion (type 3), and the combined
  experimental LUT form (type 4), including component/all-component precedence
  and tile-specific regions;
- independently padded ADS `DOads`/`DSads` parsing and bounded topology
  retention, plus ATK whole-sample and arbitrary-filter inverse lifting with
  all coefficient widths, offsets, multitap steps, odd/even initialization,
  constant or symmetric extension, and binary128 conversion;
- main-header-only CBD enforcement, including compact one-depth-for-all
  descriptors; tile-header CBD is malformed syntax;
- nonzero CRG fractional registration is carried into full/selected
  component/tile bounds and JP2/JPX color resampling without host floating
  point;
- DFS-selected components use an explicit bounded per-stage decomposition
  sequence: no-DWT, horizontal-only, vertical-only, and bidirectional stages
  drive dynamic quantization-band counts, resolution dimensions,
  precinct/code-block topology, progression, and inverse synthesis; selected
  reduction is accepted through the consecutive finest bidirectional prefix;
- retained COD precinct exponents and checked component-resolution precinct
  grid/cardinality derivation;
- checked image, component, tile, tile-part, metadata, input, and work limits;
- deterministic rejection of truncated prefixes, invalid ordering, duplicate
  brands/headers/composition boxes, impossible geometry, out-of-range
  codestream/layer/frame selection, reserved `opct`/`inst` fields, trailing
  raw-codestream bytes, malformed JPX channel mappings, and reserved fields.

Pinned Verus verifies the exact `src/lib.rs` Cargo root with 2,264 obligations
and zero errors. The repository records deterministic dispositions in
`production-proof-coverage.json`, which is excluded from ordinary packages.
The urgent phase-M checkpoint removes all 113 termination-bypass attributes
reintroduced by the later JPIP, HT, Part 2, and JPX work. Explicit decreases,
stable cursor/pass snapshots, bounded recursive-depth arguments, and loop
invariants now cover those paths. The functional-refinement ledger links all
54 selected executable Part 1,
Part 2/JPX, Part 9, and Part 15 requirement groups to 160 contracted
production wrappers, their specification functions, concrete callers, proofs,
and tests, with zero unlinked selected groups. Part 15 coverage includes JPH
branding; profile signaling and classification; pass grouping and mixed
routing; cleanup, MEL/VLC/UVLC/MagSgn, and refinement stream semantics; and
all selected capability combinations. Exact external PRF/CPF profile-number
registry meanings remain inaccessible metadata and are not guessed; executable
PRF/CPF presence and consistency checks are linked. The coverage generator
scans every Rust source under `src/`, `specs/`, and `proofs/` and rejects any
bypass attribute without an exact source mapping. All 2,054 project functions
have complete production-proof dispositions (2,049 verified functions plus
five Rust-only adapters), and the termination boundary is empty. A 2026-10-16
probe of rolling Verus `0.2026.10.16.4fc2653` without `--no-lifetime` still exited 1 with the
`VerusErasureCtxt`/`thir_body` internal error on both the exact production root
and `validation/verus-lifetime-reproducer.rs`. CI therefore retains stable
Verus `0.2026.10.04.426d8b0` and its `--no-lifetime` workaround; Rust borrow
checking remains active. The new dispositions include the
fragment-continuity/completeness,
dependency-cardinality, and component-wavelet partition/work obligations.
`lifetime-analysis-blocker.json` records the release
metadata, asset checksum, commands, and reduced reproducer result. This
lifetime-analysis boundary keeps the proof attestation blocked independently
of termination, and release publication remains subject to the separately
recorded release gates.

The selected Part 2 marker families have bounded typed syntax and execution;
DFS/ADS topology, ATK inverse lifting, all selected NLT forms, CBD scope, and
CRG share the raw/JP2/JPX full and selected routes. Remaining approved
boundaries are encoding; transport, TLS, server, request, and spatial
scheduling; incremental streaming and internal parallelism; ICC execution;
unpinned enumerated color values; unselected parts; and unpinned authenticated
cache-signaling syntax. JPX metadata payloads beyond bounded structural
association nesting remain opaque rather than codec state. These boundaries
are synchronized between `support-matrix.json` and
`unsupported-boundary-audit.json`.

The selected normative Part 2 edition is ITU-T T.801 (08/2023), common text
ISO/IEC 15444-2:2023. `source-provenance.json` separately binds the acquired
06/2021 T.801 publication by SHA-256 for cited unchanged syntax and annex
semantics; it explicitly does not claim that artifact alone is a byte-for-byte
source for every 08/2023 consolidation change. ICC execution, unpinned color
registry values, and encoding remain approved out-of-scope boundaries, not
coverage gaps.

Part 1 Maxshift supports every legal `SPrgn` value through 255 for component
precision through 38 bits. Tier-1 coefficients use a bounded six-limb,
384-bit magnitude representation, supporting the maximum 292 ROI-adjusted
bit-planes and 874 coding passes. Legacy significance/refinement and HT
cleanup/SigProp/MagRef update that representation directly; Maxshift is undone
before checked narrowing, inverse quantization, and the existing public typed
sample outputs.

## Planned single-source architecture

- `src/` will be the only executable implementation and Cargo source of truth.
- `specs/` will contain functional models.
- `proofs/shared/` will contain abstract views and representation invariants.
- `proofs/spec_properties/` will contain implementation-independent theorems.
- `proofs/production_impl/` will contain proof bodies and proof-only macros
  attached to production functions.
- `validation/` contains corpus provenance and will contain conformance,
  differential, negative, and performance validation.
- `fuzz/` will contain valid-input and arbitrary-input fuzz targets.

The pinned Verus configuration must verify the exact `src/lib.rs` that Cargo
builds. Ordinary builds and packages must not contain proof bodies. A second
proof-only executable implementation is prohibited.

The requested `microsoft/ced_rs` and `microsoft/liburlpattern_rs` repositories
were not publicly retrievable when this checkpoint was created, so no
implementation detail is attributed to them. The concrete erasure, cfg, and
single-root mechanics used here are grounded in the pinned public Verus
release.

## Recorded approval

The owner approved `support-matrix.json` in chat on 2026-10-13. Before release,
the project must still:

1. Confirm acquisition rights for each normative edition.
2. Confirm the Part 9 publication record and the selected color-space registry
   snapshot.
3. Obtain CELA/legal approval before importing any standards-derived vector.

See [`SECURITY.md`](SECURITY.md), [`THREAT_MODEL.md`](THREAT_MODEL.md), and
[`RELEASING.md`](RELEASING.md) for the planned security and release gates.
