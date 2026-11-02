# Reversible decorrelation resolver rejects normative N(N+1) coefficient arrays using an N² count

`DEV-49C7BAFDE3` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § J.3.1.1.3

**Required behavior:** Resolve array-backed collections according to the documented graph-collection model, rejecting coefficient vectors whose length differs from the computed expected element count and enforcing reversible precision checks.

**Observed deviation:** Reversible decorrelation resolver rejects normative N(N+1) coefficient arrays using an N² count

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-49C7BAFDE3 --output rfc-results/DEV-49C7BAFDE3
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
bash -c 'for bin in target/deepspec-mct/debug/deps/jpeg2000_rs-*; do if test -x "$bin" && ! test -d "$bin"; then exec "$bin" --exact codestream::tests::resolve_mct_collection_clause_7ca4daea --nocapture; fi; done; exit 2'
```

### Test input and recorded failure

A valid direct unit entry, not a claimed complete codestream. Parser is initialized with empty input and HARDENED limits through new_parser; parse_mct accepts explicit complete unfragmented segments with Zmct=Ymct=0. For the quoted case there are six unique input components and six permuted unique output components, matching nonzero decorrelation index 7, no offsets selected, 42 Int16 integer coefficients, zero fractional precision, scale factors 2 and final scale 4. These satisfy the retrieved 2021 J.3.1.1.3 conditions and native caller/reference/lookup contracts. Six components and 42 elements are far below caps of 64 and 4096, respectively; all tested sizes remain within those caps. The resolver does not require the parser input to contain the separately supplied arrays, nor an N*N array as an entry premise.

**Recorded failure** (matched by the replay runner):

```text
direct resolve_mct_collection: J.3.1.1.3 N=6, integer coefficients=42, offsets=false: expected full collection, got Err(MalformedInput { offset: 0, kind: InvalidTransform })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/resolve_mct_contract.rs:L9-L10`](files/src/codestream/resolve_mct_contract.rs#L9-L10)

```text
let limits = ResourceLimits::HARDENED;
    let parser = new_parser(&[], &limits).unwrap();
```

[`src/codestream/resolve_mct_contract.rs:L19-L20`](files/src/codestream/resolve_mct_contract.rs#L19-L20)

```text
let count = (n + 1) * n;
        assert!(count <= usize::from(limits.maximum_components()).pow(2));
```

[`src/codestream/resolve_mct_contract.rs:L26-L27`](files/src/codestream/resolve_mct_contract.rs#L26-L27)

```text
for row in 0..n { coefficients[row * n + n - 1 - row] = 2; }
        coefficients[count - 1] = 4;
```

[`src/codestream/resolve_mct_contract.rs:L39-L39`](files/src/codestream/resolve_mct_contract.rs#L39)

```text
parser.parse_mct(Segment { marker: MCT, offset: 0, payload: &bytes[4..] }).unwrap()
```

[`src/codestream/resolve_mct_contract.rs:L65-L65`](files/src/codestream/resolve_mct_contract.rs#L65)

```text
kind: MctArrayKind::Decorrelation, reversible: true, transform_index: 7, offset_index: 0,
```

[`src/codestream/resolve_mct_contract.rs:L76-L80`](files/src/codestream/resolve_mct_contract.rs#L76-L80)

```text
let actual = parser.resolve_mct_collection(&reference, &arrays);
            checks += 1;
            if actual != expected {
                failures.push(format!("direct resolve_mct_collection: J.3.1.1.3 N={n}, integer coefficients={count}, offsets={with_offsets}: expected full collection, got {actual:?}"));
            }
```

[`src/codestream/resolve_mct_contract.rs:L113-L113`](files/src/codestream/resolve_mct_contract.rs#L113)

```text
assert!(failures.is_empty(), "{} discriminating checks failed:\n{}", failures.len(), failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/resolve_mct_contract.rs`](files/src/codestream/resolve_mct_contract.rs)

**Build commands** (in order):

```sh
bash -c 'printf '"'"'\ninclude!("resolve_mct_contract.rs");\n'"'"' >> src/codestream/tests.rs && cargo test --locked --lib --no-run --target-dir target/deepspec-mct'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

This is a reproduced unit-contract code deviation for the standards-derived meaning of clause 7ca4daea027df926ca127cca, not an instruction to rewrite its oracle to match the Verus model. I read clause_origin (rfc_map b091a9c63532ebcacba21070, finding a72486651c3b6f2a46bb7fdec2e050397216c308f97f94dbb365476614475380) and retrieved its original T.801 (06/2021) J.3.1.1.3 section using read_rfc_section with the origin item. Outside the informative factorization note, it explicitly requires (N+1)*N coefficients for a nonzero reversible decorrelation array index, equal nonempty input/output cardinalities, integer coefficients and power-of-two scale factors. The positive harness meets these conditions. Agreement of the native model test and implementation on N*N cannot override this normative count. Qualification: project REFERENCES.md selects the 2023 edition and explicitly leaves its reconciliation with the acquired 2021 edition open. This diagnosis establishes the admission mismatch against the cited 2021 authority; it does not assert verified 2023 conformance differences or complete numerical transform correctness.

Realism was assessed before pursuing the branch: dimensions are only 1..8,16,31,63, bounded by HARDENED.maximum_components=64; the largest positive array is 4032 elements, below parse_mct's 4096-element cap. Its Int16 marker is 8074 bytes including marker/length/header, well below the 16-bit marker length limit. The largest wrong-length probe is 4033 elements, also below the cap. Allocations are small heap vectors, cases are sequential, and there is no recursive fixture. The real decode_mcc_collection caller accepts the same reversible decorrelation variant with equal unique component lists and transform index 7, offset index 0 or 8; resolve_mct_stage forwards those references and arrays without imposing an N*N entry precondition. The direct resolver is a valid unit entry: new_parser(&[], &limits) is valid, parse_mct explicitly receives complete legal segments independent of parser.input, arrays have unique matching indices/kinds, and fraction_bits=0. This is not a fixture requiring an uncalled owner to establish hidden state. The earlier reused broad native model sweep includes synthetic states, but it returns successfully; those states are not the failing positive setup.

The failing assertion is the final failures.is_empty() aggregate: positive checks append a discrepancy when actual != the full expected collection. The host tail truncates the earliest discrepancy, so I do not invent its N value; an exact retained positive discrepancy is N=6, offsets=false, 42 integer coefficients returning InvalidTransform. Compiler declaration/CFG and source establish the route: resolve_mct_collection accepts rows=columns=6, uses no offsets, finds/clones decorrelation array 7, then calls expected_mct_elements(6,6,Decorrelation,true). Its first Decorrelation branch ignores reversible and returns 36. MIR bb32 calls the helper, bb38-bb39 compute 42 != 36, and bb44 takes bb48-bb49 to InvalidTransform. The required complete array is therefore rejected. The output also records acceptance of N*N arrays, corroborating the admission boundary, but attribution rests on the valid positive case, not potentially invalid truncated coefficient fixtures. Fractional rejection at normative count may be masked by the dimension guard and is not an independently established precision deviation. No full codestream/profile reachability or numerical reconstruction result is claimed, and no production edits or new replay were needed.

[Standards quote, production lines, and provenance](../../evidence.md#dev-49c7bafde3)

</details>
