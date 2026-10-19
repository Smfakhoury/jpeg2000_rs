# Supported decoder profiles

This document describes implementation scope, not certification. JPEG 2000 is
the ISO/IEC 15444 and corresponding ITU-T T.8xx standards family. It is not a
single RFC, and this library does not implement every part of that family.

## Inputs and outputs

| Interface | Behavior |
|---|---|
| Raw codestream | Component decoding and metadata inspection |
| JP2 | Container validation, typed component output, palette/channel mapping, and explicit color conversion |
| JPH | Branding/capability inspection and legacy/HT/mixed component decoding |
| JPX | Explicit indexed codestream, layer, composition, and frame APIs; no implicit first-codestream selection |
| JPIP | Binary messages, sparse databins, range/completeness status, reconstructed decoder inputs, and progressive updates |

Output retains component precision and signedness, image origins, subsampling,
and channel association metadata. Component sample output and converted color
output are distinct interfaces.

## Coding features

The implementation contains:

- Multiple tiles, components, tile parts, and quality layers.
- Signed/unsigned components with nominal precision from 1 to 38 bits.
- LRCP, RLCP, RPCL, PCRL, and CPRL progression and POC windows.
- Precincts, inclusion/zero-bitplane tag trees, coding-pass and segment lengths.
- Main/tile COD, COC, QCD, QCC, and RGN state and precedence.
- PLM/PLT packet lengths, PPM/PPT packed headers, SOP/EPH, and TLM checks.
- Legacy MQ and raw-bypass Tier-1 passes, coding-style routing, and Maxshift undo.
- HT cleanup, significance propagation, magnitude refinement, placeholder and
  replacement pass sets, and mixed legacy/HT routing.
- Reversible 5/3 and fixed-point irreversible 9/7 synthesis.
- Part 2 MCT/MCC/MCO transforms and segmented records; directional/general
  decomposition and kernel metadata via DFS/ADS/ATK; CRG, NLT, and CBD handling.

Every feature is subject to resource, geometry, numeric-range, and profile
validation. A list of implemented code paths is not an independent proof that
every normative requirement of their standard editions is represented.

## Containers, color, and JPIP

JP2 supports component bit depths, palette mappings, channel definitions,
resolution metadata, and ICC profile retention. Color execution is pinned to
enumerated values 16 (sRGB), 17 (grayscale), and 18 (sYCC), including signed,
subsampled, and origin-aware channels and alpha handling.

JPX exposes codestream/header association, layer registration, channel
association, opacity, composition instructions, timing, and indexed frame
rendering. Rendering is bounded and deterministic; it does not provide a
wall-clock animation player or interpret arbitrary application metadata.

JPIP supports checked binary message headers and VBAS fields, out-of-order
fragments, duplicate/overlap consistency, final-length/completeness queries,
metadata placeholders, precinct mapping, packet reconstruction, and
transactional progressive decode. Availability and missing-range results
distinguish incomplete data from successful reconstruction.

## Selection

Selected APIs accept a tile index or a unique rectangular tile set, an absolute
image-reference-grid region, and a native resolution reduction. They validate
component origins/subsampling and skip unnecessary tiles or high-resolution
bodies before Tier-1. Packet framing that must be parsed is still accounted
as input/work.

## Deliberate exclusions

- Encoding.
- JPEG 2000 parts other than the selected decoder families below.
- Network transport, HTTP client/server behavior, TLS, authentication, request
  scheduling, and JPIP cache-model signaling syntax.
- An ICC color-management engine. Profiles are retained without execution.
- Implicit execution of unknown enumerated color spaces or external
  profile-number registry values.
- Arbitrary semantic interpretation of XML, UUID, and application metadata.
- General streaming incremental decoder state and internal parallelism.

Unsupported capabilities remain typed errors. Reserved fields, contradictory
signaling, and malformed structures are not accepted as supported extensions.

## Selected standards and traceability limits

| Part | Reference edition | Scope |
|---|---|---|
| 1 | ISO/IEC 15444-1:2024 / ITU-T T.800 (2024-07) | Core codestream and JP2 |
| 2 | ISO/IEC 15444-2:2023 / ITU-T T.801 (2023-08) | Codestream extensions and JPX |
| 4 | ISO/IEC 15444-4:2024 / ITU-T T.803 (2024-02) | Conformance validation target |
| 9 | ISO/IEC 15444-9:2026 / ITU-T T.808 (2026-06 catalog record) | Selected non-network JPIP behavior |
| 15 | ISO/IEC 15444-15:2019 / ITU-T T.814 (2019-06) | HTJ2K |

Part 2 implementation references include the separately acquired T.801
06/2021 text. That source is not a byte-for-byte record of the selected
08/2023 consolidation; reconciling edition differences remains a traceability
requirement. Confirm the final Part 9 publication record before claiming
edition-level conformance.

Official Part 4 assets are not bundled. Passing unit tests, generated fixtures,
or Verus does not establish official conformance. Fixed-point numerical models
need independently measured tolerances for lossy output.

See [references](REFERENCES.md), [proof boundaries](PROOFS.md), and
[validation instructions](validation/VALIDATION.md).
