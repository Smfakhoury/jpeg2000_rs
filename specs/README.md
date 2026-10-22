# Specification audit guide

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

## Frozen F5H directional-reader stages

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
The remaining stages are upstream partitioning and pass sequencing, quad
traversal/metadata construction, cleanup block reconstruction, significance/refinement
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
