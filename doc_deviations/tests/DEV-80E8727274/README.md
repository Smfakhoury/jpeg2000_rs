# Precinct validator accepts non-LRCP effective tile COD overrides

`DEV-80E8727274` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.7.3

**Required behavior:** For every input byte slice, requested u16 layer count, and ResourceLimits value, observe a normal return for which parsing the slice with those limits succeeds as L and L.info.image_size.tile_count() succeeds as T. The result is Ok(()) if and only if all of the following hold: T=1; L.tile_parts.len()=1; the main-header coding style declares exactly the requested number of layers and LRCP progression; the sole tile part's effective coding style also uses LRCP progression; neither the main header nor the sole tile part has any progression-change records; and the sole tile part has neither an attached packet-length source nor an attached packed-packet-header source. Otherwise the result is Err(DecodeError::MalformedInput { offset: 0, kind: MalformedKind::InvalidJpipLayout }). An attached packet-length or packed-header source is forbidden even when it represents zero entries or zero bytes. This is structural eligibility for explicit-plan reconstruction, not a guarantee of successful entropy decoding.

**Observed deviation:** Precinct validator accepts non-LRCP effective tile COD overrides

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-80E8727274 --output rfc-results/DEV-80E8727274
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/precinct-contract-tests --exact jpip::tests::precinct_contract_structural_eligibility --nocapture --test-threads=1
```

### Test input and recorded failure

A safe direct unit-contract entry with real parsed bytes, not a synthetic CodestreamLayout: 96-byte 1x1 single-component, single-tile, single-part stream, one requested and declared layer, default ResourceLimits, valid tile COD override, no progression-change or attached packet-header/length sources. A zero-byte packet header represents an empty packet; full entropy decoding is not required by this structural clause. The owning caller need not execute for this unit contract, and only the direct-validator false acceptance is claimed.

**Recorded failure** (matched by the replay runner):

```text
main progression=0, effective tile progression=1: expected Err(MalformedInput { offset: 0, kind: InvalidJpipLayout }), got Ok(())
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_precinct_validation_tests.rs:L59-L59`](files/src/jpip_precinct_validation_tests.rs#L59)

```text
let limits = ResourceLimits::default();
```

[`src/jpip_precinct_validation_tests.rs:L113-L114`](files/src/jpip_precinct_validation_tests.rs#L113-L114)

```text
let override_cod = segment(0xff52, &[0, tile, 0, 1, 0, 0, 0, 0, 0, 1]);
            let bytes = precinct_contract_stream(1, main, &[], &[override_cod], &[0]);
```

[`src/jpip_precinct_validation_tests.rs:L120-L123`](files/src/jpip_precinct_validation_tests.rs#L120-L123)

```text
assert_eq!(
                layout.tile_parts[0].coding_style.progression_order,
                orders[usize::from(tile)]
            );
```

[`src/jpip_precinct_validation_tests.rs:L124-L130`](files/src/jpip_precinct_validation_tests.rs#L124-L130)

```text
check(
                &format!("main progression={main}, effective tile progression={tile}"),
                &bytes,
                1,
                &limits,
                main == 0 && tile == 0,
            );
```

[`src/jpip_precinct_validation_tests.rs:L71-L71`](files/src/jpip_precinct_validation_tests.rs#L71)

```text
let actual = validate_precinct_codestream(bytes, layers, limits);
```

[`src/jpip_precinct_validation_tests.rs:L31-L33`](files/src/jpip_precinct_validation_tests.rs#L31-L33)

```text
let length = u32::try_from(bytes.len() - start).unwrap();
    bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&EOC);
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_precinct_validation_tests.rs`](files/src/jpip_precinct_validation_tests.rs)
- [`validation/precinct-contract-alloc/Cargo.toml`](files/validation/precinct-contract-alloc/Cargo.toml)
- [`validation/precinct-contract-alloc/lib.rs`](files/validation/precinct-contract-alloc/lib.rs)
- [`validation/build-precinct-contract-tests.py`](files/validation/build-precinct-contract-tests.py)

**Build commands** (in order):

```sh
python3 validation/build-precinct-contract-tests.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed eligibility comparison is main progression=0, effective tile progression=1: the host observed Ok(()) instead of InvalidJpipLayout at offset 0. The final assertion aggregates that comparison and the analogous tile orders 2, 3 and 4; fixture parsing and parsed-order assertions completed successfully before that assertion. This violates exactly the clause's effective-tile LRCP requirement. The clause is sound: SUPPORTED.md restricts explicit-plan reconstructed input to one tile-part with LRCP progression, not merely an LRCP main-header default. The parser intentionally accepts a tile COD override and exposes it as the tile part's effective coding_style.

Entry realism was checked: each failing fixture is only 96 bytes, a 1x1 image, one component, one tile, one tile part, one layer, zero decomposition levels, and a one-byte empty-packet payload, with ordinary owned Vec storage and ResourceLimits::default(). This is well below hardened limits (2,147,483,648 input bytes, 100,000,000 pixels, 64 components, 16,384 tiles/parts, 256 layers and 33 resolutions); no excessive stack, pool or nesting demand exists. The existing real-caller regression uses the same small codestream constructor. Separate successful layer-sweep cases use a test-only expanded layer limit up to 65,535 and about 65.6 KB of bytes; those are not the failing cases, and the progression matrix explicitly uses the unmodified default limits.

This is an admissible direct unit-contract test, not a bypassed caller-owned state fixture: the entry takes immutable bytes/layers/limits, has no additional preconditions, and itself parses the actual bytes. The harness independently parses those same bytes and verifies the main and effective tile progression orders, then directly invokes the original private validator through a cfg(test) include. The replay builder only registers test code and a test allocator dependency; it does not change production behavior. Real caller context is consistent: transcode_precinct_codestream assembles headers/packets/EOC and calls this function as its final gate before returning Some(codestream). The negative caller path itself was not replayed and is not claimed.

Compiler declaration and CFG confirm the route: parse_layout succeeds (bb0-bb4), tile_count is one (bb8), tile_parts.len() is one (bb11), requested/main layers match (bb12), main progression is LRCP (bb13), and no main/tile POC, attached lengths or packed headers are present (bb14-bb24). Execution reaches Ok(()) in bb27 without examining tile_parts[0].coding_style.progression_order. Source read_tile_part assigns the tile COD into tile_coding_style and returns it in TilePartPayload, so the unchecked override is real parsed state. Consequence established by replay is false structural eligibility for a documented non-LRCP input; no entropy-decoding failure or packet corruption is asserted.

[Standards quote, production lines, and provenance](../../evidence.md#dev-80e8727274)

</details>
