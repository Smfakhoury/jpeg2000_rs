# PLM parser rejects standard one-octet Nplm framing as InvalidMarkerLength

`DEV-C56978D32B` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.7.2

**Required behavior:** PLM parsing shall interpret encoded per-tile-part packet-length groups from the main-header marker bytes and preserve their wire order for tile-part consumers.

**Observed deviation:** PLM parser rejects standard one-octet Nplm framing as InvalidMarkerLength

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-C56978D32B --output rfc-results/DEV-C56978D32B
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/plm-standard-tests codestream::tests::standard_plm::standard_plm_octet_counts_and_wire_order --exact --nocapture
```

### Test input and recorded failure

This is a permitted direct unit-contract test of metadata interpretation. The real constructor supplies valid parser references, lifetime, limits and zero counters; the real segment reader supplies a correctly bounded raw PLM segment at offset 0. Zplm and next_index both equal 0 and groups starts empty, matching the actual main-header owner's state. Case 1 has Lplm=5, a one-byte Nplm value 1 and one completed positive codeword 1, conforming to pinned T.800 (06/2019) A.7.2 Tables A.35/A.36. Its seven total bytes, one packet and one group are well within platform/configuration limits. No owning-caller normalization is skipped, and production code is unchanged by the test-only child-module patch.

**Recorded failure** (matched by the replay runner):

```text
case 1: valid A.35 octet framing rejected: MalformedInput { offset: 0, kind: InvalidMarkerLength }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/tests/standard_plm.rs:L6-L8`](files/src/codestream/tests/standard_plm.rs#L6-L8)

```text
let encoded = encode_packet_lengths(lengths);
    let mut group = vec![u8::try_from(encoded.len()).expect("Nplm <= 255")];
    group.extend(encoded);
```

[`src/codestream/tests/standard_plm.rs:L36-L38`](files/src/codestream/tests/standard_plm.rs#L36-L38)

```text
let mut cases = vec![
        vec![vec![]],
        vec![vec![1]],
```

[`src/codestream/tests/standard_plm.rs:L66-L72`](files/src/codestream/tests/standard_plm.rs#L66-L72)

```text
let bytes = standard_marker(0, &body);
        let mut parser = new_parser(&bytes, &limits).unwrap();
        let raw = parser.read_segment().unwrap();
        assert_eq!(raw.marker, PLM);
        assert_eq!(raw.offset, 0);
        let mut next = 0;
        let mut groups = Vec::new();
```

[`src/codestream/tests/standard_plm.rs:L74-L79`](files/src/codestream/tests/standard_plm.rs#L74-L79)

```text
if let Err(error) = parser.parse_plm(raw, &mut next, &mut groups) {
            failures.push(format!(
                "case {case}: valid A.35 octet framing rejected: {error:?}"
            ));
            continue;
        }
```

[`src/codestream/tests/standard_plm.rs:L97-L97`](files/src/codestream/tests/standard_plm.rs#L97)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/tests/standard_plm.rs`](files/src/codestream/tests/standard_plm.rs)
- [`validation/plm-standard-tests.patch`](files/validation/plm-standard-tests.patch)

**Build commands** (in order):

```sh
bash -c 'set -e; patch --batch --fuzz=0 -p1 < validation/plm-standard-tests.patch; cargo test --locked --lib --no-run; for binary in target/debug/deps/jpeg2000_rs-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" target/plm-standard-tests; exit 0; fi; done; exit 127'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing check in this replayed test is its final assert!(failures.is_empty(), "{}", failures.join("\n")); the preceding framing/read_segment assertions succeeded. The host tail truncates the case-0 diagnostic, so the grounded concrete witness is case 1: a single packet of length 1, encoded as marker bytes FF 57 00 05 00 01 01 (payload [Zplm=0,Nplm=1,Iplm=1]). This is a valid direct unit-contract entry, not a fixture bypassing an owning caller. new_parser initializes the actual parser and counters, read_segment returns the actual PLM Segment, next_index is zero, and groups is empty, exactly as initialized by the main-header owner. No full packet body is required by this metadata helper. Real parse_layout dispatch passes the raw segment to parse_plm with no conversion of Nplm to four bytes.

Realism was assessed before tracing the implementation: the 38 independent unit fixtures have at most three groups, at most 450 encoded information bytes and at most 458 total marker bytes each; the fixed 255-packet fixture uses 255 encoded bytes, and generated groups contain at most 30 codewords each. Nplm never exceeds 255, there is no recursive nesting or packet-body allocation proportional to numeric lengths, and all counters start at zero. These inputs are far below default limits of 2 GiB input, 16384 tile-parts, 67108864 packets, 268435456 packet-header bytes and 1000000000 work units. The seven-byte case-1 witness is itself ordinary caller metadata, independent of any large numeric case.

Authority is the rfc_map origin, not the correlated implementation-derived contracts/model/native four-byte oracle. I read its parent finding and the pinned T.800 (06/2019) A.7.2 section (SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e). Its normative Table A.35 expressly assigns Nplmi 8 bits, 0..255, and Lplm 4..65535; Table A.36 gives completed MSB-first seven-bit packet length codewords. Thus case 1 meets the cited standard and the clause's ordinary PLM main-header applicability. This is not PPM's distinct 32-bit count syntax. No claim about unaudited later-edition deltas is needed for this cited-edition finding.

describe_declaration identifies the exact helper and parameter types. Its MIR bb0 tests payload length <5; bb1/bb2 construct and return InvalidMarkerLength before any index increment, validator, or group publication. Case 1 has three payload bytes and necessarily takes this route. Source also reads Nplm with read_u32 and advances position by four on larger payloads, revealing the same wrong-width framing root cause. Rejecting this valid standard group violates the clause's requirement to interpret the main-header packet-length groups. It is not test_error: the test neither requires an invalid entry nor mistakes the implementation's four-byte framing for a normative precondition. It is not spec_error: the assigned standard and advertised PLM support sustain the clause. The concrete observed consequence is rejection, not independently demonstrated reordering or a continuation-specific defect; successful ordering checks are blocked by the framing error. The truncated first diagnostic is a weak diagnostic only, since the case-1 line and replayed harness provide an unambiguous failing target call.

[Standards quote, production lines, and provenance](../../evidence.md#dev-c56978d32b)

</details>
