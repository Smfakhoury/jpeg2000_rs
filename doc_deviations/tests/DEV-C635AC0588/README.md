# Indexed JPX composition paints a zero-LIFE nonpersistent instruction that should be skipped

`DEV-C635AC0588` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § M.5.3.2.1

**Required behavior:** JPX frame inspection and indexed rendering preserve the frame-closing persistence and timing semantics for the collected composition instructions.

**Observed deviation:** Indexed JPX composition paints a zero-LIFE nonpersistent instruction that should be skipped

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-C635AC0588 --output rfc-results/DEV-C635AC0588
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 -c 'import os, pathlib, subprocess; name='"'"'jpx::tests::timeline_expands_repetition_reuse_and_timing_deterministically'"'"'; bins=[p for p in pathlib.Path('"'"'target/debug/deps'"'"').glob('"'"'jpeg2000_rs-*'"'"') if p.is_file() and os.access(p,os.X_OK)]; assert len(bins)==1, bins; exe=str(bins[0]); listed=subprocess.check_output([exe,'"'"'--list'"'"','"'"'--exact'"'"',name],text=True); assert name+'"'"': test'"'"' in listed, listed; os.execv(exe,[exe,'"'"'--exact'"'"',name,'"'"'--nocapture'"'"'])'
```

### Test input and recorded failure

Valid direct normalized unit entry, not claimed public-byte reachability: four layers and four in-bounds disjoint 1x1 placements on a 4x1 canvas; sequential layer assignment, no reuse/crop, one unrepeated instruction set, tick=1, loop_count=1, two requested in-range frames. build_timeline owns creation of all inspected frame objects. Supplied layer keys cover every assignment, each grayscale layer has exactly one u16 sample and alpha=None denotes opacity. Fresh supplied banks are used per render. Default limits and the explicit two-frame builder bound suffice. LIFE=0/PERSIST=false is an expressly defined standard special case, not an invalid state. Neither setup nor output depends on altered production behavior.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: T.801 M.5.3.2.1: PERSIST=false/LIFE=0 should perform no action; column 1 must remain transparent, not display the skipped thumbnail
  left: ([(0, 17, false), (17, 5, true)], [1000, 2000, 3000, 0], [65535, 65535, 65535, 0], [0, 0, 0, 4000], [0, 0, 0, 65535])
 right: ([(0, 17, false), (17, 5, true)], [1000, 0, 3000, 0], [65535, 0, 65535, 0], [0, 0, 0, 4000], [0, 0, 0, 65535])
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`frame-semantics.patch:L16-L16`](files/frame-semantics.patch#L16)

```text
+        assert_frame_closing_semantics();
```

[`src/jpx_frame_semantics_tests.rs:L188-L194`](files/src/jpx_frame_semantics_tests.rs#L188-L194)

```text
    let skipped = closing_semantics_composition(vec![
        closing_semantics_instruction(0, 0, true),
        closing_semantics_instruction(1, 0, false),
        closing_semantics_instruction(2, 17, false),
        closing_semantics_instruction(3, 5, true),
    ]);
    let frames = build_timeline(&skipped, 4, 2).unwrap();
```

[`src/jpx_frame_semantics_tests.rs:L205-L206`](files/src/jpx_frame_semantics_tests.rs#L205-L206)

```text
    let first = closing_semantics_render(&frames, 4, 0).unwrap();
    let next = closing_semantics_render(&frames, 4, 1).unwrap();
```

[`src/jpx_frame_semantics_tests.rs:L45-L51`](files/src/jpx_frame_semantics_tests.rs#L45-L51)

```text
                Some(Ok(OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Grayscale,
                    colors: vec![vec![u16::try_from((index + 1) * 1000).unwrap()]],
                    alpha: None,
                })),
```

[`src/jpx_frame_semantics_tests.rs:L55-L64`](files/src/jpx_frame_semantics_tests.rs#L55-L64)

```text
    render_normalized_frame(
        width,
        1,
        frames,
        1,
        requested,
        bank,
        limits.maximum_decoded_bytes(),
        &limits,
    )
```

[`src/jpx_frame_semantics_tests.rs:L218-L220`](files/src/jpx_frame_semantics_tests.rs#L218-L220)

```text
            vec![(0, 17, false), (17, 5, true)],
            vec![1000, 0, 3000, 0],
            vec![u16::MAX, 0, u16::MAX, 0],
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`frame-semantics.patch`](files/frame-semantics.patch)
- [`src/jpx_frame_semantics_tests.rs`](files/src/jpx_frame_semantics_tests.rs)

**Build commands** (in order):

```sh
patch --batch --forward --fuzz=0 -p0 -i frame-semantics.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpx.rs'"'"').read_text(); t=Path('"'"'src/jpx_frame_semantics_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jpx_frame_semantics_tests.rs");'"'"')==1; assert s.count('"'"'assert_frame_closing_semantics();'"'"')==1; assert '"'"'column 1 must remain transparent'"'"' in t; assert '"'"'for requested in [2, 0, 1, 2, 1, 0]'"'"' in t'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is the final zero-LIFE nonpersistent pixel/alpha comparison at src/jpx_frame_semantics_tests.rs:215, not an earlier metadata assertion or a build failure. Frame 0 contains gray 2000 and alpha 65535 at column 1, where the independent oracle expects transparency. This refutes the standards-derived inspection/indexed-rendering clause e11d683717746f5872e7302c; it does not demonstrate an incorrect field load in persistent() or refute the narrower getter clause 2a438ae4e8c4d1f07241ce8b.

Entry realism was checked before tracing: the failing fixture has four instructions, four opaque 1x1 grayscale layers, a 4x1 canvas, two frames, one instruction set, repeat=0, tick=1, loop_count=1 and finite closing lives 17 and 5. There is no recursive fixture or duration-driven waiting. The preceding 136 sweep fixtures run sequentially, each with at most six layers/pixels and three frames; the maximum finite LIFE is stored numerically, not expanded into that many frames. Defaults are ResourceLimits::HARDENED (256 layers, 4096 frames, 100000000 pixels, 2147483648 decoded bytes, 1000000000 work units). Even the tighter build_timeline limit of two frames in the failing case permits the result, and its instruction bound is 4*2=8 for four instructions. Real inspection calls this same builder with effective_layer_count(file) and limits.maximum_frames(). These are small, representative normalized unit inputs, not an implicit-capacity violation.

Authority is the rfc_map origin ae0ab2a24be5b5f3a4ca7374, including its parent finding. I read the pinned T.801 (06/2021) M.5.3.2.1 text (SHA256 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1). It explicitly says that when PERSIST is false and LIFE is zero, no action should be performed, and explains skipping a thumbnail/print frame. Its adjacent frame-composition rule groups persistent zero-LIFE prefixes and applies the closing instruction's LIFE/PERSIST to the resulting virtual layer. The fixture faithfully tests this exception inside an active composition sequence. This is a SHOULD recommendation deviation, not an unconditional SHALL violation. SUPPORTED.md identifies this 06/2021 text as an implementation reference while leaving reconciliation with its selected 08/2023 consolidation open; this diagnosis is limited to the cited edition and does not claim later-edition or official conformance.

Route: the existing registered timeline regression calls the test-only helper; build_timeline creates valid JpxFrameInfo objects from the supplied Composition, assigning layers 0,1,2,3. apply_timeline_instruction calls append_timeline_frame for each assigned instruction without excluding LIFE=0/PERSIST=false. The initial (0,true) instruction starts frame 0. For the next (0,false) instruction, append's MIR bb5 -> bb6 -> bb17 observes the preceding duration is zero, and bb22 unconditionally appends that instruction to the active frame. The (17,false) instruction closes that same frame; (5,true) starts frame 1. render_normalized_frame selects frame 0; select_frame_instructions includes every instruction of the selected frame, so collect_supplied_layers retrieves the valid opaque layer 1 and rendering paints its column. The later selected frame correctly omits the nonpersistent prior group, matching the observed second image. The getter MIR is only a shared-reference bool copy and correctly reports the stored closing flag.

This is an admissible direct normalized unit-contract test: no JpxFrameInfo is fabricated or mutated, dimensions and samples match, all assigned layer indices have distinct supplied entries, all crops/reuse are absent and requested frames are in range. The patch changes only cfg(test) registration and preserves the original regression. It deliberately does not establish a public encoded-file route, parser flag admission, profile signaling or decoding; those are outside this unit boundary, not prerequisites of these normalized operations. The run's abbreviated satisfies list mentions persistent zero-LIFE prefixes but omits the additional nonpersistent exception; the actual replayed source and precise failure expose that case clearly. No production changes or new replay were needed to attribute this already host-observed failure.

[Standards quote, production lines, and provenance](../../evidence.md#dev-c635ac0588)

</details>
