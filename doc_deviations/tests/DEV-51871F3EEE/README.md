# Metadata CSn incorrectly partitions logical bins and double-counts cached payload

`DEV-51871F3EEE` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** JPIP cached-payload accounting supports deterministic sparse data-bin reassembly and duplicate/overlap consistency, including metadata used for placeholder reconstruction; payload identity must follow the supported bin semantics rather than treating an irrelevant metadata codestream field as a new logical bin.

**Observed deviation:** Metadata CSn incorrectly partitions logical bins and double-counts cached payload

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-51871F3EEE --output rfc-results/DEV-51871F3EEE
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::stored_bytes_obeys_metadata_bin_identity_and_sparse_union -- --exact --nocapture
```

### Test input and recorded failure

One logical target per cache, metadata class 8 and root ID 0, valid 8/9/15/39-byte free boxes, bounded nonempty spans or legal empty final announcements, consistent overlap bytes and final lengths. Caches originate in DatabinCache::new and fragments in public JpipFragment::new via the existing helper. Default byte/fragment limits exceed the tiny sweep by orders of magnitude. CSn changes only an ignored metadata field under pinned T.808 A.3.6.1. The selected accounting failure predates intentionally conflicting probes, so no inadmissible negative-probe state is needed.

**Recorded failure** (matched by the replay runner):

```text
relabeled n=39 step=2 CSn=7: stored_bytes=41 expected union=31
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip/stored_bytes_tests.rs:L31-L32`](files/src/jpip/stored_bytes_tests.rs#L31-L32)

```text
        let mut canonical = DatabinCache::new(ResourceLimits::default());
        let mut relabeled = DatabinCache::new(ResourceLimits::default());
```

[`src/jpip/stored_bytes_tests.rs:L51-L52`](files/src/jpip/stored_bytes_tests.rs#L51-L52)

```text
            let csn = [0, 1, 7, 127, 128][step % 5];
            let alias = DatabinId::new(8, csn, 0).unwrap();
```

[`src/jpip/stored_bytes_tests.rs:L59-L61`](files/src/jpip/stored_bytes_tests.rs#L59-L61)

```text
                let result = cache.insert(fragment(
                    target, start as u64, &bytes[start..end], end == n,
                ));
```

[`src/jpip/stored_bytes_tests.rs:L64-L65`](files/src/jpip/stored_bytes_tests.rs#L64-L65)

```text
            seen[start..end].fill(true);
            let expected = seen.iter().filter(|&&present| present).count() as u64;
```

[`src/jpip/stored_bytes_tests.rs:L67-L69`](files/src/jpip/stored_bytes_tests.rs#L67-L69)

```text
                check!(cache.stored_bytes() == expected,
                    "{name} n={n} step={step} CSn={csn}: stored_bytes={} expected union={expected}",
                    cache.stored_bytes());
```

[`src/jpip/stored_bytes_tests.rs:L139-L139`](files/src/jpip/stored_bytes_tests.rs#L139)

```text
    assert!(failures.is_empty(), "stored_bytes contract violations:\n{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip/stored_bytes_tests.rs`](files/src/jpip/stored_bytes_tests.rs)
- [`stored-bytes-registration.patch`](files/stored-bytes-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i stored-bytes-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip/stored_bytes_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jpip/stored_bytes_tests.rs");'"'"') == 1; assert t.count('"'"'fn stored_bytes_obeys_metadata_bin_identity_and_sparse_union()'"'"') == 1; assert '"'"'expected union={expected}'"'"' in t; print('"'"'verified registration and oracle in replay source'"'"')'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The standards-derived clause is sound for same-target class-8 metadata. I read clause_origin (rfc_map), then independently retrieved its pinned T.808 (12/2022) A.3.6.1 through the originating map item 278a47b0ee3cd36f4c3d6f3c, anchor bb808185830df51cc635e32dd74dc317202aedeacc9e1a54cd6eacc7a0d5774c. It explicitly applies to both JPP and JPT and says: 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' Root bin 0 and box metadata match this fixture. This is not a request to normalize the raw header accessor, and agreement between implementation and refinement would not override this receiving-side rule. No claim about a newer standard edition is needed.

Realism: the additional fixtures contain 8, 9, 15, or 39 bytes of a valid free box, with one logical target per cache. Each sparse sweep has 18 spans plus one empty final announcement, followed by six negative probes; there is no recursive assembly or large stack allocation. Even five incorrectly separated CSn bins have at most 195 payload bytes before negative probes. Default limits are 2,147,483,648 input bytes and 1,048,576 fragments. The exact-budget cache deliberately admits n bytes and then a duplicate adding zero logical bytes; this is not excessive input. CSn values through 129 are irrelevant metadata fields, not 130 constructed codestreams. Existing callers ingest parsed messages via JpipFragment::from_message and cache.insert, without a metadata CSn normalization step. The direct cache-unit route is valid independently of wire parsing.

The first failed assertion is the test's final aggregate assert!(failures.is_empty(), ...), not a setup unwrap or sanitizer failure: prior checks collect diagnostics rather than aborting. The retained tail truncates the earliest collected diagnostic, so I do not claim its chronological identity. It nevertheless exposes the exact accounting check: 'relabeled n=39 step=2 CSn=7: stored_bytes=41 expected union=31', and later 'relabeled n=39 step=16 CSn=1: stored_bytes=169 expected union=39'. These checks occur before negative probes, so subsequent deliberate conflicts cannot explain them. An independent exact-budget probe reports a duplicate rejected with actual=78 at limit=39. These specifically refute the clause's requirement not to treat an irrelevant metadata codestream field as a new logical bin.

Route: public new -> public DatabinId::new(8, CSn, 0) -> existing fragment helper using public JpipFragment::new -> insert -> find_databin_index -> DatabinId::same_as -> refinement::jpip_databin_identity. The predicate unconditionally requires equal codestream fields. MIR for find_databin_index confirms false comparisons loop until None; insert then creates a fresh Databin, finds no replaced coverage, adds its span to the global stored_bytes, and commits it. The getter's declaration and MIR show only an immutable field read and return; its arithmetic is not the defect. The defect is the upstream semantic key that makes stored accounting and consistency operate on the wrong logical partition. Consequences include duplicate payload charging, split completeness, bypassed overlap conflicts, and false InputBytes rejections. Constructors and insert are public and all state is created through them; no private-state corruption or skipped owning initializer is involved. No source changes are proposed or made.

[Standards quote, production lines, and provenance](../../evidence.md#dev-51871f3eee)

</details>
