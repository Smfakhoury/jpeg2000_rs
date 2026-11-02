# JPIP parser rejects legal initial inherited-form header instead of defaulting Class and CSn to zero

`DEV-E9BA9C12DC` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.2

**Required behavior:** The supported binary-header forms preserve inspectable message identifiers while deterministic reassembly recognizes extended and ordinary precinct messages as addressing the same logical precinct data-bin.

**Observed deviation:** JPIP parser rejects legal initial inherited-form header instead of defaulting Class and CSn to zero

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-E9BA9C12DC --output rfc-results/DEV-E9BA9C12DC
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture
```

### Test input and recorded failure

Public JpipMessageParser::new(ResourceLimits::default()) provides a valid fresh context, with neither previous class nor previous codestream. No private mutation or unsafe fixture is used. T.808 (12/2022) A.2.1 expressly permits first-message bb=01 with both omitted fields defaulting to zero. The four-byte message carries ordinary precinct bin 0, byte offset 0, body length 1, no Aux or final flag, and one zero payload byte. It is far below configured capacities, requires no predecessor, and matches the fresh-parser pattern used by ingest_jpip_response. The broader supported-forms clause applies even though the failed parse prevents reaching the getter.

**Recorded failure** (matched by the replay runner):

```text
parse -> header.databin -> fragment: form=1 class=0 stream=0 bin=0 offset=0 last=false wire=[20, 00, 01, 00]: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`jpip_databin_contract.patch:L280-L281`](files/jpip_databin_contract.patch#L280-L281)

```text
+        let mut initial = JpipMessageParser::new(limits);
+        contract_parse_precinct(&mut initial, 1, (0, 0, 0), (0, false));
```

[`jpip_databin_contract.patch:L75-L76`](files/jpip_databin_contract.patch#L75-L76)

```text
+        let wire = contract_precinct_wire(form, identity, range);
+        let (message, consumed) = parser.parse(&wire).unwrap_or_else(|error| {
```

[`jpip_databin_contract.patch:L7-L7`](files/jpip_databin_contract.patch#L7)

```text
+        databin_contract_sweep();
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`jpip_databin_contract.patch`](files/jpip_databin_contract.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jpip_databin_contract.patch
```

```sh
python3 -c 'import hashlib; assert hashlib.sha256(open('"'"'src/jpip.rs'"'"','"'"'rb'"'"').read()).hexdigest() == '"'"'a997101e1b6d41f15ef6f9832cd4564f02a49fb6b42a380d01d9cab3d38c11f7'"'"', '"'"'replay source mismatch: test registration/assertions missing'"'"''
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the helper's expectation that parsing succeeds for form=1 class=0 stream=0 bin=0 offset=0 last=false, wire=[20, 00, 01, 00]. It is not a failed databin identity comparison or a sanitizer/setup failure. The preceding 25-identity sweep completes before this final fresh-parser case. Resource realism was checked first: the failing message is four bytes with a one-byte body and end offset 1; the sweep uses sequential, nonrecursive fixtures, at most eleven bytes per encoded message, and each cache holds three bins/five bytes/seven accepted fragments. Default limits allow 2,147,483,648 input bytes and 1,048,576 JPIP fragments. Large sampled identifiers are scalar identifiers, not requests to allocate that many codestreams or precincts. The actual failing identity is entirely zero. Production ingest_jpip_response likewise creates a fresh JpipMessageParser and immediately parses the first message; the test does not fabricate private state or bypass a required initialization step.

Read clause_origin: this is rfc_map, not infer. Independently read its pinned T.808 (12/2022), SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, section A.2.1 using the originating mapping item. Table A.1 permits bb=01, omitting Class and CSn. It explicitly states: 'If the Class VBAS is not present and there is no previous message, the message class identifier is 0.' It also states: 'If CSn VBAS is not present and there is no previous message, the codestream index is 0.' Those provisions apply to this first ordinary precinct message: Bin-ID=0, offset=0, length=1, no Aux, body=[0]. The documented supported inherited binary-header form therefore includes this entry. No claim about newer editions is needed. The parent's separate ordinary/extended logical-bin equivalence is not what fails here; those reassembly checks completed. Reclassifying the clause to require a predecessor would contradict this edition's explicit initial defaults, not repair a faulty oracle.

Exact route: existing libtest -> added databin_contract_sweep -> final JpipMessageParser::new(limits) -> contract_parse_precinct(form=1,(0,0,0),(0,false)) -> parser.parse(&wire) -> parse_jpip_message. Compiler declaration and MIR CFG confirm form 1 passes the form check, read_vbas extracts bin 0, then absence of explicit Class takes bb25 -> bb31. previous_class is None from the public constructor; ok_or_else yields InvalidJpipHeader and bb32 -> bb33 -> bb35 returns Err. Source src/jpip.rs:264-267 rejects this admissible first message rather than using zero. The codestream branch at 273-276 also lacks the required form-1 initial default, but the replay fails earlier at Class; that second branch is not claimed as separately reproduced. The consequence is rejection of a standard permitted supported header before creation of JpipMessage::Data, preventing identifier inspection/reassembly on this route. The subject getter's MIR is only a field copy and is not reached in this failing call; this diagnosis concerns the exact clause's supported-header-form guarantee, not an alleged incorrect getter result. The host output and replayed harness expose the specific failing operation sufficiently; no new local execution is being substituted for host replay.

[Standards quote, production lines, and provenance](../../evidence.md#dev-e9ba9c12dc)

</details>
