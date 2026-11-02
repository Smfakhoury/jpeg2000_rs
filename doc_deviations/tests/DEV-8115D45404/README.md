# Automatic JPIP reconstruction rejects complete legal tile-header bins without optional SOD

`DEV-8115D45404` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.3

**Required behavior:** Automatically normalize received JPIP header bins into reconstructed decoder inputs without imposing the explicit caller-planned header preparation contract.

**Observed deviation:** Automatic JPIP reconstruction rejects complete legal tile-header bins without optional SOD

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-8115D45404 --output rfc-results/DEV-8115D45404
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib jpip::tests::snapshot_spec_received_header_normalization -- --exact --nocapture
```

### Test input and recorded failure

All state is constructed with validated public constructors and cache insertion, without private-field mutation. Main header is SOC/SIZ/COD/QCD for one pixel, tile, component and resolution, Rsiz=0, supported progression and positive bounded layers. Received class-2 header is explicitly final and complete, excludes SOT and omits only SOD, which pinned T.808 A.3.3 makes optional; the empty-bin case is expressly permitted. One-byte zero packets are legal empty layer packets. Codestream id and tile/precinct ids are zero, selection is None/all-components, resource limits are default and comfortably exceed the entire setup. No explicit-plan framing contract applies to this automatic entry.

**Recorded failure** (matched by the replay runner):

```text
legal complete JPP header rejected: layers=1 progression=2 comment=false SOD=false: malformed input at byte 0: MissingRequiredStructure
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_snapshot_spec_tests.rs:L12-L13`](files/src/jpip_snapshot_spec_tests.rs#L12-L13)

```text
cache.insert(fragment(id(6, 0), 0, main, true)).unwrap();
    cache.insert(fragment(id(2, 0), 0, header, true)).unwrap();
```

[`src/jpip_snapshot_spec_tests.rs:L17-L18`](files/src/jpip_snapshot_spec_tests.rs#L17-L18)

```text
assert!(cache.is_complete(id(6, 0)));
    assert!(cache.is_complete(id(2, 0)));
```

[`src/jpip_snapshot_spec_tests.rs:L23-L25`](files/src/jpip_snapshot_spec_tests.rs#L23-L25)

```text
cache.build_automatic_snapshot(
        0, None, &JpipPacketSelection::all_components(), &ResourceLimits::default(),
    )
```

[`src/jpip_snapshot_spec_tests.rs:L143-L147`](files/src/jpip_snapshot_spec_tests.rs#L143-L147)

```text
for include_sod in [true, false] {
                    let mut header = markers.clone();
                    if include_sod {
                        header.extend_from_slice(&[0xff, 0x93]);
                    }
```

[`src/jpip_snapshot_spec_tests.rs:L149-L152`](files/src/jpip_snapshot_spec_tests.rs#L149-L152)

```text
let cache = snapshot_spec_cache(
                        &original[..sot], &header, &vec![0; usize::from(layers)],
                    );
                    match snapshot_spec_call(&cache) {
```

[`src/jpip_snapshot_spec_tests.rs:L169-L171`](files/src/jpip_snapshot_spec_tests.rs#L169-L171)

```text
Err(error) => failures.push(format!(
                            "legal complete JPP header rejected: layers={layers} progression={progression} comment={with_comment} SOD={include_sod}: {error}"
                        )),
```

[`src/jpip_snapshot_spec_tests.rs:L177-L177`](files/src/jpip_snapshot_spec_tests.rs#L177)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_snapshot_spec_tests.rs`](files/src/jpip_snapshot_spec_tests.rs)
- [`snapshot-tests.patch`](files/snapshot-tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i snapshot-tests.patch
```

```sh
python3 -c 'from pathlib import Path; assert '"'"'include!("jpip_snapshot_spec_tests.rs");'"'"' in Path('"'"'src/jpip.rs'"'"').read_text(); s=Path('"'"'src/jpip_snapshot_spec_tests.rs'"'"').read_text(); assert all('"'"'fn '"'"'+n+'"'"'()'"'"' in s for n in ['"'"'snapshot_spec_reconstructed_lengths'"'"','"'"'snapshot_spec_received_header_normalization'"'"','"'"'snapshot_spec_unavailable_headers'"'"'])'
```

```sh
cargo test --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the final assert!(failures.is_empty(), "{}", failures.join("\n")) in snapshot_spec_received_header_normalization, after snapshot_spec_call returned MissingRequiredStructure for SOD-less headers. This is a target-operation failure, not a setup failure or an inference from exit 101. The replay tail explicitly preserves multiple per-case errors, including the one-layer/progression-2/empty-header witness quoted below; its beginning is truncated, so I do not claim the chronologically first individual rejection is visible. The aggregate assertion and surviving individual messages identify the failed check sufficiently.

Entry realism was checked first: each new variant has a 1x1 image, one tile, one 8-bit component, no decompositions, one precinct, 1/2/7 layers and one zero-byte empty packet per layer. Its three bins contain a 65-byte SOC/SIZ/COD/QCD main header, 0/2/8/10 tile-header bytes and at most 7 packet bytes: at most 82 cached payload bytes in three fragments, and at most 96 reconstructed bytes. The 60 variants run sequentially. ResourceLimits::default is HARDENED, allowing 2,147,483,648 input bytes, 256 layers, 16,384 tiles, 64 components, 67,108,864 packets and 1,048,576 JPIP fragments. There is no excessive recursion, stack fixture, or implicit capacity stress. These are smaller versions of ordinary received bins; the real JpipAutomaticProgressiveDecoder::update_inner forwards the cache, codestream, selection, packet selection and options limits directly to this function without imposing SOD framing.

This clause has rfc_map origin, not infer origin. I read its parent finding and independently retrieved the pinned T.808 (12/2022), ISO/IEC 15444-9:2023, A.3.3 passage via read_rfc_section using origin item c3e124e770eb767b4dca0e91. It says tile-header bins shall not contain SOT, 'Inclusion of SOD markers is optional', and a server must send a tile-header bin even when empty. These fixtures meet that precise JPP applicability: class-2 tile-zero headers exclude SOT; main framing is complete Part 1 Rsiz=0; coding parameters are inherited from the main header; optional binary COM is legal; empty packets are inline. Empty headers are actually received and marked complete, not inferred from absence. The source documentation also explicitly assigns normalization to automatic reconstruction, unlike explicit caller-planned transcodes. The newer catalog edition is not established by this investigation; the finding is grounded in the cited pinned edition and documented API, not an assertion of exhaustive standards conformance.

Route: test-only inclusion inside the existing tests module exposes the private unit without production changes. snapshot_spec_cache uses DatabinId::new, JpipFragment::new and DatabinCache::insert, marking main and tile headers final and asserting is_complete. snapshot_spec_call invokes build_automatic_snapshot(0, None, all_components, default limits). Both header completeness gates pass; siz_cardinality returns one tile/one component, and main normalization accepts the main marker sequence. parse_tile_part_transitions accepts the SOT-free empty or COM-only input as one transition. normalize_automatic_tile_headers synthesizes SOT, starts found_sod=false, scans zero or one complete segments, then rejects solely because no optional SOD was found. The return precedes its own header.extend_from_slice(&[0xff, 0x93]), which could supply output framing. Compiler describe_declaration confirms the entry signature; its control_flow_graph shows the normalizer at bb83, Result branching at bb85/bb86 and error propagation at bb88. Thus the legal received input is rejected before template/packet reconstruction; the positive SOD-present counterparts do not accumulate errors. This violates automatic normalization for legal received headers, rather than a stronger incidental oracle about progression or output bytes. No callee entry requirement demands SOD: the stored normalizer summary describing rejection is an inferred output behavior, explicitly not a caller-prevalidation requirement, and cannot override A.3.3. Direct contract-valid unit testing is permitted here; no owning caller or private state invariant was bypassed.

[Standards quote, production lines, and provenance](../../evidence.md#dev-8115d45404)

</details>
