# Fresh JPIP form-01 messages reject standard default class and codestream identifiers

`DEV-30865B7D2D` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Accept the standard binary message-header forms with inherited, explicit-class, or explicit-class-and-codestream identifiers, subject to the bounded parser profile.

**Observed deviation:** Fresh JPIP form-01 messages reject standard default class and codestream identifiers

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-30865B7D2D --output rfc-results/DEV-30865B7D2D
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/run_jpip_contract.py run forms
```

### Test input and recorded failure

The context is constructed by JpipHeaderContext::new(), not fabricated or privately mutated; there is deliberately no previous message. ResourceLimits::default().with_maximum_input_bytes(65536).unwrap() is valid. The five-byte message has form 01, class/CSn omitted as A.2.1 permits with zero defaults, in-class identifier 0, offset 0, body length 2, complete body [0x12,0x34], last=false and no Aux for class 0. Its complete canonical one-byte fields and extent 2 satisfy the bounded parser and read_vbas contracts. All 20 observed variants remain 5-7 bytes, so no platform capacity issue or owning-caller requirement is skipped. This is direct header parsing, not an assertion that the body is a fully decodable JPEG2000 image.

**Recorded failure** (matched by the replay runner):

```text
A.2.1 first form-01 bytes=[20, 00, 02, 12, 34]: expected class=0, CSn=0; got Err(MalformedInput { offset: 0, kind: InvalidJpipHeader })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_contract_tests.rs:L184-L186`](files/src/jpip_contract_tests.rs#L184-L186)

```text
    let limits = ResourceLimits::default()
        .with_maximum_input_bytes(65536)
        .unwrap();
```

[`src/jpip_contract_tests.rs:L309-L317`](files/src/jpip_contract_tests.rs#L309-L317)

```text
    let data = ContractData {
        class: 0,
        stream: 0,
        bin: 0,
        offset: 0,
        last: false,
        auxiliary: None,
        body: Vec::new(),
    };
```

[`src/jpip_contract_tests.rs:L354-L361`](files/src/jpip_contract_tests.rs#L354-L361)

```text
            let mut first = data.clone();
            first.bin = bin;
            first.last = last;
            first.body = vec![0x12, 0x34];
            let (bytes, header_length) = contract_data_bytes(&first, 1);
            assert!(bytes.len() as u64 <= limits.maximum_input_bytes());
            let mut fresh = JpipHeaderContext::new();
            match parse_jpip_message(&bytes, &mut fresh, &limits) {
```

[`src/jpip_contract_tests.rs:L383-L383`](files/src/jpip_contract_tests.rs#L383)

```text
    assert!(rejected.is_empty(), "A.2.1 first-header defaults rejected: {rejected:?}");
```

[`validation/run_jpip_contract.py:L64-L66`](files/validation/run_jpip_contract.py#L64-L66)

```text
    return subprocess.run(
        [binary, name, "--exact", "--nocapture"], cwd=ROOT
    ).returncode
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_contract_tests.rs`](files/src/jpip_contract_tests.rs)
- [`validation/run_jpip_contract.py`](files/validation/run_jpip_contract.py)

**Build commands** (in order):

```sh
python3 validation/run_jpip_contract.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is src/jpip_contract_tests.rs:383, assert!(rejected.is_empty(), ...), after all preceding header-form sweeps and negative checks succeeded. Host case 07be402c570ea0a3c7248a7d specifically reports the five-byte input [20, 00, 02, 12, 34] returning InvalidJpipHeader rather than a data message with class=0 and CSn=0. This violates clause a3f0825227645d309edff279's acceptance of standard binary header forms, not an assertion about image decoding or full-response conformance.

Entry realism: the minimal call supplies only five bytes, a two-byte body, offset zero, in-class identifier zero, and no recursion, cache allocation or nested structure. The 20 fresh-context variants use 5-7 bytes each, in-class identifiers at most 16384, and the same two-byte body; input size and extent 2 are far below the configured 65536-byte ceiling. The preceding 280 sequential fixtures use bodies at most 256 bytes and offsets below 32000 (boundary offsets at most 16384); these too stay below that ceiling. Deliberately malformed/overflowing fixtures assert rejection and passed, so they are not the failing admission. No implicit stack, buffer or pool exhaustion explains this failure. The actual JpipMessageParser::new creates the same fresh context and parse delegates directly to parse_jpip_message; ingest_jpip_response likewise creates a fresh parser before reading the first message. This is also an available direct public unit-contract entry with no additional requires, so no owning caller or image reconstruction state is being bypassed.

Authority: clause_origin is rfc_map. I independently read its pinned T.808 (12/2022), carrying ISO/IEC 15444-9:2023, A.2.1 using the originating mapping item. Table A.1 defines 01 as no Class or CSn; the prose explicitly defaults missing Class and missing CSn to zero when there is no previous message. The fixture is a general ordered Annex-A JPP header, not a K.1 HTTP-UDP chunk. The public bounded parser does not impose that transport-specific first-message requirement. Class zero is supported, no Aux is required, the VBAS encodings are canonical and complete, and opaque body inspection does not require decoding a precinct. Thus neither a harness precondition error nor revising the clause to match the implementation is justified. This diagnosis is bounded to the cited 12/2022 edition and documented parser promise; the parent finding does not establish comparison with the separately cataloged 06/2026 edition, and I make no newer-edition conformance claim.

Route: Python's forms command selects the compiled exact native test jpip::tests::contract_standard_header_forms. Its final loop constructs form=1 using contract_data_bytes, creates JpipHeaderContext::new(), and directly invokes parse_jpip_message. Compiler declaration/CFG and source agree: nonempty/size/EOR checks pass; 0x20 gives form=1, jpip_header_form_valid is true, and read_vbas at position zero with mask 0x0f succeeds with in_class=0. jpip_header_has_explicit_class(1) is false, so MIR bb25 -> bb31 reads previous_class=None; ok_or_else produces InvalidJpipHeader and bb35 returns the error, before any body parsing or context update. All callee input/reference requirements on this route are met. The codestream fallback also defaults only form 2 and would reject a fresh form 1 even if the earlier class rejection were repaired alone. Consequence: admissible initial dependent headers are rejected instead of receiving standard zero identifiers. Existing native/model expectations of initial rejection are correlated implementation facts, not authority overriding A.2.1. The authoritative failure is the explicit unexpected parse result and rejected-list assertion, not merely exit 101; the separate ancillary report_deviation build error is not used as reproduction evidence.

[Standards quote, production lines, and provenance](../../evidence.md#dev-30865b7d2d)

</details>
