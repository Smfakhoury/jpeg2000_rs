# JPX Association inspection accepts fewer than two immediate children

`DEV-AFC4DEDE80` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § M.11.11

**Required behavior:** When validating an Association superbox as JPX metadata, require at least two immediate child boxes before returning a successful structural inspection; recursion and aggregate node counts do not replace immediate-child cardinality.

**Observed deviation:** JPX Association inspection accepts fewer than two immediate children

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-AFC4DEDE80 --output rfc-results/DEV-AFC4DEDE80
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib jp2::tests::association_requires_two_immediate_children -- --exact --nocapture
```

### Test input and recorded failure

A direct unit-contract entry has no requires predicates. The BoxRef is reader-produced, not forged; LBox is 8 with empty Association payload. Default limits admit eight bytes and depth 1; the recursive budget is u32::MAX-1. Empty child cardinality is intentionally invalid input for the structural validator, precisely the condition whose rejection the clause requires, not a violated separate caller/callee premise. Production callers pass the same depth/budget for metadata roots. Larger controls remain below all capacities and use unrestricted label/XML/Association children.

**Recorded failure** (matched by the replay runner):

```text
empty at depth 1: immediate=0, expected=None, observed=Ok((1, 1))
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_metadata_cardinality_test.rs:L17-L17`](files/src/jp2_metadata_cardinality_test.rs#L17)

```text
("empty".to_owned(), vec![], None),
```

[`src/jp2_metadata_cardinality_test.rs:L9-L9`](files/src/jp2_metadata_cardinality_test.rs#L9)

```text
let association = |children: &[Vec<u8>]| box32(*b"asoc", &children.concat());
```

[`src/jp2_metadata_cardinality_test.rs:L75-L79`](files/src/jp2_metadata_cardinality_test.rs#L75-L79)

```text
let mut reader = BoxReader::new(&bytes, &limits).unwrap();
            let root = reader.next_box_ref().unwrap().unwrap();
            assert_eq!(root.header.box_type(), BoxType::ASSOCIATION);
            assert_eq!(root.header.total_length(), bytes.len() as u64);
            assert!(reader.next_box_ref().is_none());
```

[`src/jp2_metadata_cardinality_test.rs:L88-L88`](files/src/jp2_metadata_cardinality_test.rs#L88)

```text
let observed = inspect_metadata_box(root, &limits, depth, u32::MAX - depth);
```

[`src/jp2_metadata_cardinality_test.rs:L98-L98`](files/src/jp2_metadata_cardinality_test.rs#L98)

```text
None => matches!(observed, Err(DecodeError::MalformedInput { .. })),
```

[`src/jp2_metadata_cardinality_test.rs:L110-L114`](files/src/jp2_metadata_cardinality_test.rs#L110-L114)

```text
assert!(
        failures.is_empty(),
        "M.11.11 immediate-child cardinality violations:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_metadata_cardinality_test.rs`](files/src/jp2_metadata_cardinality_test.rs)
- [`validation/metadata-cardinality-registration.patch`](files/validation/metadata-cardinality-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/metadata-cardinality-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jp2.rs'"'"').read_text(); assert s.count('"'"'    include!("jp2_metadata_cardinality_test.rs");'"'"') == 1; assert '"'"'fn retains_general_layer_cdef_and_nested_metadata()'"'"' in s; t=Path('"'"'src/jp2_metadata_cardinality_test.rs'"'"').read_text(); assert '"'"'fn association_requires_two_immediate_children()'"'"' in t; assert '"'"'failures.is_empty()'"'"' in t'
```

```sh
cargo test --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing predicate is the final failures.is_empty() assertion, whose first recorded mismatch is the empty Association at depth 1: expected rejection, observed Ok((1, 1)). All earlier framing/admission assertions completed. This is a valid direct unit-contract negative-input test, not a bypass of a required owning caller: BoxReader constructs the BoxRef from an ordinary eight-byte LBox=8/TBox=asoc frame, and the inspector's purpose includes inspecting malformed metadata. Requiring the very cardinality under test as an entry premise would be circular. Real production calls in the top-level parser and parse_compositing_layer pass depth=1 and remaining_depth=u32::MAX-1, exactly the first failing case's arguments. Higher-depth cases represent recursive subtree entries and are not necessary for the attribution.

Entry realism was checked: 37 fixtures at three entry depths make 111 independent calls, not one accumulated budget. Largest frame is 1221 bytes, largest tree 65 nodes, actual recursive stack at most four inspector frames. Default/HARDENED capacities are input 2147483648 bytes, metadata 16777216 bytes, depth 32, work 1000000000. Depths 1, 2 and 29 with maximum descendant edge distance 3 fit the configured limit; remaining_depth is ample. There is no oversized allocation, excessive nesting, or inadmissible fabricated header. The first failing fixture requires only eight input bytes, zero payload bytes and one node.

The origin is rfc_map, not infer. I read the pinned T.801 (06/2021) M.11.11 section via read_rfc_section (hash 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1). It explicitly defines an Association superbox as containing a sequence of two or more boxes, permits recursive associations, and describes label-plus-entity associations. Thus immediate-child cardinality, not aggregate descendant count, is the applicable structural rule. Excluding arbitrary metadata semantics does not exclude this framing/cardinality check; SUPPORTED.md also disallows accepting malformed structures as supported extensions. File-level branding/Reader Requirements placement is not an input requirement of this direct subtree-inspection unit. This assessment is confined to the cited 2021 edition; reconciliation with the project's newer selected edition remains an inherited review qualification, not a claim of newer-edition conformance.

describe_declaration and control_flow_graph confirm the route: admitted depth -> Association branch -> checked depth increment -> successful BoxReader::new -> count=1 and maximum_depth=depth -> next_box_ref returns None for empty payload -> loop breaks -> unconditional Ok((count, maximum_depth)). No immediate-child minimum is checked. Singleton children similarly recurse successfully and only affect aggregate count/depth. Consequently structurally malformed JPX associations are reported as successful inspections. The host replay compiled and registered the additive test-only include, then failed this exact cardinality assertion; it was not a setup/build/sanitizer failure. The success-shaped expected=None log is less explicit than an error name, but the replayed match explicitly requires MalformedInput for these cases. No implementation/model/native-oracle agreement was used to override the standard.

[Standards quote, production lines, and provenance](../../evidence.md#dev-afc4dede80)

</details>
