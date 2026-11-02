# JPH component decoding incorrectly requires colr when UnkC=1

`DEV-174EC5FEE4` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t814-201906.txt § D.2

**Required behavior:** Decode supported JPH containers to typed component sample planes, preserving component properties independently of the separate explicit color-conversion APIs.

**Observed deviation:** JPH component decoding incorrectly requires colr when UnkC=1

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-174EC5FEE4 --output rfc-results/DEV-174EC5FEE4
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --locked decode::tests::jph_contract::typed_planes_independent_of_colour_conversion -- --exact --nocapture
```

### Test input and recorded failure

Rust-valid byte slices and default options enter the public API, with no private-state corruption, bypassed owning caller or production modifications. Inputs are small, bounded, canonical JPH-branded containers with a single supported Part 15 payload, valid IHDR/BPCC and empty packets. Paired explicit-colour controls pass complete typed image checks. The sole metadata change is the T.814:2019 D.2-permitted combination UnkC=1, absent colr and absent cdef (hence no Typi=0). D.3 BR/MinV/CL are canonical. Max dimensions 16x14, one/three components and one tile are far below hardened limits. Unspecified colour does not require rendering because this API returns native component planes.

**Recorded failure** (matched by the replay runner):

```text
D.2 component-only JPH rejected: MalformedInput { offset: 40, kind: MissingRequiredStructure }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode/tests/jph_contract.rs:L63-L63`](files/src/decode/tests/jph_contract.rs#L63)

```text
    ihdr.extend_from_slice(&[255, 7, u8::from(!colour), 0]);
```

[`src/decode/tests/jph_contract.rs:L70-L75`](files/src/decode/tests/jph_contract.rs#L70-L75)

```text
    if colour {
        let space = if planes.len() == 1 { 17 } else { 16 };
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, space]));
    }
    // D.2: no colr implies UnkC=1 and no channel definition with Typ=0.
    // There is no cdef box here, hence no Typ fields at all.
```

[`src/decode/tests/jph_contract.rs:L163-L165`](files/src/decode/tests/jph_contract.rs#L163-L165)

```text
        let known = container(&cs, size, planes, b"jph \0\0\0\0jph ", true);
        let decoded = decode_jph(&known, &options).expect("supported JPH with explicit colour");
        assert_zero_image(&decoded, size, planes);
```

[`src/decode/tests/jph_contract.rs:L169-L170`](files/src/decode/tests/jph_contract.rs#L169-L170)

```text
        let unspecified = container(&cs, size, planes, b"jph \0\0\0\0jph ", false);
        match decode_jph(&unspecified, &options) {
```

[`src/decode/tests/jph_contract.rs:L181-L184`](files/src/decode/tests/jph_contract.rs#L181-L184)

```text
    assert!(
        missing_colour_failures.is_empty(),
        "UnkC=1, no colr, no cdef must return the same typed planes: {missing_colour_failures:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode/tests/jph_contract.rs`](files/src/decode/tests/jph_contract.rs)

**Build commands** (in order):

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/decode.rs'"'"'); s=p.read_text(); anchor='"'"'#[cfg(test)]\nmod tests {\n'"'"'; assert s.count(anchor)==1; p.write_text(s.replace(anchor, anchor+'"'"'    mod jph_contract;\n'"'"', 1))'
```

```sh
cargo test --lib --locked --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is missing_colour_failures.is_empty() at src/decode/tests/jph_contract.rs:181: every component-only counterpart returns MalformedInput { offset: 40, kind: MissingRequiredStructure }, instead of the same typed planes as its explicit-colour control. This is a reproduced admission deviation from clause 6a3078c49e79034ec262df2c, not a bad pixel oracle or an excessive fixture. Realism was checked first: 24 sequential cases, dimensions at most 16x14, origins at most 3, one or three components, one tile/layer/resolution, no decomposition, shallow box nesting, containers under 200 bytes, and at most 5,376 accounted sample bytes per image even using a deliberately loose three-full-size-plane bound. Default DecodeOptions uses ResourceLimits::HARDENED (1,000,000 per dimension, 100,000,000 pixels, 64 components, 16,384 tiles, 32 box depth, and 2,147,483,648 input/output bytes). Nothing approaches platform or configured capacity. The public validation_decoder likewise passes ordinary file bytes and options directly to decode_jph, and existing native JPH tests use the same public entry with 1x1 images and default options. No caller-owned state or internal fixture is fabricated.

Authority is the rfc_map origin, not agreement with implementation/model oracles. I read its parent finding and re-read pinned ITU-T T.814 (2019-06), hash 172d82969f9eaaa9976802944226579883b36bda02b22c307ee74c7bb8d08e58, D.2 and D.3 via the originating mapping item. D.2 explicitly permits absent Colour Specification with nonzero UnkC, leaving colourspace unspecified and prohibiting Typi=0. The harness sets UnkC=1 and omits cdef entirely, so there is no prohibited Typ field. D.3 branding requirements are satisfied by BR='jph ', MinV=0, CL='jph '. Its otherwise identical control includes colr and is successfully decoded and checked before each failing call; both share matching IHDR/BPCC, the same Part 15 Rsiz/CAP/COD, length-correct tile structure and empty packets. In particular index 0 is an ordinary 1x1 single signed component, not dependent on heterogeneous/subsampled cases. The assertion requires component samples, not implicit conversion of unspecified colour. README separates those APIs, and SUPPORTED.md selects this same Part 15 edition without excluding unspecified-colour component decoding. Thus revising the clause to require colr would hide the standards admission mismatch.

The assessed route is the direct valid unit-contract call decode_jph(&unspecified, &options) -> inspect_jph -> parse_and_inspect -> parse -> parse_jp2_header. parse passes the jp2h payload's base offset (40) without any JPH-specific admission context. parse_image_header accepts and records unknown_color_space=true; parse_jp2_header validates the present BPCC, encounters no palette, then unconditionally evaluates color.ok_or_else(...MissingRequiredStructure). That is the specific erroneous check. The error propagates unchanged through inspection and decode_jph's first '?', preventing raw decoding and returning no planes. Compiler declaration/CFG evidence confirms decode_jph bb1 calls inspect_jph and bb3 -> bb6 -> bb28 returns its rejection before decode_codestream. The host replay built successfully, executed the exact named test, reached all 24 paired controls, and failed at this aggregate check; this attribution does not rely on arbitrary exit status or setup failure. It is an agent-assessed reproduced public-entry deviation, not a formal reachability or general conformance proof.

[Standards quote, production lines, and provenance](../../evidence.md#dev-174ec5fee4)

</details>
