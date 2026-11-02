# JPEG 2000: tested deviations summary

Source: [`microsoft/jpeg2000_rs@69fe0bce9024`](https://github.com/microsoft/jpeg2000_rs/tree/69fe0bce9024ab5a4cb6556e3ebdf91134324148).
Production code was not modified.

This folder is self-contained for review: actual test sources, native traces,
build recipes, portable reproduction archives and upstream licenses are included.
No live DeepSpec database or copyrighted standards document is included.

## Independently reproduce from this repository

Requirements: Linux, Python 3.12+, Git, Bash/sh, `patch`, and Rust/Cargo 1.93.1
through rustup. Allow several GiB of temporary build space. Cargo may download
the normal locked dependencies on first use; no model service is contacted.

From the repository root:

```sh
rustup toolchain install 1.93.1 --profile minimal
python3 doc_deviations/verify.py --check
python3 doc_deviations/verify.py --all --output ../jpeg2000-deviation-results
```

`--check` verifies all five archive hashes and checks the unpacked evidence
against the sealed originals. `--all` runs every packaged finding, including the
separately qualified U1, sequentially in disposable source trees. It does not
modify production files in your checkout. The aggregate `report.json`, child
reports, build logs and complete native output are saved in the new output
directory. A zero exit means **the specified failures reproduced**, not that the
decoder passed conformance tests. Missing cases, unexpected passes, different
failures, build errors and timeouts cannot produce success.

For a smaller selection:

```sh
python3 doc_deviations/verify.py --finding F31 --output ../vbas-results
python3 doc_deviations/verify.py --finding T1 --finding T2 --output ../transform-results
python3 -m unittest discover -s doc_deviations -p test_verify.py
```

Use a new output directory for each run. The Git checkout must contain commit
`69fe0bce9024ab5a4cb6556e3ebdf91134324148`; a shallow clone may require fetching
that commit first. Reproduction intentionally uses that revision, not arbitrary
local changes or a newer decoder revision. Inspect the linked test assertions,
fixtures, registration patches and recipes to evaluate the oracles independently.
These scripts execute native builds/tests and are **not a sandbox**.

The complete entry point was independently rerun on November 2, 2026:
all five components reproduced their expected failures. See the
[fresh aggregate report](verification/report.json) and
[verification notes](verification/README.md) for the complete logs and counting
distinctions. All eight entry-point regression checks pass.

## Review results and limits

**33 curated test-backed review rows, plus one separately qualified unit witness.**
These are not 33 certified defects or universal MUST violations. The 92 failed
generated cases also include overlapping observations, candidates and
specification/test-attribution problems; those are not silently promoted here.

> **Edition warning:** our tests use T.800 (2019), T.801 (2021) and T.808 (2022),
> while the repository targets their 2024, 2023 and 2026 editions respectively.
> Their differences remain unreconciled. Native failures are reproduced;
> applicability to the newer editions is not established.
> T.814 (2019) and T.803 (2024) match the selected reference editions.

Each evidence cell links to the **actual test source** as well as the native
failure trace. F09/F24 retain their test implementations in patches, explicitly
labelled below. F28 links its assertion helper and the patch that calls it from
the existing test. Test modules can contain additional tests; the package's
`recipe.json` records the exact selected command. F11 links both its original
parser test and the additional ingestion test. U1 links the test body included
inside an existing native test, plus its registration/build recipe.

The transform source is a byte-for-byte extraction from the sealed archive
(SHA-256 `79fa0341d0aa7c64dbe38805139743dcb473a892165156a31f566aaec7047ce5`).
T1 is `wave86_arbitrary_odd_origin_standard_witness`; T2 is
`wave86_irreversible_singleton_standard_witness`. The archive contains their
`native-replay.log`. Detailed applicability notes remain in the package tables.

## Standards-behavior discrepancies (23 rows)

| ID | Observed discrepancy | Tested document / clause | Code | Test source / native trace |
|---|---|---|---|---|
| F01 | Tile-wide nonlinear-transform override loses to an inherited main-header component setting. | T.801 (2021), A.3.10 | [codestream.rs:1801-1835][c01] | [Test source][s01] / [Trace][t01] |
| F02 | Valid PLM packet-length marker rejected: parser expects a four-byte group count instead of one byte. | T.800 (2019), A.7.2, Tables A.35-A.36 | [codestream.rs:9168-9210][c02] | [Test source][s02] / [Trace][t02] |
| F03 | Reversible decorrelation arrays rejected: resolver requires N squared coefficients instead of N times (N+1). | T.801 (2021), J.3.1.1.3 | [codestream.rs:7676-7735][c03] | [Test source][s03] / [Trace][t03] |
| F04 | Consistent PLM and PLT packet-length descriptions work separately but are rejected together. | T.800 (2019), A.7.2-A.7.3 | [codestream.rs:9998-10010][c04] | [Test source][s04] / [Trace][t04] |
| F05 | Inverse nonlinear LUT uses the wrong normalization endpoint; descending LUT returns 16 instead of 0. | T.801 (2021), A.3.10; K.2.2 | [image.rs:1634-1662][c05] | [Test source][s05] / [Trace][t05] |
| F06 | Selected-color context omits a predecessor sample needed for component registration. Rendered-color corruption is not demonstrated. | T.800 (2019), A.9.1 | [decode.rs:4176-4195][c06] | [Test source][s06] / [Trace][t06] |
| F07 | Unspecified-color JPH incorrectly requires a color box even for component-plane output. | T.814 (2019), D.2-D.3 | [jp2.rs:2276-2278][c07] | [Test source][s07] / [Trace][t07] |
| F08 | A later color box that should be ignored causes rejection after a valid first color box. | T.800 (2019), I.5.3.3 | [jp2.rs:2199-2207][c08] | [Test source][s08] / [Trace][t08] |
| F09 | Color associations are treated as physical channel indices; valid associations 2 and 3 are rejected for a one-channel record. | T.800 (2019), I.5.3.6 | [refinement.rs:1049-1057][c09] | [Test patch][s09] / [Trace][t09] |
| F10 | A permitted additional contiguous JP2 codestream causes the whole file to be rejected. | T.800 (2019), I.2.2; I.5.4 | [jp2.rs:977-1090][c10] | [Test source][s10] / [Trace][t10] |
| F11 | First dependent JPIP message lacks initial Class=0 / CSn=0 defaults and is rejected. Bare ordered Annex A input, not an unreliable-transport packet. | T.808 (2022), A.2.1 | [jpip.rs:262-277][c11] | [Test source][s11] / [ingestion test][s11b] / [Parser failure][t11]; [ingestion failure][t11b] |
| F12 | Optional SOD marker is treated as mandatory in a complete JPP tile-header bin. Direct normalization witness, not full image decoding. | T.808 (2022), A.3.3 | [jpip.rs:4341-4348][c12] | [Test source][s12] / [Trace][t12] |
| F13 | Metadata fragments split into separate bins when CSn changes, although metadata identity should ignore it within one logical target. | T.808 (2022), A.3.6.1 | [refinement.rs:2410-2421][c13] | [Test source][s13] / [Trace][t13] |
| F14 | Empty clipped subband produces a zero-width code-block grid that later packet construction rejects. | T.800 (2019), B.6 | [decode.rs:2008-2046][c14] | [Test source][s14] / [Shared geometry test][t14] |
| F15 | Empty reduced resolutions acquire phantom precincts; shifted 1x1 geometry yields four mappings instead of one. | T.800 (2019), B.6 / B-16; T.808 (2022), A.3.2.1 | [decode.rs:491-497][c15] | [Test source][s15] / [Same test as F14][t14] |
| F16 | Stream-equivalent box contents retain the original xref header/type instead of the replacement header. | T.808 (2022), A.3.6.2-A.3.6.3 | [jpip.rs:2190-2210][c16] | [Test source][s16] / [Trace][t16] |
| F17 | Reserved class identifiers 3 and 7 are assigned ordinary tile/main-header meanings without a negotiated extension. | T.808 (2022), A.2.2, Table A.2 | [jpip.rs:33-44][c17] | [Test source][s17] / [Trace][t17] |
| F18 | Single-stream placeholder rejects a permitted NCS count field instead of consuming it and treating the count as one. | T.808 (2022), A.3.6.3, Table A.3 | [jpip.rs:3831-3865][c18] | [Test source][s18] / [Trace][t18] |
| F29 | Allowing SOP markers incorrectly makes one mandatory; a complete empty packet without optional SOP is rejected. | T.800 (2019), A.8.1 | [packet.rs:3989-4009][c29] | [Test source][s29] / [Trace][t29] |
| F30 | Terminal stuffed header byte is omitted from boundary accounting, so a valid following EPH marker is rejected. Tier-1 body decoding is not exercised. | T.800 (2019), B.10.1; A.8.2 | [packet.rs:4173-4194][c30] | [Test source][s30] / [Trace][t30] |
| F31 | Valid leading-zero VBAS encodings rejected: padded Class8 and length1 cause `InvalidJpipVbas` in complete independent-message parser fixtures. | T.808 (2022), A.2.1, Figures A.2-A.3 | [jpip.rs:361-389][c31] | [Test source][s31] / [Trace][t31] |
| T1 | Odd-origin arbitrary lifting selects the wrong neighbour: expected `[118,208,168]`, observed `[218,308,268]`. Production-decoded ATK kernel; even-origin control passes. | T.801 (2021), H.2.5.1 / H-3; H.2.4.3 | [transform.rs:846-905][ct1] | [Test source][st1] / [Transform archive][transforms] |
| T2 | Odd-origin irreversible singleton is not halved: Q16 sample 131072 remains 131072 instead of 65536. | T.800 (2019), F.3.6, p.120 | [transform.rs:461-469][ct2] | [Test source][st2] / [Transform archive][transforms] |

T1/T2 are summary labels for the two existing transform witnesses, not new
findings or additional executions. Both are bounded API witnesses, not complete
encoded-image reproductions. Native/model agreement on unchanged singletons
does not invalidate the independently standard-based T2 observation.

## Structural-validation gaps (7 rows)

These establish a mismatch with the cited structure and assessed validator
contract, **not** a universal requirement that every permissive reader reject
every nonconforming input.

| ID | Observed discrepancy | Tested document / clause | Code | Test source / native trace |
|---|---|---|---|---|
| F19 | Invalid nonzero tile-part length underflows into the "unknown length" sentinel reserved for Psot=0. | T.800 (2019), A.4.2; project transition documentation | [jpip.rs:4026-4043][c19] | [Test source][s19] / [Trace][t19] |
| F20 | Ordinary Part 1 COC accepts zero precinct exponents above the lowest resolution. Does not generalize to unresolved Part 2 directional cases. | T.800 (2019), A.6.1-A.6.2, Tables A.15/A.21 | [codestream.rs:8747-8749][c20] | [Test source][s20] / [Trace][t20] |
| F21 | Association box accepts zero or one immediate child instead of at least two. | T.801 (2021), M.11.11 | [jp2.rs:1342-1377][c21] | [Test source][s21] / [Trace][t21] |
| F22 | Mixed layer-registration presence accepted: one layer has creg while another does not. | T.801 (2021), M.11.7.7 | [jp2.rs:1919-1961][c22] | [Test source][s22] / [Trace][t22] |
| F23 | Palette accepts more than 1024 entries; smallest negative fixture uses 1025. | T.800 (2019), I.5.3.4 | [jp2.rs:2535-2550][c23] | [Test source][s23] / [Trace][t23] |
| F24 | JPX accepts fewer codestream-header boxes than codestreams after some headers are supplied. | T.801 (2021), M.11.6 | [jp2.rs:1194-1219][c24] | [Test patch][s24] / [Trace][t24] |
| F25 | Duplicate top-level signature and File Type boxes escape uniqueness validation. | T.800 (2019), I.5.1-I.5.2 | [jp2.rs:977-1090][c10] | [Test source][s25] / [Signature failure][t25a]; [File Type failure][t25b] |

## Documented project behavior (2 rows)

| ID | Observed discrepancy | Authority | Code | Test source / native trace |
|---|---|---|---|---|
| F26 | Explicit-plan output admits non-LRCP tile overrides outside its documented profile. Not a universal JPEG prohibition; no pixel corruption established. | [SUPPORTED.md:138-141][supported] | [jpip.rs:3740-3760][c26] | [Test source][s26] / [Trace][t26] |
| F27 | Reduced decoding chooses the wrong tile's nonlinear transform by mixing full-resolution and reduced coordinates; native sample is 128 instead of 64. | [README.md:81-83][readme]; tile-local NLT semantics | [decode.rs:3856-3872][c27] | [Test source][s27] / [Trace][t27] |

## SHOULD recommendation (1 row)

| ID | Observed discrepancy | Tested document / clause | Code | Test source / native trace |
|---|---|---|---|---|
| F28 | Zero-life nonpersistent instruction is painted rather than skipped: opaque gray instead of transparency. Normalized rendering fixture, not a public encoded-file route. | T.801 (2021), M.5.3.2.1; **SHOULD, not SHALL** | [jpx.rs:818-875][c28] | [Test assertions][s28] / [registration][s28-registration] / [Trace][t28] |

## Separately qualified observation (not included in the 33 rows)

| ID | Observed discrepancy | Tested document / clause | Code | Test source / trace / limitation |
|---|---|---|---|---|
| U1 | Non-initial U decoding omits the extension: expected `[75,1]`, observed `[35,1]`. | T.814 (2019), 7.3.4; 7.3.6, Table 3 / equation (3) | [tier1.rs:2484-2501][cu1] | [Test body][su1] / [recipe][su1-recipe] / [Trace][tu1]; [applicability notes][u-notes]. Isolated U-field fixture; complete high-magnitude codestream/public image reachability not established. |

U1 is not promoted into the confirmed Core family count. Its narrower original
prefix/suffix clause received a test-error attribution; that does not erase the
separate extension-aware observation. It must not be generalized to profiles
whose magnitude bounds guarantee extension-free coding.

## Reproduce and inspect the complete records

| Package | Detailed table / scope notes | Instructions and downloadable evidence |
|---|---|---|
| F01-F27 | [Original findings](jpeg2000-review-package-20261029/FINDINGS.md) | [README](jpeg2000-review-package-20261029/README.md) / [archive](jpeg2000-review-package-20261029.tar.gz) |
| F28-F30 | [Supplement findings](jpeg2000-review-supplement-20261030/FINDINGS.md) | [README](jpeg2000-review-supplement-20261030/README.md) / [archive](jpeg2000-review-supplement-20261030.tar.gz) |
| F31 and additional F11 wrapper | [Final JPIP findings](jpeg2000-final-jpip-witnesses-20261030/FINDINGS.md) | [README](jpeg2000-final-jpip-witnesses-20261030/README.md) / [archive](jpeg2000-final-jpip-witnesses-20261030.tar.gz) |
| T1-T2 | `WITNESSES.md` inside archive | [Source, tests and native log][transforms] |
| Qualified U1 | [Witness and scope][u-notes] | [Archive](jpeg2000-u-extension-witness-20261030.tar.gz) |

For the original, supplement, final-JPIP or U1 package, from its directory:

```sh
python3 reproduce.py --repo /path/to/jpeg2000_rs --all --output ./reproduction-results
```

The local repository must contain the pinned commit. The runner needs Python
3.12+, Rust/Cargo 1.93.1 and the package's documented prerequisites, but no
DeepSpec, SOMA or standards database. It succeeds only when the expected native
failure and its specific diagnostics reproduce. Builds, unrelated failures and
timeouts do not count.

For T1/T2, extract the transform archive and follow its `WITNESSES.md`.
Native regressions intentionally fail; they are not passing conformance tests.
Original packages and their checksums have not been modified by this summary.

**Counting:** F14/F15 share one test, F25 has two backing cases, and the added
F11 ingestion wrapper is not a new root. Rows, tests, clauses and independent
root causes are different counts. No complete Verus proof or certification is
claimed. See the [final run index](jpeg2000-review-index-20261030.md) for all
execution totals, two explicit coverage gaps and remaining review limitations.

[c01]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/codestream.rs#L1801-L1835
[c02]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/codestream.rs#L9168-L9210
[c03]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/codestream.rs#L7676-L7735
[c04]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/codestream.rs#L9998-L10010
[c05]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/image.rs#L1634-L1662
[c06]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/decode.rs#L4176-L4195
[c07]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L2276-L2278
[c08]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L2199-L2207
[c09]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/refinement.rs#L1049-L1057
[c10]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L977-L1090
[c11]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L262-L277
[c12]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L4341-L4348
[c13]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/refinement.rs#L2410-L2421
[c14]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/decode.rs#L2008-L2046
[c15]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/decode.rs#L491-L497
[c16]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L2190-L2210
[c17]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L33-L44
[c18]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L3831-L3865
[c19]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L4026-L4043
[c20]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/codestream.rs#L8747-L8749
[c21]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L1342-L1377
[c22]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L1919-L1961
[c23]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L2535-L2550
[c24]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jp2.rs#L1194-L1219
[c26]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L3740-L3760
[c27]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/decode.rs#L3856-L3872
[c28]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpx.rs#L818-L875
[c29]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/packet.rs#L3989-L4009
[c30]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/packet.rs#L4173-L4194
[c31]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L361-L389
[ct1]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/transform.rs#L846-L905
[ct2]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/transform.rs#L461-L469
[cu1]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/tier1.rs#L2484-L2501
[supported]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/SUPPORTED.md#L138-L141
[readme]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/README.md#L81-L83

[t01]: jpeg2000-review-package-20261029/cases/2bbb4b7f6d9814081dc6d341/efad0ebad5f0e363d0ab2122.log
[t02]: jpeg2000-review-package-20261029/cases/c7779586ba7e5308becdb3ab/b062a512e7267bca1f14bad8.log
[t03]: jpeg2000-review-package-20261029/cases/a352b8922cbf51067edebe9c/87e93cf11efec2e559bb6034.log
[t04]: jpeg2000-review-package-20261029/cases/9a82786057472396f5b9ef42/db69a93bf86d084071e11986.log
[t05]: jpeg2000-review-package-20261029/cases/19e89c8564c4bcac4d2588b1/43212f9b85589508f443ca6f.log
[t06]: jpeg2000-review-package-20261029/cases/166619668a5d7e448c6c7ed7/cc6701332d6429318af1632c.log
[t07]: jpeg2000-review-package-20261029/cases/dcdc31654724c4c2ad030fd2/debd9f4cb2047d9570f27b9b.log
[t08]: jpeg2000-review-package-20261029/cases/56c3811ca88c040d23aa7d2a/349f62632c2048a190657687.log
[t09]: jpeg2000-review-package-20261029/cases/d558f26e6e40b685f1a86c0c/93c8c78793c85a4f283ead83.log
[t10]: jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/e68962b8c90cb4b6836ef844.log
[t11]: jpeg2000-review-package-20261029/cases/5419dc777157dc2a298419c7/a17a60fedfd5f588b3c6f6a3.log
[t11b]: jpeg2000-final-jpip-witnesses-20261030/cases/66aff522bc300198b9d75ec3/recorded-test.log
[t12]: jpeg2000-review-package-20261029/cases/944e3d6310a427b8a34ed222/75d55d0e82420b19700d15de.log
[t13]: jpeg2000-review-package-20261029/cases/baf1d4fa2691d414510aeaae/6430a2e679802b9144ea5a4b.log
[t14]: jpeg2000-review-package-20261029/cases/8a33f3c4299ccaa7ac8de80c/1b6ab2a4ad1f14f5081e6afd.log
[t16]: jpeg2000-review-package-20261029/cases/e1f075d7b5416d4f472c0f9a/9c7d6b44332022f336f2ca0d.log
[t17]: jpeg2000-review-package-20261029/cases/baf1d4fa2691d414510aeaae/99fed806685c9122fb266211.log
[t18]: jpeg2000-review-package-20261029/cases/afef8cd7d6537e8a08814419/9105d99750798402cec08c7f.log
[t19]: jpeg2000-review-package-20261029/cases/32cc62dbf63c6da1321051c0/294dc3cab57bd4b0e4d6345f.log
[t20]: jpeg2000-review-package-20261029/cases/7b34b23231926ec82a67e1e9/13da61704101aa4758fec233.log
[t21]: jpeg2000-review-package-20261029/cases/9970988c331d1d0c2b443c1d/31f9c6f9ee4357015c058216.log
[t22]: jpeg2000-review-package-20261029/cases/bb705c76759f74c8112ad3a5/e62d0f8ba0be84c76f97c921.log
[t23]: jpeg2000-review-package-20261029/cases/b22776d76bcd039a5c524749/85c71c4469785571f2d66fe3.log
[t24]: jpeg2000-review-package-20261029/cases/111437184a2d565c3ed13b32/09f93dd85547f3089d8c40f4.log
[t25a]: jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/68574d962e014b599022f579.log
[t25b]: jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/1732b85cde6699859e0f568a.log
[t26]: jpeg2000-review-package-20261029/cases/0825d2151cc175fbae2c4912/46e0d3178e7a407a7247f7b8.log
[t27]: jpeg2000-review-package-20261029/cases/12db5111ad29d4dc5a0ff9bb/817c898abc85ba6ef483648d.log
[t28]: jpeg2000-review-supplement-20261030/cases/d18e7664ef908ecf323e5b74/e4ed0a3af2c9d0dbd08711cc.log
[t29]: jpeg2000-review-supplement-20261030/cases/f71c07d698d8441f7f243d58/44c55ec8f76ec6b24e477888.log
[t30]: jpeg2000-review-supplement-20261030/cases/f68e289bc92b358b513e6f20/23cf463d2324619732b2286f.log
[t31]: jpeg2000-final-jpip-witnesses-20261030/cases/55730ec94ef7342e41f0ebc5/recorded-test.log
[tu1]: jpeg2000-u-extension-witness-20261030/cases/f4c99d87482bdb3bb429c765/recorded-test.log
[u-notes]: jpeg2000-u-extension-witness-20261030/README.md
[transforms]: jpeg2000-transform-witnesses-20261030.tar.gz

[s01]: jpeg2000-review-package-20261029/cases/2bbb4b7f6d9814081dc6d341/artifacts/validation/nonlinear_lookup_contract.rs
[s02]: jpeg2000-review-package-20261029/cases/c7779586ba7e5308becdb3ab/artifacts/src/codestream/tests/standard_plm.rs
[s03]: jpeg2000-review-package-20261029/cases/a352b8922cbf51067edebe9c/artifacts/src/codestream/resolve_mct_contract.rs
[s04]: jpeg2000-review-package-20261029/cases/9a82786057472396f5b9ef42/artifacts/src/codestream/read_tile_part_contract.rs
[s05]: jpeg2000-review-package-20261029/cases/19e89c8564c4bcac4d2588b1/artifacts/src/typed_layout_contract_tests.rs
[s06]: jpeg2000-review-package-20261029/cases/166619668a5d7e448c6c7ed7/artifacts/src/decode_crg_tests.rs
[s07]: jpeg2000-review-package-20261029/cases/dcdc31654724c4c2ad030fd2/artifacts/src/decode/tests/jph_contract.rs
[s08]: jpeg2000-review-package-20261029/cases/56c3811ca88c040d23aa7d2a/artifacts/src/jp2_header_spec_tests.rs
[s09]: jpeg2000-review-package-20261029/cases/d558f26e6e40b685f1a86c0c/artifacts/cdef-spec-tests.patch
[s10]: jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/artifacts/src/jp2/spec_tests.rs
[s11]: jpeg2000-review-package-20261029/cases/5419dc777157dc2a298419c7/artifacts/src/jpip_header_context_contract_tests.rs
[s11b]: jpeg2000-final-jpip-witnesses-20261030/cases/66aff522bc300198b9d75ec3/artifacts/src/jpip_ingest_contract_tests.rs
[s12]: jpeg2000-review-package-20261029/cases/944e3d6310a427b8a34ed222/artifacts/src/jpip_normalization_contract_tests.rs
[s13]: jpeg2000-review-package-20261029/cases/baf1d4fa2691d414510aeaae/artifacts/src/jpip_databin_contract_tests.rs
[s14]: jpeg2000-review-package-20261029/cases/8a33f3c4299ccaa7ac8de80c/artifacts/src/jpip_precinct_map_contract.rs
[s15]: jpeg2000-review-package-20261029/cases/8a33f3c4299ccaa7ac8de80c/artifacts/src/jpip_precinct_map_contract.rs
[s16]: jpeg2000-review-package-20261029/cases/e1f075d7b5416d4f472c0f9a/artifacts/src/jpip_metadata_contract_tests.rs
[s17]: jpeg2000-review-package-20261029/cases/baf1d4fa2691d414510aeaae/artifacts/src/jpip_databin_contract_tests.rs
[s18]: jpeg2000-review-package-20261029/cases/afef8cd7d6537e8a08814419/artifacts/src/jpip_placeholder_contract_tests.rs
[s19]: jpeg2000-review-package-20261029/cases/32cc62dbf63c6da1321051c0/artifacts/src/jpip_transition_contract_tests.rs
[s20]: jpeg2000-review-package-20261029/cases/7b34b23231926ec82a67e1e9/artifacts/src/codestream/tests/coc_clause_tests.rs
[s21]: jpeg2000-review-package-20261029/cases/9970988c331d1d0c2b443c1d/artifacts/src/jp2_metadata_cardinality_test.rs
[s22]: jpeg2000-review-package-20261029/cases/bb705c76759f74c8112ad3a5/artifacts/src/jp2_layer_reference_tests.rs
[s23]: jpeg2000-review-package-20261029/cases/b22776d76bcd039a5c524749/artifacts/src/jp2_palette_contract_tests.rs
[s24]: jpeg2000-review-package-20261029/cases/111437184a2d565c3ed13b32/artifacts/deepspec_codestream.patch
[s25]: jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/artifacts/src/jp2/spec_tests.rs
[s26]: jpeg2000-review-package-20261029/cases/0825d2151cc175fbae2c4912/artifacts/src/jpip/transcode_contract_tests.rs
[s27]: jpeg2000-review-package-20261029/cases/12db5111ad29d4dc5a0ff9bb/artifacts/src/decode_selected_contract_tests.rs
[s28]: jpeg2000-review-supplement-20261030/cases/d18e7664ef908ecf323e5b74/artifacts/src/jpx_frame_semantics_tests.rs
[s28-registration]: jpeg2000-review-supplement-20261030/cases/d18e7664ef908ecf323e5b74/artifacts/frame-semantics.patch
[s29]: jpeg2000-review-supplement-20261030/cases/f71c07d698d8441f7f243d58/artifacts/src/packet_progression_contract_tests.rs
[s30]: jpeg2000-review-supplement-20261030/cases/f68e289bc92b358b513e6f20/artifacts/src/packet_jpip_contract_tests.rs
[s31]: jpeg2000-final-jpip-witnesses-20261030/cases/55730ec94ef7342e41f0ebc5/artifacts/validation/jpip_vbas_contract.rs
[st1]: jpeg2000-review-test-sources/wave86_transform_witnesses.rs
[st2]: jpeg2000-review-test-sources/wave86_transform_witnesses.rs
[su1]: jpeg2000-u-extension-witness-20261030/cases/f4c99d87482bdb3bb429c765/artifacts/src/tier1_noninitial_u_contract.rs
[su1-recipe]: jpeg2000-u-extension-witness-20261030/cases/f4c99d87482bdb3bb429c765/recipe.json
