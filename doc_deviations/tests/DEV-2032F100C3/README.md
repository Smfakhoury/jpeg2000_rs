# JP2 palette validator accepts NE above the T.800 (06/2019) limit of 1024

`DEV-2032F100C3` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § I.5.3.4

**Required behavior:** JP2 palette validation against the local T.800 (06/2019) contract should admit NE only in 1..1024, while preserving the validated entry count and complete table.

**Observed deviation:** JP2 palette validator accepts NE above the T.800 (06/2019) limit of 1024

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-2032F100C3 --output rfc-results/DEV-2032F100C3
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --locked jp2::tests::palette_contract_t800_entry_count_domain_and_complete_table -- --exact --nocapture
```

### Test input and recorded failure

The assessed parser unit entry has no requires. All inputs are initialized safe Rust values with valid shared borrows. Each negative fixture has NPC=1, a supported unsigned-eight-bit descriptor, exact normal 8-byte pclr framing and a complete NE-byte table, without padding or truncation. Header fields are produced by BoxReader rather than fabricated. At NE=1025 there are 1029 payload bytes and 1037 box bytes. The largest case is 65547 box bytes, under default input/metadata limits; a single box requires one framing work unit and no recursion. Only the tested NE validation predicate is false. No callee receives an invalid precision, width, offset, or header. The sole source patch registers test code under cfg(test), preserving production behavior.

**Recorded failure** (matched by the replay runner):

```text
T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted [1025, 1026, 2048, 32768, 65535]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_palette_contract_tests.rs:L64-L65`](files/src/jp2_palette_contract_tests.rs#L64-L65)

```text
let mut reader = BoxReader::new(bytes, &ResourceLimits::default()).unwrap();
    let box_ref = reader.next_box_ref().unwrap().unwrap();
```

[`src/jp2_palette_contract_tests.rs:L76-L77`](files/src/jp2_palette_contract_tests.rs#L76-L77)

```text
assert!(reader.next_box_ref().is_none());
    parse_palette(box_ref, 0)
```

[`src/jp2_palette_contract_tests.rs:L179-L179`](files/src/jp2_palette_contract_tests.rs#L179)

```text
for entries in [0, 1025, 1026, 2048, 32768, u16::MAX] {
```

[`src/jp2_palette_contract_tests.rs:L192-L196`](files/src/jp2_palette_contract_tests.rs#L192-L196)

```text
Ok(palette) => {
                contract_assert_retained_palette(&palette, entries, &columns, &expected);
                wrongly_admitted.push(entries);
                eprintln!("predicate violated: admitted forbidden NE={entries}");
            }
```

[`src/jp2_palette_contract_tests.rs:L199-L202`](files/src/jp2_palette_contract_tests.rs#L199-L202)

```text
assert!(
        wrongly_admitted.is_empty(),
        "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted {wrongly_admitted:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_palette_contract_tests.rs`](files/src/jp2_palette_contract_tests.rs)
- [`validation/palette-contract-tests.patch`](files/validation/palette-contract-tests.patch)

**Build commands** (in order):

```sh
patch --batch --fuzz=0 -p1 -i validation/palette-contract-tests.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jp2.rs'"'"').read_text(); t=Path('"'"'src/jp2_palette_contract_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jp2_palette_contract_tests.rs");'"'"') == 1; assert '"'"'fn validates_palette_mapping_relationships()'"'"' in s; assert '"'"'fn retains_signed_palette_and_channel_definition_records()'"'"' in s; assert '"'"'fn palette_contract_wire_order_precision_and_signedness()'"'"' in t; assert '"'"'fn palette_contract_t800_entry_count_domain_and_complete_table()'"'"' in t; print('"'"'Verified test-only registration and both clause assertions in replay source'"'"')'
```

```sh
cargo test --lib --locked --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is wrongly_admitted.is_empty(), with host output: "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted [1025, 1026, 2048, 32768, 65535]". All preceding legal-count retention checks passed. This specifically refutes the NE admission part of clause e8e89fffa5faef739c2121f3, not table preservation. The host also explicitly logged acceptance of NE=1025, so attribution does not rest on cargo's nonzero exit alone.

Entry realism: this failing case uses one normally framed pclr box at a time, no recursion, no fabricated private state, and default ResourceLimits. Legal cases range over NE=1..1024 with three columns (eight encoded bytes per row), at most 8,206 input bytes and 3,072 i64 values per table; the 1,024 iterations total 524,800 rows, processed sequentially. Negative cases use one unsigned-eight-bit column, with box size NE+12, at most 65,547 bytes and 65,535 i64 values (524,280 bytes per retained or expected vector). These sizes are well below default limits of 2,147,483,648 input bytes, 16,777,216 metadata bytes and 1,000,000,000 framing work units; there is one box and no nesting or stack-sized table. The minimal refutation, NE=1025, needs only 1,037 input bytes. Real header callers likewise obtain framed BoxRefs and pass them directly to parse_palette without prevalidating NE; no owning caller establishes an NE<=1024 entry requirement. The supplied assessed unit_contract entry has no requires, and all downstream contracts are respected: descriptor 7 means unsigned precision 8, each sample slice has length one, offsets are zero, and header fields originate in BoxReader. Out-of-domain NE is the field this validator must reject, not an unsafe or inadmissible parser entry.

The replay patch only adds an include inside the existing cfg(test) module. The route is the replayed unit test -> contract_palette_fixture -> box32 -> contract_parse_palette_box -> BoxReader::new/next_box_ref -> unchanged parse_palette. For NE=1025,NPC=1, payload length is exactly 3+1+1025=1029. Compiler declaration evidence identifies the actual parser and callees; MIR bb13 checks entries==0 and bb14 checks columns==0, then permits the nonzero case. The supported descriptor and exact-length checks succeed, the loops decode all entries, and bb166 returns Ok(Palette). There is no upper-bound check. The harness verifies that returned palette before collecting the forbidden count and failing the range assertion.

Authority: clause_origin is rfc_map, not infer. I read its parent finding and re-read pinned T.800 (06/2019) I.5.3.4 using the origin item e033943567bf66a65e7591f2; document SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e. Both NE prose and Table I.12 expressly give 1 to 1024. This is the ordinary JP2 pclr interpretation, with no JPX reinterpretation. SUPPORTED.md advertises JP2 container validation and rejects malformed structures as extensions. Thus this is a reproduced mismatch against the explicitly named local 2019 validation contract, not grounds to revise a standards-derived clause to match the implementation. The project names a newer 2024 target and disclaims certification; this finding does not establish a 2024 conformance failure or claim the standard mandates a particular DecodeError variant. Whole-file acceptance and other palette/header constraints are not inferred from this direct unit reproduction.

[Standards quote, production lines, and provenance](../../evidence.md#dev-2032f100c3)

</details>
