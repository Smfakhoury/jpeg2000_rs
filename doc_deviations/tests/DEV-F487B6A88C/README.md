# Metadata cache identity incorrectly depends on message codestream index

`DEV-F487B6A88C` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** For metadata messages belonging to one logical target, constructed class-8 identities used for semantic cache lookup shall identify the metadata bin independently of the message codestream index. Retaining a raw CSn for inspection or header inheritance must not prevent equal metadata in-class identifiers from contributing to the same metadata bin.

**Observed deviation:** Metadata cache identity incorrectly depends on message codestream index

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-F487B6A88C --output rfc-results/DEV-F487B6A88C
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/databin-contract-tests jpip::tests::metadata_databin_identity_ignores_codestream --exact --nocapture
```

### Test input and recorded failure

A direct public unit-contract test with no constructor preconditions, supported metadata class 8 and ordinary CSn 0/1, metadata bin 0, one logical target per fresh cache, complete well-framed LBox=12 XML payload, offset 0, final length 12, no auxiliary field, and default resource limits. All state is established through DatabinId::new, JpipFragment::new and DatabinCache::insert, without private-field mutation. The nonzero split cases use consistent complementary fragments and admissible insertion orders. No count or buffer limit is approached.

**Recorded failure** (matched by the replay runner):

```text
DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 } / DatabinId { class_identifier: 8, class: Metadata, codestream: 1, in_class: 0 }, split=0, reverse=false: wrong final length/prefix/bytes/intervals for DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 }: None
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_databin_contract_tests.rs:L202-L203`](files/src/jpip_databin_contract_tests.rs#L202-L203)

```text
    let payload = box32(*b"xml ", b"<a/>");
    assert_eq!(payload.len(), 12);
```

[`src/jpip_databin_contract_tests.rs:L208-L209`](files/src/jpip_databin_contract_tests.rs#L208-L209)

```text
        let first = DatabinId::new(8, stream, bin).unwrap();
        let second = DatabinId::new(8, stream ^ 1, bin).unwrap();
```

[`src/jpip_databin_contract_tests.rs:L116-L127`](files/src/jpip_databin_contract_tests.rs#L116-L127)

```text
            let mut cache = DatabinCache::new(ResourceLimits::default());
            let mut fragments = Vec::new();
            if split != 0 {
                fragments.push(fragment(first, 0, &payload[..split], false));
            }
            fragments.push(fragment(second, split as u64, &payload[split..], true));
            if reverse {
                fragments.reverse();
            }
            for value in fragments {
                cache.insert(value).unwrap();
            }
```

[`src/jpip_databin_contract_tests.rs:L135-L136`](files/src/jpip_databin_contract_tests.rs#L135-L136)

```text
            for identity in [first, second] {
                let status = cache.status(identity);
```

[`src/jpip_databin_contract_tests.rs:L226-L231`](files/src/jpip_databin_contract_tests.rs#L226-L231)

```text
    assert!(
        failures.is_empty(),
        "{} metadata semantic identity failures; first examples: {:#?}",
        failures.len(),
        &failures[..failures.len().min(12)]
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_databin_contract_tests.rs`](files/src/jpip_databin_contract_tests.rs)
- [`validation/databin-contract-tests.patch`](files/validation/databin-contract-tests.patch)
- [`validation/databin-contract-runner.py`](files/validation/databin-contract-runner.py)

**Build commands** (in order):

```sh
bash -eu -c 'patch --batch --forward -p1 < validation/databin-contract-tests.patch; grep -Fq '"'"'include!("jpip_databin_contract_tests.rs");'"'"' src/jpip.rs; cargo test --lib --no-run --locked; python3 validation/databin-contract-runner.py'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing test assertion is the final failures.is_empty() assertion in metadata_databin_identity_ignores_codestream; its first retained counterexample is the split=0, reverse=false case at class 8, bin 0, CSn 0 versus 1. Only the second identity's complete 12-byte XML box is inserted, yet status(first) returns None instead of the completed metadata bin. This violates this exact clause's semantic lookup guarantee, not a requirement to erase raw CSn or make derived Rust equality ignore it. Entry realism: the suite sweeps 132 scalar coordinate pairs, with fresh caches for each of five split points and two orders. Each primary cache accepts at most two payload fragments (12 total bytes) before a one-byte conflict probe; a reference cache contains one 12-byte fragment. Empty-bin checks use final empty fragments. There is no recursive metadata expansion, large bin-index-based allocation, or large payload: u64 boundary identifiers are labels, not counts. Defaults allow 2,147,483,648 input bytes and 1,048,576 fragments. The observed first witness uses only CSn 0/1 and bin 0, so no extreme index or implicit capacity limit is needed. The public direct-unit entry has no requires; class 8 satisfies the classifier contract, fragments use checked public construction, nonfinal empty fragments are skipped, offsets and final lengths are consistent, AUX is None, and each cache represents a single logical target. Production parsing also passes explicit/inherited CSn directly into this constructor, and from_message preserves the resulting identity; the test does not bypass an owning caller invariant. Compiler describe_declaration and CFG show class 8 taking the successful bb0-bb1-bb2-bb3-bb5-bb7 route and retaining the raw CSn. Cache insert/status then call find_databin_index -> same_as -> jpip_databin_identity, whose unconditional codestream equality rejects the metadata alias. At split=0 this makes the completed bin invisible through CSn 0; at split=1 it partitions complementary bytes into two bins, as also observed. The rfc_map parent is authoritative for the stated standards contract: read_rfc_section independently returned T.808 (12/2022), A.3.6.1, pinned SHA 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: 'The codestream index shall be ignored in any message\nwhich has the metadata-bin class identifier.' Its surrounding text explicitly applies to both JPP and JPT metadata from one logical target and allows arbitrary numeric metadata-bin IDs. Existing model agreement cannot override this exception. This assessment is bounded to that cited edition and one-target assumption; the parent notes that the project's newer selected 2026 edition has not been acquired/compared, so no whole-edition conformance conclusion is claimed. The output tail truncates the panic heading and total failure count, but retains the exact failing test, specific semantic-check diagnostics and FAILED verdict; the attribution rests on those concrete checks, not exit 101 alone.

[Standards quote, production lines, and provenance](../../evidence.md#dev-f487b6a88c)

</details>
