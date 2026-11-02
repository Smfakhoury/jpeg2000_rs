# F11 root overlap: ingestion rejects an initial dependent header instead of applying Annex A identifier defaults

`DEV-A47C95C101` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Consume standard binary JPIP message-header forms, including inherited identifiers, using response-local parsing context.

**Observed deviation:** F11 root overlap: ingestion rejects an initial dependent header instead of applying Annex A identifier defaults

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-A47C95C101 --output rfc-results/DEV-A47C95C101
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::contract_ingest_header_forms_response_local -- --exact --nocapture
```

### Test input and recorded failure

Safe DatabinCache::new(ResourceLimits::default()) and ordinary Rust slices/borrows; no private state mutation. Prior accepted responses leave only distinct metadata bins (class 8/codestream 127/bin 0, three bytes; class 8/codestream 0/bin 0, one byte), two fragments and four stored bytes. The new response [0x30,0,1,0] is a complete Annex A form-1 final precinct-bin-0 message with initial class/codestream defaults 0, offset 0 and one-byte body. Its semantic bin is absent, so it cannot conflict with those metadata bins; all resource counts/extents are far below default limits. This API has no transport-packet precondition, and no H.2 unreliable transport is invoked. Response-local reset is respected, not bypassed.

**Recorded failure** (matched by the replay runner):

```text
input=[30, 00, 01, 00], result=Err(MalformedInput { offset: 0, kind: InvalidJpipHeader })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_ingest_contract_tests.rs:L61-L62`](files/src/jpip_ingest_contract_tests.rs#L61-L62)

```text
let result = ingest_jpip_response(input, cache);
    assert!(result.is_ok(), "input={input:02x?}, result={result:?}");
```

[`src/jpip_ingest_contract_tests.rs:L97-L97`](files/src/jpip_ingest_contract_tests.rs#L97)

```text
let mut cache = DatabinCache::new(ResourceLimits::default());
```

[`src/jpip_ingest_contract_tests.rs:L255-L261`](files/src/jpip_ingest_contract_tests.rs#L255-L261)

```text
let previous = DatabinId::new(8, 127, 0).unwrap();
    contract_ingest(
        &mut cache,
        &contract_message(3, previous, 0, b"old", true),
        1,
    );
    contract_ingest(&mut cache, &[0x50, 8, 0, 1, b'n'], 1);
```

[`src/jpip_ingest_contract_tests.rs:L263-L265`](files/src/jpip_ingest_contract_tests.rs#L263-L265)

```text
let initial_dependent = [0x30, 0, 1, 0]; // precinct 0, one empty packet, final
    eprintln!("A.2.1 initial defaults: wire={initial_dependent:02x?}, expected class=0 CSn=0");
    contract_ingest(&mut cache, &initial_dependent, 1);
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_ingest_contract_tests.rs`](files/src/jpip_ingest_contract_tests.rs)
- [`validation/jpip-ingest-contract.patch`](files/validation/jpip-ingest-contract.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/jpip-ingest-contract.patch
```

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip_ingest_contract_tests.rs'"'"').read_text(); assert p.count('"'"'include!("jpip_ingest_contract_tests.rs");'"'"') == 1; assert '"'"'fn ingests_response_with_inherited_context()'"'"' in p; assert '"'"'fn contract_ingest_payload_reassembly()'"'"' in t; assert '"'"'fn contract_ingest_header_forms_response_local()'"'"' in t'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is contract_ingest's result.is_ok() at src/jpip_ingest_contract_tests.rs:62 for input [30, 00, 01, 00], with Err(MalformedInput { offset: 0, kind: InvalidJpipHeader }). It is not a build failure, fixture failure, failed later byte assertion, or inference from exit status. The additive patch only includes a test module inside the existing cfg(test) tests module; host compilation and registration guards succeeded and this named native test actually executed.

Entry realism: this public unit-contract entry has no additional entry requirements. The failing response is four bytes, contains one nonrecursive message with a one-byte body, and uses class 0, codestream 0, bin 0, offset 0, final length 1. ResourceLimits::default() is HARDENED: maximum_input_bytes=2,147,483,648, maximum_metadata_bytes=16,777,216, maximum_jpip_fragments=1,048,576. The safely constructed cache contains only two compatible metadata fragments totaling four bytes before this call; successful acceptance would produce three bins, three accepted fragments, and five stored bytes. The earlier 144 class/bin/stream combinations per encoding allocate fresh caches, each holding only three messages/three bytes, rather than accumulating a large state. Large numeric identifiers in earlier successful sweeps are identifiers, not allocation sizes; the failing identifiers are all zero. There is no excessive stack, nesting, pool, buffer, or count demand. No external caller path is claimed: this is a permitted direct public API unit test, comparable in scale and invocation to the existing ingests_response_with_inherited_context regression. No fabricated parser/header state or private cache mutation is used.

Authority is the rfc_map origin dc9db665e258a9562dc090af, finding 31268b3beec44bbd2e61921d15f62ac2544307539a6a4197c9bf82f8098dd3d5, not a correlated native or Verus oracle. Independently reread the pinned T.808 (12/2022), A.2.1 (manifest SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc): absent initial Class defaults to 0 and absent initial CSn defaults to 0. Table A.1 permits bb=01 with neither field. Thus 0x30 is a complete form-1 Bin-ID, final flag set, in-class identifier 0; the remaining bytes encode offset 0, length 1, body 0x00. Class 0 requires no auxiliary field, and this API retains opaque precinct payload rather than decoding a complete codestream. Also reread H.2: its explicit-first-header requirement is for packets on an unreliable data connection. This harness invokes bare ordered Annex A message ingestion, with no such packet transport; the module explicitly excludes transport implementation. H.2 therefore does not invalidate this entry. The claim is scoped to the acquired 12/2022 edition, not conformance to a newer project-selected edition or official test assets.

Route: the harness's contract_ingest directly calls ingest_jpip_response. The wrapper creates JpipMessageParser::new(cache.limits), whose context has previous_class=None and previous_codestream=None regardless of prior calls sharing that cache. Its first parse receives the full four-byte slice. parse_jpip_message accepts the form and reads the initial in-class field, then requires context.previous_class through ok_or_else instead of using the standard's initial 0. It returns InvalidJpipHeader before reading offset, length, or body; ingestion propagates that error before fragment construction/insertion. Compiler declaration facts and MIR confirm ingestion bb0 initialization, bb3/bb4 parsing, bb5/bb6 Result branching and bb9 error propagation; parser bb31-bb35 is the missing-class Option error route. Even after repairing that first branch, the subsequent codestream branch also lacks the initial form-1 default, so both defaults belong to the same parser root cause. The consequence is rejection of a standard permitted response, not improper inheritance from an earlier response. The implementation-descriptive lifecycle clause that predicts this rejection does not override this standards-derived clause or the module's documented standard-header support. This is neither a wrong test oracle nor a spec overreach on these established premises.

Deduplicate with published F11/the existing parse-header finding and candidate a344db6b768a6a42a25deab2; this wrapper observation does not establish a new root-cause family. Preserve the existing missing-canonical-test-file/list-validation warnings and review labels: they do not erase the supplied original overlay and observed native failure, but no clean-list validation, formal reachability proof, exhaustive conformance, or execution of assertions after line 265 is claimed. No files, expectations, production code, models, or prior records were modified and no additional replay was substituted.

[Standards quote, production lines, and provenance](../../evidence.md#dev-a47c95c101)

</details>
