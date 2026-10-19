# Threat model

## Security objective

Decode hostile JPEG 2000 containers, codestreams, and reconstructed JPIP
databins without memory unsafety, panics, arithmetic wrap, unbounded resource
use, state confusion, or nondeterministic failure.

The decoder is not a trust boundary for authenticity, confidentiality, DRM,
malware detection, or semantic validation of embedded XML, UUID payloads, ICC
profiles, or other application metadata.

## Assets

- Process memory and availability.
- Integrity of decoded sample planes and metadata.
- Deterministic enforcement of caller-selected resource limits.
- Isolation of native differential oracles and restricted corpus assets.
- Accuracy of proof, conformance, fuzz, and release attestations.

## Trust boundaries

- Every input byte and all metadata derived from it are untrusted.
- Public and standards-derived corpora are untrusted until hashed and reviewed.
- Native reference codecs are untrusted, memory-unsafe subprocesses.
- Verus, Rust, compiler, CI, and corpus tooling are trusted only at pinned
  revisions recorded by release attestation.
- Proof assumptions are limited to reviewed standard-library contracts. Codec
  algorithms may not use `external_body`, `assume`, or `admit`.

## Threats and mandatory controls

| Threat | Examples | Mandatory controls |
|---|---|---|
| JP2/JPX box-length attacks | Wrapped 32-bit length, invalid extended length, nested to-EOF boxes, overlapping children | Checked cursor arithmetic; parent-bounded slices; nesting, metadata, and input limits; exact consumption contracts |
| JPX source confusion | Duplicate brands, implicit first-codestream choice, mismatched `jpch` ordinal, invalid `creg` references, unbounded layer/instruction counts, silently ignored composition instructions | Validate compatible brands, ordering, uniqueness, ordinal associations, registration references/grid values, dimensions, offsets, and counts; require explicit codestream/layer selection; reject unexecuted semantics with typed failures |
| JPH/HT profile confusion | `jph ` compatibility on a JP2/JPX primary brand, non-Part-15 data mislabeled JPH, HT data mislabeled JP2, contradictory `Rsiz`/CAP/COD/COC signaling, reserved style/capability modes | Require canonical JPH branding; retain bounded CAP/PRF/CPF metadata; cross-check container, SIZ, Ccap15 constrained sets, and effective block styles (including legal all-legacy HTDECLARED); reject malformed contradictions before block execution; retain normative mixed legacy/HT selection for each code-block |
| JPX composition amplification | Huge canvas, off-canvas layers, cumulative decoded planes, alpha arithmetic overflow, animation loops | Bound canvas pixels, cumulative converted bytes, and per-pixel work; checked clipping/index arithmetic; deterministic widened-integer source-over; reject animation/instructions/scaling/unsupported blend modes |
| Image geometry overflow | Huge origins, dimensions, tiles, components, subsampling, or output strides | Checked normalized geometry; reject contradictory bounds; enforce pixel, component, tile, and decoded-byte limits before allocation |
| Selective-decode confusion | Duplicate/out-of-range tiles, disconnected tile sets, origin-relative region ambiguity, empty subsampled planes, excessive reduction, limits charged to unselected Tier-1 work | Absolute half-open image-reference-grid regions; checked origin/subsampling ceiling maps; unique rectangular tile coverage; per-component reduction validation; skip unselected tiles and high-resolution packet bodies before Tier-1; deterministic `InvalidSelection`; selected packet/precinct/code-block/output accounting |
| Marker state confusion | Illegal ordering, duplicate required markers, marker in the wrong header, truncated segment | Typed parser states; exhaustive transitions; segment-bounded payloads; deterministic typed errors |
| Tile-part confusion | Contradictory indexes or counts, overlap, reordering abuse, missing data | Validate identities and declared lengths; account each byte once; cap tile-parts; require legal completion |
| Packet iterator explosion | Adversarial progression changes, enormous precinct grids or layers, duplicate packets | Proved termination and index bounds; checked cardinalities; packet/work limits; visited-state accounting where needed |
| Tag-tree pathology | Excessive depth, repeated refinement, invalid node topology | Prevalidated bounded topology; monotonic state; node/work limits; progress invariants |
| Packet-header corruption | Bit stuffing errors, packed-header confusion, runaway inclusion data | Parent-bounded bit readers; explicit stuffed-bit state; header-byte and work limits; exact segment accounting |
| MQ decoder corruption | Invalid state index, renormalization loop, byte-in state confusion | Proved table indexes and termination; bounded reads; explicit sentinel behavior; no unchecked indexing |
| Tier-1 amplification | Excessive passes, bit-planes, segments, or coefficient scans | Validate pass counts and style flags; coefficient/pass/work limits; monotonic segment accounting |
| HT block decoder corruption | Invalid cleanup, refinement, MEL/VLC, MagSgn, SigProp, or MagRef streams | Validate packet/layer pass routing and `Scup`; bound physical regions; apply directional HT stuffing; reject cursor/bit-claim collisions, invalid termination fill, exponent/u overflow, invalid table/symbol paths, edge significance, and work overruns; preserve four-row causal traversal and cleanup-only MagRef eligibility; replace coefficient planes transactionally |
| DWT allocation/access overflow | Oversized levels, odd dimensions, neighbor underflow, temporary-buffer multiplication | Precompute checked shapes; resolution and byte limits; proved lifting bounds; reuse bounded scratch storage |
| Numerical abuse | NaN/Inf propagation or platform-varying irreversible output | Define finite arithmetic path and rounding; reject impossible parameters; conformance tolerances; cross-platform fixtures |
| Component transform overflow | Adversarial MCT graphs, segmented records, dependency coefficients, component-axis wavelets, binary64 coefficients, precision growth, signedness confusion | Require contiguous bounded fragment series with unique initial/final declarations and complete reassembly; require unique bounded MCO DAG order and referenced arrays/records; reject within-stage overlap; execute inverse stages in reverse order; deterministic ties-to-even Q32 conversion; checked i128 products/sums, triangular cardinalities, wavelet partitions, signed canvas parity, rescaling, decoded-scratch-byte, and cumulative work limits; declared-domain validation; typed output |
| JPIP fragment attacks | Overlap, gaps, duplicates, sparse huge offsets, conflicting final lengths | Sorted disjoint interval cache; checked offsets/end arithmetic; accepted-fragment/input-byte limits; identical duplicate/overlap merging; failure-atomic byte/final-length conflicts; completeness only for coverage `[0, final_length)` |
| JPIP progressive/transcode plans | Huge packet plans, missing interior ranges, non-monotonic layers, class 0/1 alias conflicts, forged precinct IDs, malformed packed/length headers, ambiguous tile-part overrides, state advance after failed decode | Packet/input/work/layer limits; exact range reports; transactional logical-bin alias merge; checked SIZ/COD/COC-derived identifier formula; existing packet parser and progression/POC iterator; exact PLT or parsed `Psot` boundary consumption; packet-state rollback on incomplete headers; marker normalization and codestream revalidation; typed ambiguity only when a standard boundary is absent; progressive cursor commits only after successful decode |
| JPIP metadata assembly | Cyclic/deep placeholder graphs, forged box lengths/types/bin IDs, missing leaf bins, codestream substitution, oversized reconstructed containers | Complete-bin requirements and semantic-needs status; checked standard `phld` flags 1/4; active-bin cycle detection; box-depth/input/work bounds; recomputed checked 32/64-bit box lengths; automatic codestream reconstruction; final JP2/JPX/JPH parser validation |
| Metadata abuse | Huge XML, UUID, ICC, palette, labels, or unknown boxes | Global metadata limit; lazy bounded views; no XML or ICC execution; preserve unknown data without interpretation |
| Native oracle compromise | Crafted stream exploits OpenJPEG, Grok, or JasPer | Separate process; ASan/UBSan where supported; no network; read-only corpus; time, memory, file, and output limits |
| Attestation drift | Results generated from different source, standards, tools, or corpus | Hash every input; bind results to one immutable revision; schema validation; fail on stale or missing evidence |

## Failure behavior

All loops must either consume input, advance a bounded state variable, or
return an error. Limit checks happen before allocation or expensive work.
Partial internal state is never returned as a successful image. Trailing bytes
after EOC are rejected for raw codestream input and remain governed by the
containing region for container/databin input.

## Residual risks requiring explicit approval

- Irreversible 9/7 and irreversible component transforms are not claimed to be
  functionally proved in Verus; numerical behavior requires separate
  conformance evidence.
- ICC payloads are preserved but not interpreted by an in-crate color
  management engine.
- Native differential oracles remain memory-unsafe despite process isolation.
- Part 2 and Part 9 have broad state spaces. The JPIP PoC accounts message
  parsing and cached fragment storage, but precinct transcoding, protocol
  state, transport, requests, authentication, and TLS remain outside it.
