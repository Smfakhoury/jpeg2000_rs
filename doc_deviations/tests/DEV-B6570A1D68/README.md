# Metadata inspection rejects standard 8-bit PLM length framing

`DEV-B6570A1D68` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.7.2

**Required behavior:** Codestream metadata inspection must include supported PLM/PLT layout admission and preserve layout-parser errors without fabricating successful metadata.

**Observed deviation:** Metadata inspection rejects standard 8-bit PLM length framing

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-B6570A1D68 --output rfc-results/DEV-B6570A1D68
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --locked codestream::tests::metadata_contract::metadata_plm_plt_admission_and_error_propagation -- --exact --nocapture
```

### Test input and recorded failure

Safe Rust byte vectors and ResourceLimits::default(); no private parser state fabricated. Existing baseline helpers construct SOC/SIZ/COD/QCD, exactly one SOT/SOD/EOC tile-part with patched Psot and valid 64x32 one-component geometry. The added standard_plm uses 8-bit Nplmi per pinned T.800 A.7.2. At layers=1 the payload [0,1,1] describes the same one-byte packet as PLT [0,1], and inline tile data [0] encodes an empty, byte-aligned packet under B.9/B.10.3. All fixture and Psot assertions completed. The failing stream is 95 bytes and all positive sweep cases are at most 862 bytes and 255 layers/packets, below default resource limits. Test-only inclusion changes no production behavior.

**Recorded failure** (matched by the replay runner):

```text
legal T.800 PLM=true, PLT=true, layers=1: expected metadata, got MalformedInput { offset: 65, kind: InvalidMarkerLength }
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

[`src/codestream/metadata_contract_tests.rs:L173-L173`](files/src/codestream/metadata_contract_tests.rs#L173)

```text
accepted(&mut failures, layers, true, true);
```

[`src/codestream/metadata_contract_tests.rs:L175-L175`](files/src/codestream/metadata_contract_tests.rs#L175)

```text
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

The first failing check is the aggregated acceptance assertion at metadata_contract_tests.rs:175, whose first recorded mismatch is layers=1, PLM=true, PLT=true: expected metadata but received MalformedInput { offset: 65, kind: InvalidMarkerLength }. This is a valid direct unit-contract entry to parse, not a bypass of a required owning caller. Real callers jp2::parse_and_inspect and decode::decode_jpx_file likewise pass a raw codestream byte slice and resource limits directly; neither imposes an alternate PLM encoding. Realism was checked before tracing: the first failing stream is only 95 bytes, 64x32 (2048 pixels), one unsigned 8-bit component, one tile/part/resolution/precinct/layer/packet, with one inline empty packet byte. The complete sweep has at most 862 input bytes, 255 layers/packets, one tile-part and no nesting; defaults allow 2147483648 input bytes, 16777216 metadata bytes, 256 layers, 67108864 packets, 16384 tile-parts, and 100000000 pixels. These do not approach an implicit platform, stack, pool or configured limit. The separately tightened packet-limit negative case is intentional and is not the observed failure.

Authority is the rfc_map origin, not agreement with existing native fixtures or Verus models. I read the pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, A.7.2 through the originating mapping item. Table A.35 specifies Nplmi as 8 bits, and the text explicitly permits PLM and PLT together or separately. Its note limits the PLM description to 255 bytes/packets per tile-part; this test respects that limit. B.9 and B.10.3 independently justify the byte-aligned empty packets. The first PLM is FF57 0005 00 01 01: Zplm=0, Nplmi=1, Iplm=1, matching the tile-header PLT and the single inline packet. No conflicting declarations, unsupported profile, malformed segment length or packet truncation is constructed.

Route: test -> accepted(...,1,true,true) -> fixture -> parse -> parse_layout -> new_parser -> Parser::parse_layout main-header PLM arm -> parse_plm. At offset 65 the valid PLM payload is [0,1,1], length 3. parse_plm incorrectly requires at least five payload bytes, immediately returning InvalidMarkerLength. For larger legal groups its four-byte read of Nplmi likewise treats an 8-bit count and packet bytes as a u32. The main-header '?' and parse wrapper propagate that rejection; MIR confirms the error branch. Consequently standard, supported packet-length metadata prevents successful metadata inspection, violating the admission part of the exact clause. Error propagation itself works; it is not the deviation. The pre-existing plm test helper uses the same four-byte dialect and cannot override the cited standard. The separate PLM/PLT mutual-exclusion branch identified by the parent finding is not reached in this replay and is not claimed reproduced here. The harness's route print alone is not reachability proof; the specific observed rejection, exact fixture bytes, main-header dispatch and callee guard establish this assessed route.

[Standards quote, production lines, and provenance](../../evidence.md#dev-b6570a1d68)

</details>
