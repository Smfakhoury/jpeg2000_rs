# Metadata inspection rejects valid T.800 (06/2019) PLM framing by treating Nplmi as four bytes

`DEV-4F2CFF1E64` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.7.2

**Required behavior:** Metadata inspection must structurally parse its codestream layout, including supported PLM packet-length metadata, and propagate layout-admission errors rather than return partial success.

**Observed deviation:** Metadata inspection rejects valid T.800 (06/2019) PLM framing by treating Nplmi as four bytes

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-4F2CFF1E64 --output rfc-results/DEV-4F2CFF1E64
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --locked codestream::tests::metadata_contract::metadata_structural_admission_includes_standard_plm -- --exact --nocapture
```

### Test input and recorded failure

The harness uses ResourceLimits::default() and byte constructors, not fabricated Parser internals. Existing siz/codestream_with_headers helpers emit SOC, ordinary Part-1 SIZ with one unsigned 8-bit 64x32 component/tile, COD with nonzero LRCP layers and zero decomposition levels, QCD, a main-header PLM, one SOT with correct Psot/Isot/TPsot/TNsot, SOD, one padded empty packet byte per layer, and EOC. The harness independently checks critical header fields and Psot. No PPM/PPT/SOP/EPH changes packet-length semantics. standard_plm encodes Zplm=0, one 8-bit Nplmi equal to the number of packet-length bytes, and one terminating Iplm=1 per actual packet. Maximum 255 layers/packets and 597-byte input fit default limits. For the first failing case, one packet, Nplmi=1 and an 89-byte stream establish a legal, small direct parse entry.

**Recorded failure** (matched by the replay runner):

```text
legal T.800 PLM=true, PLT=false, layers=1: expected metadata, got MalformedInput { offset: 65, kind: InvalidMarkerLength }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/metadata_contract_tests.rs:L20-L22`](files/src/codestream/metadata_contract_tests.rs#L20-L22)

```text
let mut payload = vec![0, layers];
        payload.extend(vec![1; usize::from(layers)]);
        let result = segment(PLM, &payload);
```

[`src/codestream/metadata_contract_tests.rs:L46-L46`](files/src/codestream/metadata_contract_tests.rs#L46)

```text
let bytes = codestream_with_headers(1, &main, &tile, &vec![0; usize::from(layers)]);
```

[`src/codestream/metadata_contract_tests.rs:L121-L122`](files/src/codestream/metadata_contract_tests.rs#L121-L122)

```text
let limits = ResourceLimits::default();
        let bytes = fixture(layers, plm, plt);
```

[`src/codestream/metadata_contract_tests.rs:L124-L124`](files/src/codestream/metadata_contract_tests.rs#L124)

```text
match parse(&bytes, &limits) {
```

[`src/codestream/metadata_contract_tests.rs:L240-L243`](files/src/codestream/metadata_contract_tests.rs#L240-L243)

```text
for layers in layers_to_check() {
            accepted(&mut failures, layers, true, false);
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/metadata_contract_tests.rs`](files/src/codestream/metadata_contract_tests.rs)

**Build commands** (in order):

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/codestream/tests.rs'"'"'); s=p.read_text(); line='"'"'\ninclude!("metadata_contract_tests.rs");\n'"'"'; p.write_text(s+line) if line not in s else None'
```

```sh
cargo test --lib --locked --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first reported failing check is the accumulated acceptance check for layers=1, PLM=true, PLT=false: parse returns MalformedInput { offset: 65, kind: InvalidMarkerLength } rather than metadata. This is an admissible direct unit-contract entry, not a bypass of an owning caller. Entry realism was checked before tracing: the failing acceptance sweep uses 1..255 layers, one component, one 64x32 tile (2048 pixels), one tile-part, at most 255 one-byte packets, and codestreams of 89..597 bytes; it has no recursive nesting or large fixture arrays. Defaults allow 256 layers, 64 components, 16384 tiles/tile-parts, 67108864 packets, 100000000 pixels and 2147483648 input bytes. These are ordinary bounded codestream inputs, not an implicit-capacity stress case. The real jp2::parse_and_inspect caller similarly passes a codestream box's byte slice and caller limits to this public function.

The origin is rfc_map, not infer. I read its parent finding and manifest, then independently retrieved A.7.2 from the pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, using originating item 7d135fbf69889a164456fada. Table A.35 explicitly specifies Nplmi size 8 bits, values 0..255; A.7.2 permits PLM independently of PLT and defines inline packet lengths to include headers. Thus the fixture's Zplm=0, one-byte Nplmi=1, Iplm=1 is legal for its single one-byte empty packet. SUPPORTED.md expressly claims PLM support. Existing native four-byte PLM helpers and correlated Verus annotations do not override this edition's syntax. No claim is made about an unacquired later edition.

Compiler declaration and MIR CFG confirm parse calls parse_layout and propagates its Err through the residual branch. Source traces parse_layout -> new_parser -> Parser::parse_layout, consumes SOC/SIZ/COD/QCD, then dispatches the PLM at offset 65 to parse_plm. For layers=1 the payload is [0,1,1], length 3, so parse_plm's payload.len()<5 guard immediately returns InvalidMarkerLength. For layers>=3 it instead reads four bytes starting at Nplmi as a u32 and advances four bytes, also incompatible with the standard. The wrapper's error propagation works; the underlying admission error is erroneous, denying metadata for valid supported PLM. This directly violates the clause's supported-PLM admission guarantee, so neither the faithful oracle nor the clause should be revised to match the four-byte dialect. The host-observed assertion and replayed harness, not a nonzero exit alone, establish this route. Combined PLM/PLT exclusion and full pixel decoding are not attributed by this case.

[Standards quote, production lines, and provenance](../../evidence.md#dev-4f2cff1e64)

</details>
