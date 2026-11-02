# Initial dependent JPIP header is rejected instead of applying class-zero default

`DEV-CA85C0D7B5` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** The JPIP header context should support the documented inherited-class binary message-header form.

**Observed deviation:** Initial dependent JPIP header is rejected instead of applying class-zero default

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-CA85C0D7B5 --output rfc-results/DEV-CA85C0D7B5
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture
```

### Test input and recorded failure

Fresh safe public new() context has no predecessor, exactly the initial-message condition specified by T.808 A.2.1. The input is a complete three-byte form-01 header: Bin-ID 1, omitted Class and CSn defaulting to zero, offset zero, length zero, no Aux and no body. It is an admissible direct parser/context unit-contract entry, with ordered operations and no private-state writes, skipped state-owning caller, size violation, or payload-decoding claim.

**Recorded failure** (matched by the replay runner):

```text
valid dependent/independent header must parse: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`inherited_class_registration.patch:L8-L8`](files/inherited_class_registration.patch#L8)

```text
+        check_inherited_class_contract();
```

[`src/jpip_inherited_class_contract.rs:L119-L124`](files/src/jpip_inherited_class_contract.rs#L119-L124)

```text
    for mut context in [JpipHeaderContext::new(), JpipHeaderContext::default()] {
        assert_eq!(context.previous_class(), None);
        assert_eq!(context.previous_codestream(), None);
        let initial = message(1, 0, 0, 0, &[]);
        assert_eq!(initial, [0x21, 0, 0]);
        accept(&initial, &mut context, 0, 0, 0, &[]);
```

[`src/jpip_inherited_class_contract.rs:L46-L49`](files/src/jpip_inherited_class_contract.rs#L46-L49)

```text
        let result = parse_jpip_message(input, context, &ResourceLimits::default());
        let (parsed, consumed) = result.unwrap_or_else(|error| {
            panic!("valid dependent/independent header must parse: {error:?}")
        });
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`inherited_class_registration.patch`](files/inherited_class_registration.patch)
- [`src/jpip_inherited_class_contract.rs`](files/src/jpip_inherited_class_contract.rs)

**Build commands** (in order):

```sh
patch -p1 --batch --forward -i inherited_class_registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); assert s.count('"'"'check_inherited_class_contract();'"'"') == 1; assert s.count('"'"'include!("jpip_inherited_class_contract.rs");'"'"') == 1; h=Path('"'"'src/jpip_inherited_class_contract.rs'"'"').read_text(); assert '"'"'assert_eq!(initial, [0x21, 0, 0]);'"'"' in h; assert '"'"'fn check_inherited_class_contract()'"'"' in h'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing check is accept()'s requirement that parse_jpip_message succeed on [0x21, 0x00, 0x00] with a fresh JpipHeaderContext::new(); the host reports InvalidJpipHeader. This violates e747fe080bc71ebe1d1c602a's documented inherited-header support, interpreted using its rfc_map parent's explicitly applicable T.808 (12/2022), A.2.1, rather than the separate inferred observer-history clause. I read the parent provenance/document manifest and retrieved A.2.1 in full through the origin item. Table A.1 permits form 01 with neither Class nor CSn; the text explicitly defaults both identifiers to zero when there is no previous message. The test assumes one reliably ordered stream per context, which matches that section's applicability. No newer-edition conformance is claimed.

Entry realism: the seven-class, 24-iteration sweep runs sequentially with five positive calls and two negative calls per iteration (840 positive and 336 negative calls total), no recursion or accumulated reassembly state, bodies at most three bytes, messages at most 13 bytes, and offset+length at most 65,538. ResourceLimits::default() uses HARDENED.maximum_input_bytes = 2,147,483,648. Large codestream values in the sweep are syntactic indices, not requests to allocate that many codestreams; maximum_codestreams is a count/reassembly constraint, not a header-parser index bound. The actual failing entry is just three bytes, offset and length zero, codestream zero, and no allocation/pool/depth pressure. Real JpipMessageParser::new initializes this same public context and parse delegates directly to parse_jpip_message with it. Existing native tests exercise the same API with small explicit/inherited headers but do not cover the initial dependent-header default.

The replay patch only registers the helper inside the existing test module. The helper uses public constructors and parser calls, not fabricated private state. For 0x21 the continuation and final bits are clear, form=1, and in-class identifier=1; the remaining two single-byte VBAS values encode offset=0 and length=0, with no Aux for class 0 and exactly zero body bytes. Header interpretation does not require the fixture to decode a complete image or payload. MIR confirms that input/form validation and read_vbas succeed, then bb23/bb25 select the omitted-class branch, bb31 applies ok_or_else to previous_class=None, and bb32/bb33/bb35 propagate the error to the return bb94. This occurs before codestream selection and context installation. Thus the missing class-zero default specifically produces the observed rejection. The nearby codestream branch also lacks the initial form-1 default, but that later branch was not reached by this failing call and is not independently reproduced here.

This is a parser/context composition deviation under the documented clause, not an incorrect previous_class() accessor: describe_declaration shows that observer simply returns Option<u64> without callees, and the post-parse accessor assertions are not reached for this input. The output exposes both the failing input/prestate and the exact failed success check, so this is not attribution from exit status alone. The first new() case failed; default() and subsequent initial-default inheritance cases were not executed and remain coverage gaps. No production changes were made.

[Standards quote, production lines, and provenance](../../evidence.md#dev-ca85c0d7b5)

</details>
