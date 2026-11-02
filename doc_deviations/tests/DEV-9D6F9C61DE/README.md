# Class-8 metadata lookup incorrectly includes the codestream index in data-bin identity

`DEV-9D6F9C61DE` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Provide complete metadata-bin views through the documented sparse JPIP cache; for standards-facing class-8 identity within one logical target, ignore the message codestream index.

**Observed deviation:** Class-8 metadata lookup incorrectly includes the codestream index in data-bin identity

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-9D6F9C61DE --output rfc-results/DEV-9D6F9C61DE
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 run_databin_view_case.py metadata_identity
```

### Test input and recorded failure

One fresh public cache represents one logical target. Both IDs are constructed successfully by DatabinId::new, with Metadata class and root in-class ID 0. The payload is a length/type-checked JPEG 2000 family free box. The existing fragment helper calls JpipFragment::new with offset 0, final=true, no auxiliary field, and default limits. insert succeeds and the same-ID view equals the expected payload. Only CSn changes in the failing query, which T.808 12/2022 A.3.6.1 requires ignoring for metadata. No private fields, transport states, allocation-sized numeric labels, recursive nesting, or invalid downstream preconditions are introduced; the decisive cache has one fragment and 25 bytes.

**Recorded failure** (matched by the replay runner):

```text
complete root: stored CSn=0, queried CSn=1, expected=[0, 0, 0, 25, 102, 114, 101, 101, 109, 101, 116, 97, 100, 97, 116, 97, 32, 105, 100, 101, 110, 116, 105, 116, 121], actual=Err(MalformedInput { offset: 0, kind: MissingRequiredStructure })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`databin_view_contract_tests.patch:L137-L138`](files/databin_view_contract_tests.patch#L137-L138)

```text
+            let root = DatabinId::new(8, stored_stream, 0).unwrap();
+            let root_alias = DatabinId::new(8, other_stream, 0).unwrap();
```

[`databin_view_contract_tests.patch:L151-L154`](files/databin_view_contract_tests.patch#L151-L154)

```text
+            cache.insert(fragment(root, 0, &expected, true)).unwrap();
+            assert_eq!(cache.databin_view(root).unwrap().as_bytes(), expected);
+            let actual = cache.databin_view(root_alias).map(JpipByteView::into_bytes);
+            if actual != Ok(expected.clone()) {
```

[`databin_view_contract_tests.patch:L238-L240`](files/databin_view_contract_tests.patch#L238-L240)

```text
+        assert!(
+            failures.is_empty(),
+            "insert -> databin_view violates metadata CSn independence:\n{}"
```

[`run_databin_view_case.py:L33-L33`](files/run_databin_view_case.py#L33)

```text
    result = subprocess.run([binary, name, "--exact", "--nocapture", "--test-threads=1"])
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`databin_view_contract_tests.patch`](files/databin_view_contract_tests.patch)
- [`run_databin_view_case.py`](files/run_databin_view_case.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i databin_view_contract_tests.patch
```

```sh
python3 -c 'import hashlib; from pathlib import Path; assert hashlib.sha256(Path('"'"'src/jpip.rs'"'"').read_bytes()).hexdigest() == '"'"'3fad65d700b3556fd5ec8d9400494dedaabf9a43a381e19c8bab1afef3ff8ff5'"'"', '"'"'Source overlay missing or modified'"'"''
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final failures.is_empty() assertion in databin_view_metadata_identity_ignores_codestream; its first recorded mismatch is a complete root metadata bin stored with CSn=0 returning MissingRequiredStructure at offset 0 when queried with CSn=1, instead of its 25 original bytes. This is a valid direct cache-unit entry, not a fabricated private representation or a skipped caller prerequisite. The fixture uses DatabinId::new(8, ..., 0), DatabinCache::new, and checked JpipFragment::new/insert; successful same-ID retrieval establishes receipt of the complete payload before the alias query. The free box has a checked length and type. Each metadata cache holds at most 25 payload bytes, at most three accepted fragments and three implementation bins, with no recursive metadata expansion. Across all five CSn pairs and four non-metadata controls the identity test inserts 103 fragments into separate short-lived caches, not one accumulating cache. Default limits allow 2,147,483,648 input bytes, 16,777,216 metadata bytes and 1,048,576 fragments. The decisive CSn pair 0/1 is small even relative to maximum_codestreams=64. Later large numeric labels size no allocation and are not necessary to establish this deviation.

The clause has rfc_map origin, not infer origin. I read its parent finding, pinned document manifest and T.808 (12/2022) A.3.6.1 through read_rfc_section using originating item caf66b63dc43823a29f23597. That section expressly says 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' It applies to metadata from one logical target, requires root bin ID 0, and expressly permits an empty root. The fixture uses one target per cache. Thus the alias oracle implements the cited metadata exception; no cross-target merging is claimed. The exact-codestream matching in the stored lookup/status contracts and correlated implementation model describes the same implementation behavior and does not override this normative exception. This conclusion is limited to acquired T.808 12/2022 (SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc), not the unacquired 06/2026 edition or general JPIP conformance.

The replay runner selects and executes the registered native test exactly; host build and source-overlay checks succeeded. The assessed route is checked fragment construction -> insert -> databin_view(root_alias) -> find_databin_index -> DatabinId::same_as -> jpip_databin_identity. The helper unconditionally compares codestreams, so the sole stored class-8/bin-0 entry does not match CSn=1. databin_view's MIR bb0 -> bb1 -> bb2 -> bb3 -> bb6 -> bb27 propagates the initial missing-index error, before any completeness or byte-copy branch. This rejects an already complete semantic metadata bin. The separately reported mixed-CSn fragments likewise remain split into incomplete implementation bins. Real ingestion does not supply a missing canonicalization premise: parse_jpip_message retains explicit/inherited CSn in DatabinId, JpipFragment::from_message preserves header.databin, and ingest_jpip_response directly inserts that fragment; metadata consumers construct class-8 IDs and use these same lookup paths. The replay is a direct public-unit-contract reproduction, not a claimed end-to-end wire/session reproduction. Host output is truncated to eight mismatch details by the harness, but the first complete-root mismatch includes the operation, both indices, expected bytes and actual error and is sufficient independently of the exit code.

[Standards quote, production lines, and provenance](../../evidence.md#dev-9d6f9c61de)

</details>
