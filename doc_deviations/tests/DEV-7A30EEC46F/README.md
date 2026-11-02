# Reserved JPIP class identifiers 3 and 7 admitted as semantic header classes

`DEV-7A30EEC46F` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.2

**Required behavior:** For the supported standard binary-message profile, expose semantic header classes only for assigned message identifiers; reserved signaling is not to be admitted as a supported extension.

**Observed deviation:** Reserved JPIP class identifiers 3 and 7 admitted as semantic header classes

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-7A30EEC46F --output rfc-results/DEV-7A30EEC46F
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 jpip_class_contract_suite.py jpip::tests::standard_assigned_classes_exclude_reserved_signaling
```

### Test input and recorded failure

Public scalar-only checked-constructor inputs, initially codestream=0 and in_class=0, with no private-state edits, payloads, parser context or downstream plan invariants. Reserved codes are legitimate negative inputs to the checked constructor, whose job is admission. Only a bounded 54,340 sequential probes and small vectors are used; u64 identity extrema do not imply huge buffers. The runner's sole existing-source change is an asserted test-only include insertion and its host build succeeded.

**Recorded failure** (matched by the replay runner):

```text
DatabinId::new(3, 0, 0) -> Ok(DatabinId { class_identifier: 3, class: TileHeader, codestream: 0, in_class: 0 }); Table A.2 semantic class expected None
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_class_contract_cases.rs:L38-L39`](files/src/jpip_class_contract_cases.rs#L38-L39)

```text
let result = DatabinId::new(code, codestream, in_class);
            match (standard_class_oracle(code), result) {
```

[`src/jpip_class_contract_cases.rs:L5-L9`](files/src/jpip_class_contract_cases.rs#L5-L9)

```text
2 => Some(DatabinClass::TileHeader),
        4 | 5 => Some(DatabinClass::TileData),
        6 => Some(DatabinClass::MainHeader),
        8 => Some(DatabinClass::Metadata),
        _ => None,
```

[`src/jpip_class_contract_cases.rs:L58-L62`](files/src/jpip_class_contract_cases.rs#L58-L62)

```text
assert!(
        violations.is_empty(),
        "reserved signaling exposed as supported, or assigned class rejected:\n{}",
        violations.join("\n")
    );
```

[`jpip_class_contract_suite.py:L24-L24`](files/jpip_class_contract_suite.py#L24)

```text
assert edited.replace(registration, "", 1) == original
```

[`jpip_class_contract_suite.py:L59-L59`](files/jpip_class_contract_suite.py#L59)

```text
os.execv(binary, [binary, "--exact", sys.argv[1], "--nocapture"])
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_class_contract_cases.rs`](files/src/jpip_class_contract_cases.rs)
- [`jpip_class_contract_suite.py`](files/jpip_class_contract_suite.py)

**Build commands** (in order):

```sh
python3 jpip_class_contract_suite.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is violations.is_empty() at src/jpip_class_contract_cases.rs:58, not a setup failure or an arbitrary nonzero exit. The first recorded mismatch is DatabinId::new(3, 0, 0) returning Ok with class TileHeader when the independent Table A.2 oracle assigns no class; code 7 similarly returns MainHeader. This violates clause cca7e9b1da20c54a63cf8dfc's assigned-only supported semantic header guarantee.

Entry realism: this case performs 209 scalar identity pairs times 260 class codes = 54,340 sequential constructor probes. It retains only 209 pairs, 260 codes and at most 12 diagnostic strings, with no payload allocation, recursion, codestream parsing, cache or resource-pool use. Large u64 identities are labels, not allocation sizes. The first counterexamples already use codestream=0/in_class=0; these are ordinary small identities comparable to production callers' main-header construction (6, codestream, 0). DatabinId::new is a public checked constructor with scalar u64 arguments and no caller-side assigned-code requirement; supplying a reserved identifier to test its admission policy is admissible, not fabricating an invalid DatabinId. The constructor itself establishes all private fields. The target unit contract has no entry requires, and no owning parser/codestream-plan invariant is bypassed.

Route: the submitted Python runner inserts only an include in the existing cfg(test) module, builds the native library tests and executes this exact libtest. The test calls DatabinId::new for each code. from_identifier's validity helper accepts <=8, then explicitly maps 3 to TileHeader and 7 to MainHeader. Constructor MIR proceeds bb0->bb1->bb2->bb3->bb5->bb7, storing that class and returning Ok. The test's (None, Ok(...)) mismatch arm collects it and the final emptiness assertion fails. On these failing branches the harness does NOT separately call class(); the observed operation is successful admission and the exposed Debug class field. The class accessor's MIR independently shows a single bb0 copying that same stored class, without any further check. Thus this is a reproduced admission/classification deviation, not a claim that a failing accessor call or an end-to-end wire decode was replayed.

Authority: clause_origin is rfc_map, not infer. Reading the parent item's original T.808 (12/2022) A.2.2 at pinned sha256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc confirms Table A.2 assigns only 0,1,2,4,5,6,8 and reserves all other values. Header code 2 is JPP-only and code 6 is JPP/JPT; neither 3 nor 7 is assigned. SUPPORTED.md says reserved signaling is not a supported extension, and the module describes standard binary headers. The odd-identifier Aux rule is parsing syntax for skipping unrecognized messages, not a semantic assignment of every odd neighbor. This test does not impose a wire-level skip-versus-reject policy or exact error variant; DatabinId has no unknown/reserved class representation. Existing implementation/model agreement cannot override that standard and documented policy. The project lists a 2026 Part 9 catalog target and explicitly cautions about publication/edition traceability; this finding is limited to the acquired 2022 edition, as the parent finding assumes, and is not a claim about unavailable 2026 text. No production changes or additional replay are needed to attribute this already host-observed failure.

[Standards quote, production lines, and provenance](../../evidence.md#dev-7a30eec46f)

</details>
