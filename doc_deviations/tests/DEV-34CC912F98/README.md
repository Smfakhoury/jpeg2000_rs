# Fresh JPIP history rejects the standard initial inherited header instead of selecting class 0 and codestream 0

`DEV-34CC912F98` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Create a fresh, independent message-history context for the supported standard inherited and explicit JPIP message-header forms; absence of previous messages must be distinguishable from a previously selected identity.

**Observed deviation:** Fresh JPIP history rejects the standard initial inherited header instead of selecting class 0 and codestream 0

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-34CC912F98 --output rfc-results/DEV-34CC912F98
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::fresh_header_context_preserves_absence_and_independent_standard_history -- --exact
```

### Test input and recorded failure

Public constructor only; no private-state writes, injected faults, or fabricated history. Default ResourceLimits; three input bytes and zero body/offset are within capacity. Form 01, bin 0, canonical single-byte zero Msg-Offset/Msg-Length, no final flag, and no Aux for initial even class 0 satisfy T.808 (12/2022) A.2.1/A.2.2 in a fresh ordered JPP sequence with codestream 0 available. A direct header-parser unit test need not run cache insertion or image decoding. Absence of previous class/codestream is the legitimate initial state for which the standard defines both defaults.

**Recorded failure** (matched by the replay runner):

```text
fresh inherited header must select standard initial identity (0,0): MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_header_context_contract_tests.rs:L86-L90`](files/src/jpip_header_context_contract_tests.rs#L86-L90)

```text
    let mut first_inherited = JpipHeaderContext::new();
    // A.2.1 supplies class=0 and CSn=0 when there is no previous message.
    // No codestream body/marker validation is required for header parsing.
    check(&mut first_inherited, &limits, 1, 0, 0)
        .expect("fresh inherited header must select standard initial identity (0,0)");
```

[`src/jpip_header_context_contract_tests.rs:L9-L17`](files/src/jpip_header_context_contract_tests.rs#L9-L17)

```text
        let mut bytes = vec![form << 5];
        if form >= 2 {
            bytes.push(class as u8);
        }
        if form == 3 {
            bytes.push(codestream as u8);
        }
        bytes.extend([0, 0]);
        bytes
```

[`src/jpip_header_context_contract_tests.rs:L33-L33`](files/src/jpip_header_context_contract_tests.rs#L33)

```text
        let (message, consumed) = parse_jpip_message(&bytes, context, limits)?;
```

[`src/jpip_header_context_contract_tests.rs:L57-L58`](files/src/jpip_header_context_contract_tests.rs#L57-L58)

```text
    let limits = ResourceLimits::default();
    assert_eq!(limits.maximum_codestreams(), 64);
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_header_context_contract_tests.rs`](files/src/jpip_header_context_contract_tests.rs)
- [`tests/jpip-context-registration.patch`](files/tests/jpip-context-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i tests/jpip-context-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip_header_context_contract_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jpip_header_context_contract_tests.rs");'"'"') == 1; assert '"'"'fn fresh_header_context_preserves_absence_and_independent_standard_history()'"'"' in t; assert '"'"'fn parses_all_three_standard_header_forms_and_auxiliary()'"'"' in s; assert '"'"'fn rejects_malformed_prefixes_and_vbas()'"'"' in s'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the final expect("fresh inherited header must select standard initial identity (0,0)"): parsing [0x20,0x00,0x00] returns MalformedInput { offset: 0, kind: InvalidJpipHeader }. Earlier freshness, independence, explicit-form and established-history checks completed. Entry realism: the generated sweep makes 1,030 sequential parsing calls, each with only 3–5 header bytes, zero payload, zero offset, bin 0, no nesting and a constant number of small value contexts; it does not retain an aggregate buffer/cache or consume a fragment pool. Codestream indices are 0–63, within the default 64-codestream capacity, and inputs are far below the 2,147,483,648-byte default input limit. The failing call uses only codestream 0. Real callers JpipMessageParser::new and ingest_jpip_response likewise start with JpipHeaderContext::new and feed ordinary message bytes into this parser. This is a valid direct unit entry, not private-state fabrication or bypass of a required owning caller.

Read clause_origin: this is an rfc_map-derived clause, not merely the independent inferred constructor None/None postcondition. Independently read the pinned T.808 (12/2022), SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, A.2.1 and A.2.2 through the originating map. Table A.1 permits indicator 01 with neither Class nor CSn. A.2.1 explicitly assigns class 0 and codestream 0 when those fields are absent and no previous message exists. Offset and length are non-negative, so zero is admissible. A.2.2 identifies class 0 as a JPP precinct message and requires Aux only for odd classes. Assume an ordered fresh JPP message sequence with codestream 0 available; header parsing does not require the fixture to supply codestream marker data. The implementation documents support for the standard inherited form without excluding the initial message. Thus the failed check exercises the supported-standard-forms meaning of c4f741cfbb88dfa62062114b; it does not demand changing the valid None/None representation. This conclusion is confined to the cited 2022 edition, not reconciliation with newer editions.

Compiler evidence for new shows a single block returning None/None with no callees. Parser MIR and source then trace the actual failure: nonempty input passes the size check and is not EOR; form=(0x20>>5)&3=1 is valid; read_vbas with mask 0x0f consumes the first byte as in-class 0; jpip_header_has_explicit_class(1) is false (MIR bb23/bb25), so bb31 copies previous_class=None and calls ok_or_else; bb32/bb33 select the error and bb35 propagates it to return bb94. This is the precise rejection seen by the host, before Msg-Offset/Msg-Length parsing or context updates. The absent-CSn branch also lacks the required form-1 initial default, but that is a secondary source observation, not a separately reproduced failure. The constructor itself correctly distinguishes absent history from Some(0); its parser consumer incorrectly treats that absence as forbidden instead of applying the standard defaults. A correlated native regression rejecting a fresh inherited header cannot override the cited standard. No production changes or additional replay are needed to attribute this already host-reproduced failure.

[Standards quote, production lines, and provenance](../../evidence.md#dev-34cc912f98)

</details>
