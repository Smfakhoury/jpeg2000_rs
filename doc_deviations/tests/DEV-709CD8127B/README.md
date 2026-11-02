# SOP-enabled progression decoding rejects standards-permitted omission of SOP

`DEV-709CD8127B` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.8.1

**Required behavior:** Progression precinct decoding routes the tile-part coding style and expected tile packet sequence through the documented packet-decoding model, including its SOP-prefix validation policy.

**Observed deviation:** SOP-enabled progression decoding rejects standards-permitted omission of SOP

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-709CD8127B --output rfc-results/DEV-709CD8127B
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/deepspec-progression-suite --exact packet::tests::progression_contract_sop_policy_and_optional_marker --nocapture
```

### Test input and recorded failure

A tiny valid first-packet unit entry: a parsed single-tile Part-1 stream, legal COD Scod=2, one empty header byte 0x00, no EPH or packed source, all initial cursors/counters unchanged at zero, coherent one-block literal precinct, valid component/style and constructor-derived grid/input/progress, default resource limits. A.8.1 in the assigned 2019 edition permits SOP omission on this packet. Absence of SOP is the legal input being exercised, not a violated entry requirement. No body decoding or unsafe state fabrication is involved.

**Recorded failure** (matched by the replay runner):

```text
T.800 A.8.1 permits absent SOP with Scod bit 1 set: packed=false, flags=2, actual=Err(MalformedInput { offset: 0, kind: Truncated }), expected=Ok((PacketBodyDecode { present: false, code_blocks: 1, contributions: 0, segments: 0, passes: 0, header_bytes: 1, body_bytes: 0, bytes_consumed: 1 }, 1, 0))
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/packet_progression_contract_tests.rs:L421-L430`](files/src/packet_progression_contract_tests.rs#L421-L430)

```text
    for packed in [false, true] {
        for flags in [0_u8, 2, 4, 6] {
            let eph = flags & 4 != 0;
            let mut header = vec![0];
            if eph {
                header.extend_from_slice(&[0xff, 0x92]);
            }
            let tile: &[u8] = if packed { &[] } else { &header };
            let stream = contract_codestream(flags, 1, tile, packed.then_some(header.as_slice()));
            let layout = contract_layout(&stream, flags, 1, packed.then_some(header.as_slice()));
```

[`src/packet_progression_contract_tests.rs:L433-L442`](files/src/packet_progression_contract_tests.rs#L433-L442)

```text
            let actual = contract_progression_call(
                &layout.tile_parts,
                None,
                contract_coordinate(0),
                0,
                0,
                0,
                None,
                &mut precinct,
            );
```

[`src/packet_progression_contract_tests.rs:L97-L97`](files/src/packet_progression_contract_tests.rs#L97)

```text
    let input = prepare_progression_payloads(parts, parts[0].tile_index, &limits).unwrap();
```

[`src/packet_progression_contract_tests.rs:L117-L117`](files/src/packet_progression_contract_tests.rs#L117)

```text
    let mut progress = progression_tile_progress_new(&params).unwrap();
```

[`src/packet_progression_contract_tests.rs:L127-L129`](files/src/packet_progression_contract_tests.rs#L127-L129)

```text
    decode_progression_precinct_packet(
        &params, &parts[0], &progress, coordinate, expected, precinct,
    )
```

[`src/packet_progression_contract_tests.rs:L522-L522`](files/src/packet_progression_contract_tests.rs#L522)

```text
    assert!(omissions.is_empty(), "{}", omissions.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/packet_progression_contract_tests.rs`](files/src/packet_progression_contract_tests.rs)
- [`validation/deepspec-progression-registration.patch`](files/validation/deepspec-progression-registration.patch)
- [`validation/build-progression-contract.py`](files/validation/build-progression-contract.py)

**Build commands** (in order):

```sh
python3 validation/build-progression-contract.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the final omissions.is_empty() check at src/packet_progression_contract_tests.rs:522. Its first collected mismatch is packed=false, flags=2: a complete one-byte empty-packet header produces Truncated instead of Ok with one tile byte consumed. This is a code deviation under the clause's rfc_map authority, not a bad oracle inferred from native/model agreement. I read clause_origin and independently retrieved its original T.800 (06/2019), ISO/IEC 15444-1:2019 A.8.1 passage, version 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, through the originating rfc_map item. It explicitly says that when COD allows SOP, each packet may or may not have an SOP segment; Nsop still counts every packet. It also explicitly permits SOP placement before tile-part packet bodies with PPT/PPM. Thus the intended SOP policy is permission, not mandatory presence. The clause's generic model-routing wording is interpreted with that parent finding; the existing model/callee summaries that charge six SOP bytes whenever enabled share the implementation restriction and are not caller preconditions overriding the standard. SUPPORTED.md advertises SOP/EPH support without documenting this mandatory-per-packet restriction.

Entry realism was checked: the failing case has one 1x1 image/component/tile, one resolution/layer/precinct/code-block, a one-byte empty header, zero body bytes, and counters/cursors all zero. Serialized omission streams are below 100 bytes, with no recursive/deep structure, versus default/HARDENED capacities of 2 GiB input, 268435456 header bytes, 16777216 blocks, 67108864 packets and 1000000000 work units. No excessive allocation or stack demand is involved. The fixture serializes main-header COD and QCD, a correctly sized SOT/SOD tile-part and EOC, parses it using parse_layout, and prepares packed payloads, grid and progress using production constructors. The reused literal precinct has one state/context/1x1 decoder and one nonempty subband, matching component/resolution/precinct zero. The first failing case is inline, so packed-placement edge cases cannot account for it. Although its helper allows direct counter overrides for other tests, this failure writes only the zero values already produced by progression_tile_progress_new. It is an admissible direct unit-contract entry, not a forged caller history: execute_progression_packet also passes the first unseen coordinate, initial cursors and expected=None for absent PLM/PLT. No prerequisite requires six marker bytes merely because SOP is allowed.

Compiler declaration and MIR CFG establish the target route: set layer zero, fetch packed[0], take selection=None (decode=true), derive expected sequence zero, and tail-call decode_precinct_packet. Its shape/resource/offset/style checks succeed and it passes sop=true from the parsed COD flag into decode_precinct_packet_payload. That calls decode_precinct_packet_payload_prefix before header decoding. The prefix unconditionally attempts data.get(0..2) under parameters.sop; the legal inline data=[0] therefore returns Truncated at offset zero before reading its false presence bit. The replay's specific error, not merely exit 101, matches this branch. Legal SOP omission with EPH and/or packed headers is likewise rejected in the same replay. The consequence is rejection of legal SOP-enabled Part-1 packet input. Truncated is a misleading diagnostic for this complete unmarked packet, not separate evidence of malformed setup. This attribution is scoped to the replayed native unit route and cited edition; it is not a formal whole-decoder reachability/conformance proof.

[Standards quote, production lines, and provenance](../../evidence.md#dev-709cd8127b)

</details>
