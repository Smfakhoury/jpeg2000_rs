# Threat model

## Objective and boundaries

Treat every container/codestream/databin byte and all derived metadata as
untrusted. The objective is deterministic decoding within explicit resource
bounds, without memory unsafety, panics, arithmetic wrap, or state confusion.

Decoded content is not authenticated. The library does not provide DRM,
malware detection, an XML engine, an ICC execution engine, or network security.
Rust, Verus, the solver, their standard-library contracts, and the build
environment remain part of the trusted computing base.

## Threats and controls

| Threat | Controls |
|---|---|
| Forged box/marker lengths, nested structures, truncated input | Parent-bounded slices, checked cursor arithmetic, depth/input/metadata limits, typed errors |
| Image/tile/component origin and dimension overflow | Checked geometry/subsampling, consistency checks before allocation, pixel/component/tile/output limits |
| Packet/progression amplification or duplicate coordinates | Checked cardinalities, bounded candidates, state persistence, packet/work limits and explicit progress measures |
| Malformed tag trees or coding-pass lengths | Validated topology, bounded indexes, incremental state, checked pass/byte accounting |
| MQ/Tier-1 state corruption | Bounded state tables, checked arithmetic, directional stuffing, deterministic end handling, significance/refinement eligibility checks |
| HT cleanup/refinement overlap and forged exponents | Checked Scup partition, physical/fill accounting, a single compatible non-FF fused MEL/VLC data byte excluding the Scup-bearing byte, checked refinement bit claims, bounded reconstruction and transactional writes |
| Transform/decomposition amplification | Bounded references/graphs, stage ordering, checked lifting/matrix arithmetic, scratch/output/work limits |
| Nonfinite or extreme numerical inputs | Explicit numeric encodings/ranges, deterministic fixed-point paths, typed overflow/rejection, independent numerical validation |
| Palette/channel/alpha and composition confusion | Explicit selection/association, bounds and precision checks, checked scaling/clipping, canvas/frame/work limits |
| JPIP sparse fragments and overlapping bytes | Checked offsets/final lengths, sorted interval consistency, idempotent identical overlaps, transactional conflicts, completeness/range status |
| JPIP placeholder graphs or later tile-state changes | Cycle/depth bounds, retained standard metadata, checked reconstructed lengths, exact packet boundaries, missing-data status |
| Unknown or application metadata | Bounded retention and explicit unsupported behavior; no arbitrary payload execution |
| Native reference decoder compromise | Separate subprocess, input/output/time/memory limits, trusted executable selection; never execute codec inputs as commands |
| Validation or proof overclaims | Exact source/tool pins, fail-closed proof checks, explicit skips, preserved disagreements, separation of model refinement from official conformance |

## Failure behavior

Malformed input and valid-but-unsupported capabilities have distinct typed
errors. Resource exhaustion must not produce success-shaped partial output.
APIs that expose progressive availability explicitly distinguish incomplete
input from successful decode.

Every loop or recursive traversal must have a bounded progress argument.
Do not add trusted codec bodies, assumptions, admissions, or unchecked native
execution as a shortcut around verification.

## Validation

Use unit, malformed-prefix, metamorphic, differential, resource, and
coverage-instrumented fuzz tests. Official conformance vectors and numerical
tolerance results are separate requirements; fuzz smoke tests do not prove
coverage. Consult [PROOFS.md](PROOFS.md) for tool limitations and
[validation instructions](validation/VALIDATION.md) for reproducible checks.

Report suspected vulnerabilities using [SECURITY.md](SECURITY.md).
