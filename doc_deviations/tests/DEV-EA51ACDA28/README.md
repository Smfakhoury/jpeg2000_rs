# Inline EPH inspection rejects a valid header with terminal FF stuffing

`DEV-EA51ACDA28` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.8.2

**Required behavior:** Apply the template coding-style EPH policy during inline packet inspection, propagate marker errors, and include a validated EPH in the successful consumed prefix.

**Observed deviation:** Inline EPH inspection rejects a valid header with terminal FF stuffing

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-EA51ACDA28 --output rfc-results/DEV-EA51ACDA28
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/deepspec_jpip_contract/runner --exact packet::tests::jpip_contract_eph_policy_errors_and_prefix --nocapture
```

### Test input and recorded failure

A bounded safe-Rust inspection-unit fixture: one fresh packet state, one context and decoder, one valid 1x1 LL subband with fresh inclusion/zero-bitplane trees. The helper sets context maximum_zero_bitplanes=8 and constructs a matching eight-bitplane regular decoder using the real constructor. Header bits encode first inclusion, ZBP=6<8, one pass, Lblock=8, length=255. Mandatory terminal stuffing is 00, followed by enabled EPH FF92 and all 255 declared opaque body bytes. Layer zero is valid, component zero has a valid style, packed headers and SOP are absent, all default limits are respected. Inspection does not execute Tier-1, so entropy validity of the body is not a premise.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: inline inspection result/error/prefix
  left: Err(MalformedInput { offset: 0, kind: InvalidPacketHeader })
 right: Ok(261)
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/packet_jpip_contract_tests.rs:L61-L65`](files/src/packet_jpip_contract_tests.rs#L61-L65)

```text
let mut precinct = literal_packet_precinct();
    precinct.contexts[0].maximum_zero_bitplanes = 8;
    precinct.decoders[0] = Tier1CodeBlockDecoder::new(
        1, 1, 8, SubbandOrientation::Ll, Tier1CodingStyle::default(),
    ).unwrap();
```

[`src/packet_jpip_contract_tests.rs:L75-L80`](files/src/packet_jpip_contract_tests.rs#L75-L80)

```text
assert_eq!(bits.len(), 24);
    assert_eq!(pack_packet_bits(&bits), [0xc0, 0xbe, 0xff]);
    let header = [0xc0, 0xbe, 0xff, 0];
    assert_eq!(header[3] & 0x80, 0, "mandatory stuffed zero");
    let body = vec![0xff; 255];
    (precinct, jpip_framed(&header, &body, false, eph, 0))
```

[`src/packet_jpip_contract_tests.rs:L205-L208`](files/src/packet_jpip_contract_tests.rs#L205-L208)

```text
let (mut precinct, data) = jpip_terminal_ff_packet(true);
    assert_eq!(&data[4..6], &[0xff, 0x92], "EPH follows the complete stuffed header");
    jpip_assert_inspect(&jpip_template(false, true), &data, 0, 0, &mut precinct,
        &limits, Ok(4 + 2 + 255));
```

[`src/packet_jpip_contract_tests.rs:L50-L54`](files/src/packet_jpip_contract_tests.rs#L50-L54)

```text
assert_eq!(
        decode_jpip_precinct_packet_inner(template, bytes, sequence, layer, precinct, limits),
        expected,
        "inline inspection result/error/prefix"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/packet_jpip_contract_tests.rs`](files/src/packet_jpip_contract_tests.rs)
- [`jpip_contract_registration.patch`](files/jpip_contract_registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jpip_contract_registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/packet.rs'"'"').read_text(); t=Path('"'"'src/packet_jpip_contract_tests.rs'"'"').read_text(); assert s.count('"'"'include!("packet_jpip_contract_tests.rs");'"'"')==1; assert t.count('"'"'#[test]'"'"')==7; assert '"'"'let header = [0xc0, 0xbe, 0xff, 0];'"'"' in t; assert '"'"'Ok(4 + 255)'"'"' in t; assert '"'"'Ok(4 + 2 + 255)'"'"' in t; assert '"'"'fn jpip_assert_parser_accounting('"'"' in t; assert '"'"'assert_eq!(physical, logical,'"'"' in t; assert '"'"'actual_jpip_outer_restores_all_raw_fields_and_clone_preserves_unused_tree_words();'"'"' in t; assert '"'"'full_progression_uses_real_ppt_reassembly_and_plt_iteration_without_tile_body_bytes();'"'"' in t'
```

```sh
cargo test --locked --lib --no-run --target-dir target/deepspec_jpip_contract
```

```sh
python3 -c 'from pathlib import Path; import os,shutil,subprocess; d=Path('"'"'target/deepspec_jpip_contract'"'"'); a=[p for p in (d/'"'"'debug/deps'"'"').glob('"'"'jpeg2000_rs-*'"'"') if p.is_file() and os.access(p,os.X_OK)]; assert len(a)==1,a; r=d/'"'"'runner'"'"'; shutil.copy2(a[0],r); names=subprocess.check_output([str(r),'"'"'--list'"'"'],text=True); assert names.count('"'"'packet::tests::jpip_contract_'"'"')==7,names'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is src/packet_jpip_contract_tests.rs:50, reached by the terminal-FF case at lines 205-208: the host observed Err(MalformedInput { offset: 0, kind: InvalidPacketHeader }) instead of Ok(261). This violates clause 62986c33c311dfc825987719's EPH validation/consumed-prefix guarantee on an admissible direct inspection-unit entry, not a build failure or Tier-1 decode failure. Capacity check: this case supplies 261 bytes (four header bytes, two EPH bytes, 255 body bytes), one code block, one subband, two one-node trees, one new pass, layer 0 of two, and eight maximum bitplanes. These are far below default limits (2,147,483,648 input bytes, 268,435,456 header bytes, 67,580 code-block bytes, 16,777,216 code blocks/tag-tree nodes, 874 passes, 256 layers, and 1,000,000,000 work units); there is no deep recursion or oversized fixture. The fresh state, single-leaf trees, matching vector lengths, coordinates and eight-bitplane decoder are consistent with the normal PacketPrecinctDecoder::new/new_subbands construction for one block. It is an inspection contract: opaque declared body bytes need not establish entropy-decoder success because the adapter explicitly passes decode_tier1=false. No owning-caller preparation is skipped that this direct unit contract requires.
Route: jpip_contract_eph_policy_errors_and_prefix -> jpip_terminal_ff_packet(true) -> jpip_assert_inspect -> decode_jpip_precinct_packet_inner. Its MIR bb1 installs layer zero; bb12 constructs inline data with absent packed headers and invokes decode_precinct_packet at offset zero, SOP disabled, EPH enabled, Tier-1 disabled. Contribution parsing consumes exactly 24 information bits C0 BE FF: presence and first inclusion, six zero bitplanes, one pass, Lblock incremented from 3 to 8, length 255. The fourth byte 00 is the mandatory final stuffed/padded byte. PacketBitReader only loads a following stuffed byte when another information bit is requested. align_to_byte merely clears bits_remaining, so after_headers obtains header_bytes=3, then validates packet_data[3..5]=00 FF as EPH instead of the actual FF92 at [4..6]. The invalid-marker branch returns InvalidPacketHeader, propagated unchanged by the inner adapter's MIR bb16. The exact failed check is marker validation at the incorrectly computed boundary, not a failure in fixture setup.
Authority: clause_origin is rfc_map, not infer. I read its parent finding and manifest, and independently read cited T.800 (06/2019) B.10.1 and A.8.2 through the document tools. B.10.1 expressly requires a stuffed zero even when FF would otherwise be the last header byte; A.8.2 places FF92 immediately after the complete packet header when COD signals EPH. The harness meets both. The parent finding explicitly excluded the final-FF boundary from its implemented assessment; that is a limitation of that assessment, not an input illegality or a caller requirement that the implementation already compute the boundary correctly. Accordingly this is not grounds to revise a sound standards rule to match the correlated native/model accounting. Edition reconciliation with selected T.800 (2024) remains outstanding as recorded in specs/README.md: this diagnosis is grounded in the cited 2019 edition and the stated inspection clause, not a certification of 2024 conformance or a fully decoded JPIP codestream.

[Standards quote, production lines, and provenance](../../evidence.md#dev-ea51acda28)

</details>
