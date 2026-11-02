# Metadata cache identity incorrectly includes CSn, producing spurious semantic dependencies

`DEV-A64A9EFAD1` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Expose outstanding dependencies of the documented metadata-placeholder assembly facility so incomplete data is distinguishable from successful reconstruction.

**Observed deviation:** Metadata cache identity incorrectly includes CSn, producing spurious semantic dependencies

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-A64A9EFAD1 --output rfc-results/DEV-A64A9EFAD1
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/semantic-needs-tests --exact jpip::tests::semantic_metadata_needs_ignore_metadata_codestream_index --nocapture
```

### Test input and recorded failure

One cache contains exactly one logical JP2 target; there is no cross-target mixing. Root has signature, JP2 ftyp, one ordinary 28-byte flags=1 phld replacing jp2h, and literal jp2c bytes. Child contains the original jp2h contents with ihdr and colr. Placeholder OrigID is the matching eight-byte child ID and original length/type are preserved. Both fragments use offset zero and final=true via JpipFragment::new and successful cache.insert. Root=150 bytes, child=37 bytes, output=167 bytes; depth one and two fragments satisfy default limits. CSn values 0 and 1 are valid constructor inputs and must be ignored for class-8 identity under the cited edition. u64::MAX is an ID, not an allocation request. The test obtains the assembly from its real public producer; no private state or production behavior is changed.

**Recorded failure** (matched by the replay runner):

```text
bin=18446744073709551615 root_csn=0 child_csn=1 query_csn=0 complete=true: actual=(Partial, [(MetadataBin, 18446744073709551615)], None) expected=(Available, [],
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`semantic-needs.patch:L189-L189`](files/semantic-needs.patch#L189)

```text
+                [(0, 0, 0), (7, 7, 7), (0, 1, 0), (0, 0, 1), (3, 5, 7)]
```

[`semantic-needs.patch:L200-L200`](files/semantic-needs.patch#L200)

```text
+                        DatabinId::new(8, root_csn, 0).unwrap(),
```

[`semantic-needs.patch:L210-L210`](files/semantic-needs.patch#L210)

```text
+                                DatabinId::new(8, child_csn, bin).unwrap(),
```

[`semantic-needs.patch:L222-L227`](files/semantic-needs.patch#L222-L227)

```text
+                    let report = cache.assemble_semantic_container(query_csn).unwrap();
+                    let needs: Vec<_> = report
+                        .semantic_needs()
+                        .iter()
+                        .map(|need| (need.kind(), need.identifier()))
+                        .collect();
```

[`semantic-needs.patch:L257-L261`](files/semantic-needs.patch#L257-L261)

```text
+        assert!(
+            violations.is_empty(),
+            "metadata CSn must not create spurious dependencies:\n{}",
+            violations.join("\n")
+        );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`semantic-needs.patch`](files/semantic-needs.patch)

**Build commands** (in order):

```sh
bash -c 'patch --batch --forward -p1 < semantic-needs.patch && grep -Fq '"'"'fn semantic_metadata_needs_ignore_metadata_codestream_index()'"'"' src/jpip.rs && grep -Fq '"'"'exact outstanding dependencies'"'"' src/jpip.rs && grep -Fq '"'"'for split in splits'"'"' src/jpip.rs && cargo test --locked --lib --no-run && cp "$(find target/debug/deps -maxdepth 1 -type f -executable -name '"'"'jpeg2000_rs-*'"'"')" target/semantic-needs-tests'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the test's final assert!(violations.is_empty()), which collects mismatches between reported dependencies/status/container and the standard-derived expectation; it is not a constructor failure, timeout, or arbitrary exit. The retained host output specifically shows complete root/child metadata yielding Partial with a MetadataBin need instead of Available with no needs. This violates clause 14ba92bb9ab4b9fc4b9ed06e in its rfc_map context: reported outstanding dependencies must describe genuinely incomplete target metadata, not an artificial CSn partition. I read the parent finding and the pinned T.808 (12/2022) A.3.6.1 through read_rfc_section using origin item 4bbbf5c259181f66f15ae648. It explicitly requires ignoring CSn on metadata messages, allows arbitrary numeric metadata-bin IDs, and assigns root ID 0. No claim about the unacquired 2026 edition is needed.

Entry realism was checked before tracing: 25 independent fresh caches, at most two final offset-zero fragments per cache, 150 root bytes plus 37 child bytes, 167 reconstructed bytes, one placeholder descent, and a 1x1 single-component codestream. These are far below default HARDENED limits (2 GiB input, 16 MiB metadata, depth 32, 1,048,576 fragments). The large bin IDs occupy eight bytes and are not counts, indices into allocated arrays, or requested payload lengths. The standard permits arbitrary numeric IDs. The existing semantic_metadata_assembly_places_nested_boxes_and_codestreams caller uses the same safe fragment/insert/assembly/report route and comparable fixtures; it restricts CSn to zero, which masks rather than legitimizes the identity error. This direct unit-contract test needs no production reachability proof.

For the retained complete-child witness, insert stores root (class=8, CSn=0, ID=0) and child (class=8, CSn=1, ID=u64::MAX) for one logical target. assemble_semantic_container(0) finds the root, parses the ordinary flags=1 jp2h placeholder, and expands the child using metadata_codestream=0. is_complete -> status -> find_databin_index -> DatabinId::same_as -> jpip_databin_identity rejects the complete child solely because 0 != 1. expand_metadata_bin consequently pushes MetadataBin(u64::MAX), returns false, and assembly returns Partial/None. semantic_needs faithfully exposes that erroneous producer report. describe_declaration and the getter CFG confirm the accessor only borrows self.needs and dereferences the Vec; the deviation is in supporting identity/assembly logic, not slice mechanics. Root-CSn mismatch rows independently show a spurious MetadataBin(0). All setup uses public safe constructors and successful insert/assembly results, not private field fabrication or a bypass of the owning producer.

The host tail truncates the beginning of the aggregate panic, so I do not claim its earliest mismatching sweep row is visible. The exact retained mismatch and the replayed aggregation/assertion identify the failed check sufficiently. The earlier standalone report_deviation artifact/replay error is not used as evidence. Review labels on the original mapping do not invalidate the explicit single-target assumption established here.

[Standards quote, production lines, and provenance](../../evidence.md#dev-a64a9efad1)

</details>
