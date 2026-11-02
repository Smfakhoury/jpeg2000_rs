# Initial standard dependent JPIP header rejected instead of using zero identifier defaults

`DEV-772E004106` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Consume standard binary message-header forms, including inherited identifiers, using a fresh response-local parsing context.

**Observed deviation:** Initial standard dependent JPIP header rejected instead of using zero identifier defaults

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-772E004106 --output rfc-results/DEV-772E004106
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::contract_ingest_standard_headers_and_response_local_defaults -- --exact --nocapture
```

### Test input and recorded failure

The caller uses DatabinCache::new(ResourceLimits::default()), DatabinId::new(0,0,0), and safe ingestion only. For the first failing instance, one preexisting metadata databin holds 'm'; the addressed precinct databin is absent. 0x30 encodes permitted form 01, final=true, bin=0; offset=0, length=1 and payload='p' are complete, followed by EOR [0,2,0]. Both missing identifiers default to zero under the cited T.808 A.2.1. Seven input bytes, two eventual stored bytes, and two eventual accepted fragments are far within configured capacities. No transport-packet constraint is asserted: this is a complete ordered de-framed response, and this safe public entry owns parser creation.

**Recorded failure** (matched by the replay runner):

```text
T.808 A.2.1 initial inherited-header defaults violated:
primed=false, bin=0: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_ingest_contract_tests.rs:L96-L96`](files/src/jpip_ingest_contract_tests.rs#L96)

```text
let mut cache = DatabinCache::new(ResourceLimits::default());
```

[`src/jpip_ingest_contract_tests.rs:L129-L131`](files/src/jpip_ingest_contract_tests.rs#L129-L131)

```text
let target = DatabinId::new(0, 0, bin).unwrap();
            let mut wire = contract_data(1, target, 0, b"p", true);
            wire.extend(contract_eor(2, b""));
```

[`src/jpip_ingest_contract_tests.rs:L133-L133`](files/src/jpip_ingest_contract_tests.rs#L133)

```text
match ingest_jpip_response(&wire, &mut cache) {
```

[`src/jpip_ingest_contract_tests.rs:L141-L141`](files/src/jpip_ingest_contract_tests.rs#L141)

```text
Err(error) => failures.push(format!("primed={primed}, bin={bin}: {error:?}")),
```

[`src/jpip_ingest_contract_tests.rs:L156-L160`](files/src/jpip_ingest_contract_tests.rs#L156-L160)

```text
assert!(
        failures.is_empty(),
        "T.808 A.2.1 initial inherited-header defaults violated:\n{}",
        failures.join("\n")
    );
```

[`src/jpip_ingest_contract_tests.rs:L2-L2`](files/src/jpip_ingest_contract_tests.rs#L2)

```text
// These are de-framed, ordered JPP message sequences, not HTTP-UDP packets.
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_ingest_contract_tests.rs`](files/src/jpip_ingest_contract_tests.rs)
- [`validation/register-jpip-contract-tests.py`](files/validation/register-jpip-contract-tests.py)

**Build commands** (in order):

```sh
python3 validation/register-jpip-contract-tests.py
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() at src/jpip_ingest_contract_tests.rs:156, reporting specifically 'T.808 A.2.1 initial inherited-header defaults violated' and InvalidJpipHeader for the initial form-1 messages. This is not an arbitrary nonzero exit: the host successfully built the additively registered library test and observed all twelve target calls returning that error. Earlier explicit/in-response inherited-header controls succeeded.

The entry is realistic and admissible. Each failing response is only 7–9 bytes, with one final one-byte Data message at offset zero and a complete three-byte EOR. The selected case's other sequences are at most 38 bytes and four Data messages; each loop creates its own safe cache. Default limits are HARDENED: 2,147,483,648 input bytes, 16,777,216 metadata bytes, and 1,048,576 accepted fragments. At the first failing call the cache contains only one successfully ingested metadata byte (class 8, codestream 0); no precinct databin exists, so accepting class 0, codestream 0, bin 0 would not conflict. The optional priming path adds only a distinct three-byte metadata databin, through ordinary successful ingestion. There is no unsafe/private-state fabrication, deep nesting, capacity pressure, or skipped owning caller. This is a public direct-unit-contract call with complete, ordered, de-framed messages, comparable to the existing 13-byte native ingestion fixture; no production callers are listed. Payload contents are opaque at this byte-ingestion boundary, not a claim that one byte forms a decodable image.

The route is test -> ingest_jpip_response -> new response-local JpipMessageParser -> parse -> parse_jpip_message. The ingestion MIR confirms construction at bb0 and parser invocation/error propagation in the initial loop. New context has previous_class=None and previous_codestream=None. For [48,0,1,112,0,2,0], the leading 0x30 is permitted form 01 with final flag and in-class identifier zero; the remaining Data fields encode offset 0, length 1, payload 'p'. The parser reads the complete Bin-ID, then requires previous_class via ok_or_else and returns InvalidJpipHeader at offset zero, before fragment construction, insertion, or EOR. Consequently a valid standard message is rejected and its payload is not cached. The additional codestream branch likewise only supplies an initial zero for form 2, although the observed failure happens earlier at missing class.

Authority is the rfc_map parent, not the implementation-correlated lifecycle inference that describes rejection. I read its document manifest, finding and applicability, and independently retrieved the full cited A.2.1 section through the parent item. T.808 (12/2022), ISO/IEC 15444-9:2023, explicitly says absent Class with no previous message means class 0, and absent CSn with no previous message means codestream 0; Table A.1 permits indicator 01 with neither field. This applies to this ordered message-sequence consumer, not an HTTP-UDP packet entry with an independent-first-header requirement. The module advertises standard inherited-header support. Thus this exact standards-derived clause is sound for the cited edition, and neither test_error nor revision to match existing rejection is justified. Edition scope remains the supplied 12/2022 document (manifest SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc); this does not establish conformance or differences for the newer catalogued 2026 edition.

[Standards quote, production lines, and provenance](../../evidence.md#dev-772e004106)

</details>
