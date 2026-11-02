# Explicit precinct transcode publishes non-LRCP effective tile coding styles

`DEV-0A9BF676EF` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.4.2

**Required behavior:** Produce the documented single-tile-part quality-prefix transcode using reframed headers rather than retaining stale original packet-length declarations.

**Observed deviation:** Explicit precinct transcode publishes non-LRCP effective tile coding styles

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-0A9BF676EF --output rfc-results/DEV-0A9BF676EF
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/jpip-transcode-suite.py run explicit_profile_rejects_all_non_lrcp_and_attached_sources
```

### Test input and recorded failure

Safe JpipPrecinctCodestreamPlan::new, JpipPrecinctPacket::new, DatabinId::new, JpipFragment::new and DatabinCache::insert establish an admissible public unit-contract entry. No private state is fabricated; private accesses only snapshot state. One layer and one one-byte packet, complete finalized headers, and three bins totaling 104 bytes are far below default limits. The reconstructed stream is 106 bytes, 1x1, one component and one tile-part. parse_layout and independent fixture-premise checks precede the target call and passed under host replay. The deliberately non-LRCP effective tile is the profile violation the routine must reject, not a violated entry requirement.

**Recorded failure** (matched by the replay runner):

```text
profile contract violations:
effective tile progression 1: published output=true, status=Available
effective tile progression 2: published output=true, status=Available
effective tile progression 3: published output=true, status=Available
effective tile progression 4: published output=true, status=Available
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip/transcode_contract_tests.rs:L53-L53`](files/src/jpip/transcode_contract_tests.rs#L53)

```text
    let result = cache.transcode_precinct_codestream(plan, k);
```

[`src/jpip/transcode_contract_tests.rs:L622-L629`](files/src/jpip/transcode_contract_tests.rs#L622-L629)

```text
        let mut tile = t.clone();
        tile[17] = progression;
        variants.push((
            format!("effective tile progression {progression}"),
            h.clone(),
            tile,
            false,
        ));
```

[`src/jpip/transcode_contract_tests.rs:L667-L668`](files/src/jpip/transcode_contract_tests.rs#L667-L668)

```text
        let layout = crate::codestream::parse_layout(&expected, &limits)
            .unwrap_or_else(|e| panic!("profile fixture {name} must parse: {e:?}"));
```

[`src/jpip/transcode_contract_tests.rs:L684-L686`](files/src/jpip/transcode_contract_tests.rs#L684-L686)

```text
        assert_eq!(explicit, eligible, "independent fixture premises {name}");
        let c = complete_cache(&main, &tile, &[0], ResourceLimits::default(), false);
        match call(&c, &p, 1) {
```

[`src/jpip/transcode_contract_tests.rs:L688-L693`](files/src/jpip/transcode_contract_tests.rs#L688-L693)

```text
            Err(e) if !eligible && e == malformed(0, MalformedKind::InvalidJpipLayout) => {}
            Ok(r) => failures.push(format!(
                "{name}: published output={}, status={:?}",
                r.codestream().is_some(),
                r.availability().status()
            )),
```

[`src/jpip/transcode_contract_tests.rs:L697-L701`](files/src/jpip/transcode_contract_tests.rs#L697-L701)

```text
    assert!(
        failures.is_empty(),
        "profile contract violations:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip/transcode_contract_tests.rs`](files/src/jpip/transcode_contract_tests.rs)
- [`validation/jpip-transcode-registration.patch`](files/validation/jpip-transcode-registration.patch)
- [`validation/jpip-transcode-suite.py`](files/validation/jpip-transcode-suite.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/jpip-transcode-registration.patch
```

```sh
python3 validation/jpip-transcode-suite.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is failures.is_empty() at replayed src/jpip/transcode_contract_tests.rs:697, reporting effective tile progression 1..4: published output=true, status=Available. This specifically refutes clause f2c61ee4ddec8a88d2a0a9e7's effective-tile LRCP publication condition, not an arbitrary nonzero exit. clause_origin is infer; SUPPORTED.md independently restricts explicit plans to a single LRCP tile-part, supporting the clause rather than revision to match the missing check. The needs_review label concerns retained dependency-summary gaps and does not invalidate this independently traced profile guarantee.

Entry realism was checked before tracing implementation: each failing variant has one 1x1 image tile, one component, one resolution, one layer, one one-byte packet, three complete bins/fragments totaling 104 stored bytes and 106 reconstructed bytes including EOC. Defaults are HARDENED: 2,147,483,648 input bytes, 256 layers, 67,108,864 packets, 1,048,576 JPIP fragments and 16,384 tiles/tile-parts. There is no deep nesting, oversized allocation or stack demand. The 17 profile variants run sequentially; no capacity-exceeding fixture is involved. The real update_inner caller likewise passes an available layer prefix of its constructed plan; this direct public unit-contract call uses k=1=p.layers() and need not instantiate the progressive decoder.

The fixture constructs plan, IDs, ranges, fragments and cache through safe constructors and insert, not private representation writes. Header-byte edits are legal supplied data: the test deliberately changes the tile COD progression byte to each valid order 1..4 while preserving main LRCP, correct segment framing, one tile-part and the layer count. Before the target call it independently assembles the expected reconstruction, parses it with default limits, checks tile/layer counts and verifies the effective tile is outside the profile. Those premise assertions passed in the host replay. Non-LRCP input is not a forbidden entry precondition: this routine is responsible for rejecting complete out-of-profile reconstructions; the constructor cannot inspect header bytes. A one-layer, one-component, one-resolution, one-precinct empty packet is also consistent across the different progression orders. The test-only registration adds a child test module; the replay build succeeded and production implementation is unchanged.

Compiler declaration/CFG and source establish the route: packet_end_for_layers succeeds; combined availability is Available; header snapshots are patched only for layer counts; the selected byte is copied; Psot and total length are rebuilt; validate_precinct_codestream parses successfully. Its MIR bb13 tests only the MAIN progression, then bb14..bb24 test POC and attached sources, reaching bb27 Ok without inspecting tile coding_style.progression_order. The parser explicitly assigns tile COD to tile_coding_style and publishes it as TilePartPayload.coding_style. The parent MIR bb114 calls this validator and bb117 publishes Some(JpipByteView). Thus the specific missing effective-tile progression guard allows all four host-observed non-LRCP outputs. This is an agent-assessed reproduced public unit-contract deviation, not a formal caller-reachability proof; no downstream entropy-decoding failure or image corruption is claimed.

[Standards quote, production lines, and provenance](../../evidence.md#dev-0a9bf676ef)

</details>
