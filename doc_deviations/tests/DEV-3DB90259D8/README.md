# cdef parser rejects valid RGB colour associations exceeding physical channel count

`DEV-3DB90259D8` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § I.5.3.6

**Required behavior:** Preserve the distinction between a cdef channel index and its color association: validate the former against output channels and interpret the latter as a color-space index or special association value, without requiring the color index to fit the physical channel count.

**Observed deviation:** cdef parser rejects valid RGB colour associations exceeding physical channel count

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-3DB90259D8 --output rfc-results/DEV-3DB90259D8
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/deepspec-cdef/cdef-tests --exact jp2::tests::spec_cdef_separates_channel_and_colour_domains --nocapture
```

### Test input and recorded failure

Admissible direct unit input: genuine framed cdef BoxRef; N=3 and exact 2+6*N payload; one physical output channel with every Cni=0; supported Color type; RGB associations 1,2,3 each represented exactly once; unsigned 1x1 component, sRGB colr, no cmap/palette/MCT, one cdef after ihdr. The pinned T.800 section expressly permits multiple descriptions for one channel and separates colour indices from physical channel indices. Sizes are far below default resource limits. There is no callee precondition requiring the association predicate to return true, and no owning-caller-only state is needed by this pure parser boundary. This is not a claim of end-to-end file reachability or complete-file conformance.

**Recorded failure** (matched by the replay runner):

```text
channels=1: RGB colour indices 1,2,3 must be retained independently of channel count: Err(MalformedInput { offset: 110, kind: InvalidBoxContents })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`cdef-spec-tests.patch:L156-L162`](files/cdef-spec-tests.patch#L156-L162)

```text
+        for channels in [1_u16, 2, 3, 4, 8, 64] {
+            // Sharing all RGB colours makes cdef necessary even with one channel.
+            let mut descriptions = vec![
+                (0, ChannelType::Color, ChannelAssociation::Color(1)),
+                (0, ChannelType::Color, ChannelAssociation::Color(2)),
+                (0, ChannelType::Color, ChannelAssociation::Color(3)),
+            ];
```

[`cdef-spec-tests.patch:L41-L47`](files/cdef-spec-tests.patch#L41-L47)

```text
+        let mut header = box32(*b"ihdr", &ihdr);
+        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
+        header.extend(box32(*b"cdef", &payload));
+        let boxes = BoxReader::new(&header, &ResourceLimits::default())
+            .unwrap()
+            .collect::<Result<Vec<_>, _>>()
+            .unwrap();
```

[`cdef-spec-tests.patch:L60-L60`](files/cdef-spec-tests.patch#L60)

```text
+        parse_channel_definition(boxes[2], channels, 73)
```

[`cdef-spec-tests.patch:L225-L225`](files/cdef-spec-tests.patch#L225)

```text
+        assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`cdef-spec-tests.patch`](files/cdef-spec-tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i cdef-spec-tests.patch
```

```sh
sh -eu -c 'grep -Fq '"'"'fn spec_cdef_retains_every_admitted_description()'"'"' src/jp2.rs; grep -Fq '"'"'fn spec_cdef_separates_channel_and_colour_domains()'"'"' src/jp2.rs; grep -Fq '"'"'failures.join("\n")'"'"' src/jp2.rs; cargo test --lib --locked --no-run --target-dir target/deepspec-cdef; set -- $(find target/deepspec-cdef/debug/deps -maxdepth 1 -type f -name '"'"'jpeg2000_rs-*'"'"' -executable); test "$#" -eq 1; cp "$1" target/deepspec-cdef/cdef-tests; target/deepspec-cdef/cdef-tests --list | grep -Fx '"'"'jp2::tests::spec_cdef_retains_every_admitted_description: test'"'"'; target/deepspec-cdef/cdef-tests --list | grep -Fx '"'"'jp2::tests::spec_cdef_separates_channel_and_colour_domains: test'"'"''
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the accumulated channels=1 exact-record expectation: the parser returned InvalidBoxContents at offset 110 instead of retaining channel 0's RGB associations 1, 2 and 3. This is a valid direct unit-contract entry, not an invalid fixture or a bypass of a required owning caller. Resource realism: the first witness has a 1x1 unsigned 8-bit image, one physical component, N=3, a 20-byte cdef payload and 65-byte header payload. The failing case's largest fixture has 64 components, N=67 and a 449-byte header payload, with only three flat boxes and heap-backed vectors. Default limits are HARDENED: 64 components, 16,777,216 metadata bytes, 2,147,483,648 input bytes and depth 32. No excessive allocation, stack nesting or platform limit explains this failure. The separate passing retention case's N=65535 is not needed for this diagnosis.

The rfc_map origin was assessed using its parent finding and the complete pinned T.800 (06/2019) I.5.3.6 section, read through the origin item; its returned version matches SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e. It explicitly permits multiple descriptions of one channel, defines RGB associations 1/2/3 as colour rather than channel indices, and requires every colour to be represented. The witness has exactly (0,0,1), (0,0,2), (0,0,3): every physical channel and required RGB colour is described, with unique defined type/association pairs. No palette, opacity or multiple-component transform is involved. The explicit cdef is necessary for this shared-channel mapping. This finding is conditional on that supplied edition and does not certify newer editions or full-file decoding.

The replay patch adds only tests and constructs real BoxReader-produced BoxRefs after ihdr and sRGB colr, then invokes the original parse_channel_definition with the actual physical channel count. Real parse_jp2_header callers likewise pass image.components when cmap is absent; the unit test establishes that input without needing a complete codestream. Compiler declaration confirms the three-argument unit boundary. MIR bb59 calls channel_definition_valid and bb60 takes its false edge through bb62-bb68 to InvalidBoxContents. The first record (0,0,1) passes, but (0,0,2) fails solely because 2 > output_channels=1. The error offset is base 73 plus cdef offset 37 = 110, matching the observed output. The callee accepts every u16 tuple as an input; its false result is not a caller-precondition violation. Its inferred/native model merely describes the same implementation restriction and cannot override the standards-derived clause. Consequently valid metadata is rejected before association conversion/retention. The accumulated assertion has weaker per-record diagnostics, but its exact error and the traced branch identify this deviation; this is not attribution based on exit status alone.

[Standards quote, production lines, and provenance](../../evidence.md#dev-3db90259d8)

</details>
