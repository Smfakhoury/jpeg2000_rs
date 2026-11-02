# Metadata-bin availability incorrectly partitions cached bytes by codestream index

`DEV-EFECF7D00D` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Provide deterministic data-bin reassembly and availability/missing-range results for the supported JPIP data-bin classes, so incomplete cached data is not mistaken for successful reconstruction.

**Observed deviation:** Metadata-bin availability incorrectly partitions cached bytes by codestream index

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-EFECF7D00D --output rfc-results/DEV-EFECF7D00D
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib availability_contract_ -- --nocapture --test-threads=1
```

### Test input and recorded failure

An admissible safe-public-API unit entry: a single logical target's root metadata bin 0, two non-overlapping eight-byte fragments of one 16-byte free box, valid explicit class/CSn framing, offsets 0 and 8, final endpoint 16, default resource limits, one checked positive-length 16-byte request. All parsing, construction and insertion return Ok; no private cache mutation, invalid final-length conflict, excessive resource use, or missing owning-caller requirement. Under T.808 (2022) A.3.6.1 the metadata CSn is semantically ignored, so 0 and 1 do not create different metadata bins.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: metadata CSn must not partition availability
  left: JpipAvailabilityReport { status: Partial, required_bytes: 16, available_bytes: 8, missing: [JpipMissingRange { databin: DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 }, offset: 8, length: Some(8) }] }
 right: JpipAvailabilityReport { status: Available, required_bytes: 16, available_bytes: 16, missing: [] }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_availability_contract_tests.rs:L276-L280`](files/src/jpip_availability_contract_tests.rs#L276-L280)

```text
    for second_stream in [0_u8, 1] {
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        let mut context = JpipHeaderContext::new();
        for (offset, stream) in [(0_u8, 0), (8, second_stream)] {
```

[`src/jpip_availability_contract_tests.rs:L283-L285`](files/src/jpip_availability_contract_tests.rs#L283-L285)

```text
            let mut wire = vec![if final_byte { 0x70 } else { 0x60 }, 8, stream, offset, 8];
            wire.extend_from_slice(&metadata[offset as usize..offset as usize + 8]);
            let (message, used) = parse_jpip_message(&wire, &mut context, &limits).unwrap();
```

[`src/jpip_availability_contract_tests.rs:L296-L298`](files/src/jpip_availability_contract_tests.rs#L296-L298)

```text
            cache
                .insert(JpipFragment::from_message(header, body, &limits).unwrap())
                .unwrap();
```

[`src/jpip_availability_contract_tests.rs:L308-L310`](files/src/jpip_availability_contract_tests.rs#L308-L310)

```text
        let report = cache
            .availability(&[JpipDatabinRange::new(target, 0, 16).unwrap()])
            .unwrap();
```

[`src/jpip_availability_contract_tests.rs:L322-L326`](files/src/jpip_availability_contract_tests.rs#L322-L326)

```text
    assert_eq!(reports[0], expected, "same-CSn control");
    assert_eq!(
        reports[1], expected,
        "metadata CSn must not partition availability"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_availability_contract_tests.rs`](files/src/jpip_availability_contract_tests.rs)
- [`availability_contract.patch`](files/availability_contract.patch)

**Build commands** (in order):

```sh
patch --batch -p1 -i availability_contract.patch
```

```sh
grep -Fq 'include!("jpip_availability_contract_tests.rs");' src/jpip.rs
```

```sh
grep -Fq 'metadata CSn must not partition availability' src/jpip_availability_contract_tests.rs
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is reports[1] == expected, labelled "metadata CSn must not partition availability", at replayed src/jpip_availability_contract_tests.rs:323. The same-CSn control passed; the changed-CSn case returned Ok(Partial), required_bytes=16, available_bytes=8, with a spurious missing interval [8,16), despite receiving both halves of the same metadata bin. This is a deviation from the targeted documented supported-class reassembly/availability clause (8f9b87d47d2934d844317263), interpreted using its rfc_map origin, not a reason to revise that clause to match the implementation. I read the parent finding and independently retrieved pinned T.808 (2022), A.3.6.1 (hash 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc): it explicitly says the codestream index shall be ignored in any metadata-bin message. It applies to both JPP and JPT streams and to this single logical target/root metadata bin 0. No image-bin cross-codestream merging is asserted. The separately inferred clause describing codestream-sensitive identity cannot override this metadata-specific normative exception.

Entry realism: each variant uses two 13-byte wire messages (five header bytes plus eight body bytes), stores only 16 payload bytes in two fragments, and queries one positive-length range [0,16). CSn values are only 0 and 1, all arithmetic is representable, there is no recursive structure, and default limits are HARDENED: input 2,147,483,648 bytes, metadata 16,777,216 bytes, fragments 1,048,576, packets 67,108,864, work 1,000,000,000. The availability scan visits one stored interval in the failing case (one merged interval in the control); no capacity is approached. These are ordinary bounded finite-range requests like the native availability consumers, not oversized synthetic inputs. The direct public-unit contract requires no owning reconstruction caller. One fresh cache/context per variant establishes target isolation; root bin 0 carries a complete 16-byte free box, with non-overlapping offsets 0 and 8 and finality announced at endpoint 16. parse_jpip_message, JpipFragment::from_message, insert, and JpipDatabinRange::new all succeed normally. Test-only inclusion exposes the existing module's fixtures and report construction for assertions; it does not fabricate internal cache state or change production behavior.

Source and compiler evidence establish the route: parsing preserves raw CSn in DatabinId; from_message copies that identity; insert calls find_databin_index, whose same_as delegates to jpip_databin_identity. That predicate unconditionally requires equal codestreams, even for class 8, so the second fragment is stored under a second bin. availability's lookup selects only class=8/CSn=0/bin=0. Its MIR lookup path bb30 -> bb33 -> bb34 retrieves that bin, advances cursor to 8 via bb69-bb71, then emits the finite tail gap via bb76 and returns Partial via bb101 -> bb102 -> bb104 -> bb106 -> bb107. Thus the specific failed check observes a false missing range and undercounts available bytes, not a setup panic, resource rejection, arbitrary nonzero exit, or ungrounded entry claim. Host replay built successfully and observed the exact report before the assertion; the bitmap and resource subtests passed. The earlier candidate endpoint inconsistency mentioned in test notes is not used as reproduction evidence; this case's host replay is the evidence.

[Standards quote, production lines, and provenance](../../evidence.md#dev-efecf7d00d)

</details>
