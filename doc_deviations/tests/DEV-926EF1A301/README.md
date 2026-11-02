# Nonzero Psot shorter than retained tile-part header is accepted as an unspecified data length

`DEV-926EF1A301` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.4.2

**Required behavior:** For every SOT-bearing header with SOT offset p and retained header length H (SOT through SOD inclusive), after its framing and earlier marker checks succeed: if its unsigned Psot value L is nonzero and L<H, parsing must return MalformedInput/InvalidTilePart at offset p, not a transition with an unspecified data length. On successful return, every such transition with nonzero Psot has data_length=Some(L-H), including Some(0) when L=H. A zero Psot has data_length=None and is legal only for the final header. Thus, in this SOT-bearing representation, None never hides an impossible nonzero declared length. These claims concern original packet-data length, not the number of supplied databin header bytes and not the PLT packet count.

**Observed deviation:** Nonzero Psot shorter than retained tile-part header is accepted as an unspecified data length

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-926EF1A301 --output rfc-results/DEV-926EF1A301
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/tile_transition_suite.py run contract_transition_psot_lengths
```

### Test input and recorded failure

A safe direct unit-contract invocation supplies a live immutable slice and matching expected_tile. SOT's length field is 10, so the complete SOT occupies 12 bytes; SOD occupies bytes 12..14. There are no intermediate segments or packet-data requirements. Psot=13 fits usize and is intentionally semantically malformed, not invalid fixture memory: validating this condition belongs to the target, whose entry has no precondition excluding malformed lengths. Part index 5 and total count 0 do not violate this helper's entry requirements. The single 14-byte header and tiny associated vectors are comfortably within resource limits; no production mutation or unsafe access is introduced.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: route=parse_tile_part_transitions(tile=7); input=[ff, 90, 00, 0a, 00, 07, 00, 00, 00, 0d, 05, 00, ff, 93]
  left: Ok([JpipTilePartTransition { tile: 7, tile_part: 5, packet_count: None, data_length: None, functional: false, header: [255, 144, 0, 10, 0, 7, 0, 0, 0, 13, 5, 0, 255, 147] }])
 right: Err(MalformedInput { offset: 0, kind: InvalidTilePart })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_transition_contract_tests.rs:L3-L11`](files/src/jpip_transition_contract_tests.rs#L3-L11)

```text
fn contract_part(tile: u16, part: u8, psot: u32, markers: &[u8]) -> Vec<u8> {
    let mut payload = tile.to_be_bytes().to_vec();
    payload.extend_from_slice(&psot.to_be_bytes());
    payload.extend_from_slice(&[part, 0]);
    let mut header = segment(0xff90, &payload);
    header.extend_from_slice(markers);
    header.extend_from_slice(&[0xff, 0x93]);
    header
}
```

[`src/jpip_transition_contract_tests.rs:L234-L245`](files/src/jpip_transition_contract_tests.rs#L234-L245)

```text
    for markers in [vec![], segment(0xff64, &[0, 0, 3])] {
        let h = 14 + markers.len();
        for length in [h as u32 - 1, 1] {
            let bad = contract_part(7, 5, length, &markers);
            for prefix in [vec![], contract_part(7, 0, 14, &[])] {
                let offset = prefix.len();
                contract_error(
                    &[prefix, bad.clone()].concat(),
                    7,
                    offset,
                    MalformedKind::InvalidTilePart,
                );
```

[`src/jpip_transition_contract_tests.rs:L50-L60`](files/src/jpip_transition_contract_tests.rs#L50-L60)

```text
    let result = parse_tile_part_transitions(input, tile);
    assert_eq!(input, before, "error path mutated input");
    assert_eq!(sentinel, vec![0x31; 97], "error path mutated caller state");
    assert_eq!(
        result,
        Err(DecodeError::MalformedInput {
            offset: offset as u64,
            kind
        }),
        "route=parse_tile_part_transitions(tile={tile}); input={input:02x?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_transition_contract_tests.rs`](files/src/jpip_transition_contract_tests.rs)
- [`validation/tile_transition_access.patch`](files/validation/tile_transition_access.patch)
- [`validation/tile_transition_suite.py`](files/validation/tile_transition_suite.py)

**Build commands** (in order):

```sh
python3 validation/tile_transition_suite.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failure is the result equality in contract_error at src/jpip_transition_contract_tests.rs:53: a complete 14-byte SOT/SOD header declaring Psot=13 returns Ok with data_length=None instead of Err(MalformedInput { offset: 0, kind: InvalidTilePart }). The earlier input-preservation and sentinel assertions passed. This directly refutes clause 20f954947eed5f28a085ba7e's nonzero L<H rejection, not merely its inferred choice of error category: any successful transition here violates the documented distinction between a nonzero original length and the legal zero-Psot unknown-length form.

Resource/entry realism: the failing call borrows one 14-byte Vec-backed immutable slice, contains one 12-byte SOT and one 2-byte SOD, uses tile 7 and part 5 with an unspecified total-part count, and has no recursion, threads, pool demand, or packet-body allocation. This is far below the configured HARDENED input limit of 2,147,483,648 bytes and tile-part limit of 16,384. The selected Psot case's preceding positive inputs have headers no larger than 273 bytes (at most two retained headers in the zero-nonfinal checks); even u32::MAX is only a declared scalar length, never an allocation request. These are ordinary header-sized buffers like those supplied by DatabinCache::tile_part_transitions, which passes complete cached bytes directly to this parser without validating Psot. No entry contract requires valid declared lengths, sequential part numbers, or the presence of original packet bytes: this helper validates retained headers, and such a malformed declaration is precisely the rejection domain under test. The direct unit-contract call therefore does not skip an owning caller required to establish state. Rust shared borrows and all accessed segment/read bounds are valid.

The replay builds the native library tests and adds access only inside the existing cfg(test) module. Its other patch changes only a pre-existing test fixture's declared lengths; no production parser behavior is altered. The failure occurs after that existing positive fixture and the generated valid-length/zero-form checks complete.

Compiler declaration confirms the immutable slice/u16 entry and owned Result return. The CFG follows the SOT branch, checks segment size 12 and matching tile, reads Psot=13, recognizes SOD at offset 12 and advances to 14. The nonfinal-zero rejection is inapplicable. It calculates H=14 in bb144-148, converts the nonzero declaration in bb151-155, then checked_sub(13,14) produces None in bb155; Option::map in bb157 preserves None, bb158-164 pushes that transition, and bb188 -> bb46 -> bb165 returns Ok. Source documentation explicitly reserves None for legal zero Psot, so this is not a spec_error permitting lenient interpretation. The exact InvalidTilePart category remains inferred from neighboring structural validation, as the reviewed clause discloses, but that uncertainty cannot explain acceptance. The sentinel check is weak evidence of general framing and is not relied on; the actual host-observed Ok result, exact input, native harness call, and CFG ground this agent-assessed reproduced deviation.

[Standards quote, production lines, and provenance](../../evidence.md#dev-926ef1a301)

</details>
