# Fresh JPIP parser rejects standard inherited headers instead of applying initial zero defaults

`DEV-F1E3AE04A3` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Accept the documented standard inherited and explicit binary message-header forms within the bounded JPIP profile.

**Observed deviation:** Fresh JPIP parser rejects standard inherited headers instead of applying initial zero defaults

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-F1E3AE04A3 --output rfc-results/DEV-F1E3AE04A3
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 deepspec_jpip_replay.py run jpip::tests::deepspec_standard_header_forms
```

### Test input and recorded failure

Public constructor establishes an ordinary valid fresh parser, without private-field mutation or skipped initialization. A.2.1 in the acquired T.808 (12/2022)/ISO 15444-9:2023 text defines absent initial Class and CSn as zero. Form 01 is permitted, in-class=0 is a supported precinct identifier, offset and body length are zero, no auxiliary is required for class zero, and all three wire bytes are present. Three bytes and zero extent are far below default InputBytes=2,147,483,648; no allocation, stack-depth or pool stress is involved. The module promises inherited standard headers and the real response consumer also begins with the same unprimed public constructor. Opaque body framing is the unit contract, not image decoding.

**Recorded failure** (matched by the replay runner):

```text
A.2.1 zero defaults violated:
first inherited [20, 00, 00]: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`deepspec_jpip_parse.patch:L117-L117`](files/deepspec_jpip_parse.patch#L117)

```text
+        let limits = ResourceLimits::default();
```

[`deepspec_jpip_parse.patch:L175-L193`](files/deepspec_jpip_parse.patch#L175-L193)

```text
+        for bin in [0, 1, 15, 16, 127, 128] {
+            for last in [false, true] {
+                let fixture = SpecMessage {
+                    class: 0,
+                    stream: 0,
+                    bin,
+                    offset: 0,
+                    body: Vec::new(),
+                    last,
+                    auxiliary: None,
+                };
+                let (wire, _) = fixture.wire(1);
+                let mut parser = JpipMessageParser::new(limits);
+                match parser.parse(&wire) {
+                    Ok(_) => fixture.assert_parse(&mut JpipMessageParser::new(limits), 1, false),
+                    Err(error) => failures.push(format!("first inherited {wire:02x?}: {error:?}")),
+                }
+            }
+        }
```

[`deepspec_jpip_parse.patch:L194-L198`](files/deepspec_jpip_parse.patch#L194-L198)

```text
+        assert!(
+            failures.is_empty(),
+            "A.2.1 zero defaults violated:\n{}",
+            failures.join("\n")
+        );
```

[`deepspec_jpip_replay.py:L38-L38`](files/deepspec_jpip_replay.py#L38)

```text
["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"]
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`deepspec_jpip_parse.patch`](files/deepspec_jpip_parse.patch)
- [`deepspec_jpip_replay.py`](files/deepspec_jpip_replay.py)

**Build commands** (in order):

```sh
python3 deepspec_jpip_replay.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() in deepspec_standard_header_forms, reporting 'A.2.1 zero defaults violated' and specifically rejecting [0x20,0x00,0x00] with InvalidJpipHeader. This violates clause 43f15ffa053ac75972b21f66, not an implementation-derived acceptance oracle. Its origin is rfc_map finding ffeed0a3342d0e813d9226d596e3080256b9aa9ddff9ae787c5ab31bc04a96f5. I read the actual cited A.2.1 passage through the parent mapping item: T.808 (12/2022), ISO/IEC 15444-9:2023, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc. Table A.1 permits bb=01, omitting Class and CSn; the text explicitly makes each identifier zero when absent and no previous message exists. Msg-Offset and Msg-Length are nonnegative, so offset=length=0 is admissible. This applies to receiving binary headers, which module documentation explicitly supports; no transport, server, complete-image or cache-reassembly precondition is needed for this direct parser unit contract. The edition uncertainty in the mapping remains: this finding establishes the cited 2022/2023 rule, not independent conformance to the catalog's newer 2026 edition.

Entry realism: the failing family contains twelve sequential constructor-based calls, each with a 3- or 4-byte header, no body, zero offset/length, bin identifier at most 128, and no recursion, fragment pool or fabricated context. ResourceLimits::default() is HARDENED with maximum_input_bytes=2,147,483,648. Earlier successful generated cases use small wire buffers and offsets below 65,536, so no implicit resource limit is approached. The real ingest_jpip_response caller likewise constructs JpipMessageParser::new(cache.limits) and immediately passes the first response slice to parse without priming inheritance. This test does not skip an owning caller required by the unit contract.

The replay patch is additive unit-test code and its Python runner builds the normal cargo --locked --lib test binary, selects the exact registered case, and verifies that one test runs. describe_declaration confirms parse directly delegates to parse_jpip_message. The delegate CFG and source show the route: nonempty input passes the whole-slice ceiling; 0x20 is not EOR; form=(0x20>>5)&3=1 is valid; masked read_vbas returns in-class zero; has_explicit_class(1) is false; new() supplied previous_class=None; ok_or_else at src/jpip.rs:265-267 therefore returns InvalidJpipHeader before reading offset or length. parse propagates that error unchanged and the test records it before failing its explicit zero-default assertion. The codestream branch also lacks the initial form-1 default, although that branch is not reached in this observed failure. Consequence: a standard first inherited data header is rejected rather than parsed as class 0, codestream 0. Existing implementation contracts accurately describing this stricter behavior do not override the standards-derived documented contract. The authoritative output exposes both the failing check and rejected wire; attribution does not rely on the runner's claimed Route label or exit code alone.

[Standards quote, production lines, and provenance](../../evidence.md#dev-f1e3ae04a3)

</details>
