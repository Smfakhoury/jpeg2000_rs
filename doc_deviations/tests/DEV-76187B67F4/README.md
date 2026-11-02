# Automatic tile-header normalization rejects complete JPP bins without optional SOD

`DEV-76187B67F4` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.3

**Required behavior:** For the documented automatic reconstruction consumer, accept a complete valid JPP tile-header data-bin without requiring an optional on-wire SOD marker, and synthesize the SOT/SOD framing needed by the derived codestream.

**Observed deviation:** Automatic tile-header normalization rejects complete JPP bins without optional SOD

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-76187B67F4 --output rfc-results/DEV-76187B67F4
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --quiet -- jpip::tests::automatic_normalization_accepts_optional_wire_sod_contract --exact --nocapture
```

### Test input and recorded failure

Admissible direct unit-contract input: tile 0, valid empty immutable Rust slice obtained from a complete, present class-2 bin, no SOT, omitted optional SOD, one transition, no malformed marker segments, and ordinary successful cache allocation/insertion. T.808 (12/2022) A.3.3 expressly permits optional SOD and requires empty tile-header bins to be sent. A valid main header can supply all coding defaults; no whole codestream is an input to this helper. The first failing entry requires no large tile count or resource stress. This assesses a valid unit entry, not full consumer reachability.

**Recorded failure** (matched by the replay runner):

```text
tile=0 retained=0 SOD=false: valid complete class-2 bin rejected: MalformedInput { offset: 0, kind: MissingRequiredStructure }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_normalization_contract_tests.rs:L27-L27`](files/src/jpip_normalization_contract_tests.rs#L27)

```text
    let mut headers = vec![Vec::new()];
```

[`src/jpip_normalization_contract_tests.rs:L39-L51`](files/src/jpip_normalization_contract_tests.rs#L39-L51)

```text
            for sod in [true, false] {
                let mut wire = retained.clone();
                if sod {
                    wire.extend_from_slice(&[0xff, 0x93]);
                }
                assert!(!wire.windows(2).any(|bytes| bytes == [0xff, 0x90]));
                let target = id(2, u64::from(tile));
                let mut cache = DatabinCache::new(ResourceLimits::default());
                cache.insert(fragment(target, 0, &wire, true)).unwrap();
                assert!(cache.is_complete(target), "complete, not absent/incomplete");
                let source = cache.databin_view(target).unwrap().into_bytes();
                assert_eq!(source, wire);
                let result = normalize_automatic_tile_headers(&source, tile);
```

[`src/jpip_normalization_contract_tests.rs:L62-L65`](files/src/jpip_normalization_contract_tests.rs#L62-L65)

```text
                    Err(error) => failures.push(format!(
                        "tile={tile} retained={} SOD={sod}: valid complete class-2 bin rejected: {error:?}",
                        retained.len()
                    )),
```

[`src/jpip_normalization_contract_tests.rs:L74-L77`](files/src/jpip_normalization_contract_tests.rs#L74-L77)

```text
    assert!(
        failures.is_empty(),
        "{} contract failures; first examples:\n{}",
        failures.len(),
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_normalization_contract_tests.rs`](files/src/jpip_normalization_contract_tests.rs)
- [`register_normalization_contract_tests.py`](files/register_normalization_contract_tests.py)

**Build commands** (in order):

```sh
python3 register_normalization_contract_tests.py
```

```sh
cargo test --lib --no-run --quiet
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is failures.is_empty() at src/jpip_normalization_contract_tests.rs:74. Its first collected failure is tile=0, retained=0, SOD=false: a complete empty class-2 bin returns MalformedInput/MissingRequiredStructure at offset 0. This specifically refutes standards-derived clause 153c097eaf82d53210996129, not merely an exit-code expectation.

Realism was assessed before tracing the implementation: the first case performs 352 sequential normalizations (22 tile identifiers x 8 headers x 2 optional-SOD variants), with a fresh cache and one fragment per call, at most 1032 input bytes, one parsed transition, heap-backed vectors, and no recursion or large stack fixture. Default HARDENED limits allow 2,147,483,648 input bytes and 1,048,576 JPIP fragments. Some boundary tile identifiers in the wider sweep exceed the default maximum_tiles=16,384 and are unnecessary to this diagnosis; the reproduced tile=0 empty case has no such issue, needs only a one-tile image, and is the smallest realistic case. Real automatic callers pass the same complete class-2 bin bytes directly to normalization. No pool exhaustion, setup panic, malformed segment, or missing-bin condition explains the first failure.

Authority is the rfc_map origin, not the conflicting inferred rejection clause. I read the original mapping's pinned T.808 (12/2022), A.3.3, SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, through read_rfc_section using origin item 985a0957d228f2a2cf94cfa0. It expressly says a JPP tile-header bin shall not contain SOT, inclusion of SOD is optional, and the server must send a tile-header bin even when empty. These rules apply to the documented automatic JPP consumer. The explicit-plan already-reframed-header restriction in SUPPORTED.md is separate; automatic reconstruction performs its own normalization. Thus requiring SOD is not a justified precondition or grounds to revise this standards clause. No assertion is made about an unacquired later edition.

The replay is a direct unit_contract test, not a purported end-to-end reconstruction. It uses existing id/fragment/segment helpers, safe immutable bytes, normal resource limits and a final fragment, then establishes cache.is_complete and exact databin_view bytes before calling normalize_automatic_tile_headers. Mandatory coding defaults may be inherited from a valid main header; this helper receives only tile-header bytes and its parser contract expressly accepts an empty marker-only header. No owning-caller invariant is fabricated or bypassed. A complete automatic-consumer reconstruction is not claimed. Source callers build_automatic_snapshot and automatic_headers independently establish relevance: both pass raw bin bytes without inserting SOD.

describe_declaration and control_flow_graph confirm the failure route. parse_tile_part_transitions takes its non-SOT branch and returns one owned transition with an empty header. Normalization's count conversion succeeds (N=1), source is empty, marker_start and position are 0, and found_sod is false. MIR bb58 initializes that state; bb59 skips scanning; bb87 branches to bb89, and bb90 returns MissingRequiredStructure. The explicit !found_sod check rejects a valid bin before the later synthesized-SOD append/output push. The same defect rejects the seven nonempty, fully framed marker-only fixtures without SOD, producing 176 recorded failures. This is a reproduced code deviation under the pinned standard, not a test error or a standards misreading. The aggregate assertion gives no per-call stack, but the quoted diagnostic, deterministic loop order and compiler/source route identify the first failed call sufficiently; only the host's per-case replay output is used as execution evidence.

[Standards quote, production lines, and provenance](../../evidence.md#dev-76187b67f4)

</details>
