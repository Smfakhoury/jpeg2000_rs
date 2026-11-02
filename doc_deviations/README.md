# JPEG 2000 tested deviations

| Item | Value |
|---|---|
| Source | [`microsoft/jpeg2000_rs@69fe0bce9024`](https://github.com/microsoft/jpeg2000_rs/tree/69fe0bce9024ab5a4cb6556e3ebdf91134324148) |
| Evidence | Native Rust tests; production code unchanged |
| Included | 30 test-backed standards findings |
| Excluded | Project-profile findings F26-F27; non-mandatory recommendation F28; qualified observation U1 |
| Edition Evaluated | Tests cite T.800 (2019), T.801 (2021), T.808 (2022), and T.814 (2019). The repository selects T.800 (2024), T.801 (2023), T.808 (2026), and T.814 (2019); newer-edition reconciliation is incomplete. |

| ID | Class | Standard | Required behavior | Test input / assertion | Observed behavior | Production code | Test source |
|---|---|---|---|---|---|---|---|
| F01 | Behavior | [T.801 (2021), A.3.10][t801] | A tile-header COD nonlinear transform overrides inherited main-header component settings for the tile. | Supply conflicting inherited component and tile-wide NLT settings; assert the tile-wide setting is selected. | The inherited component setting wins. | [codestream.rs:1801-1835][c01] | [Rust test][s01] |
| F02 | Behavior | [T.800 (2019), A.7.2, Tables A.35-A.36][t800] | `Nplm` is a one-byte packet-length group count. | Parse a valid PLM marker with a one-byte `Nplm`; assert successful packet-length recovery. | The parser expects four bytes for the count and rejects the marker. | [codestream.rs:9168-9210][c02] | [Rust test][s02] |
| F03 | Behavior | [T.801 (2021), J.3.1.1.3][t801] | A reversible N-component decorrelation array contains N times (N+1) coefficients. | Resolve a reversible MCT array with the standard coefficient count; assert acceptance. | The resolver requires N squared coefficients and rejects the array. | [codestream.rs:7676-7735][c03] | [Rust test][s03] |
| F04 | Behavior | [T.800 (2019), A.7.2-A.7.3][t800] | Consistent PLM and PLT packet-length descriptions may coexist. | Supply matching PLM and PLT lengths; assert the tile-part is accepted. | The combined descriptions are rejected although each works independently. | [codestream.rs:9998-10010][c04] | [Rust test][s04] |
| F05 | Behavior | [T.801 (2021), A.3.10; K.2.2][t801] | Inverse nonlinear LUT normalization includes the correct descending endpoint. | Apply a descending LUT endpoint; assert output 0. | Output is 16. | [image.rs:1634-1662][c05] | [Rust test][s05] |
| F06 | Behavior | [T.800 (2019), A.9.1][t800] | Component registration uses the required predecessor sample in selected-color context construction. | Build a selected-color registration context that requires the predecessor; assert it is included. | The predecessor sample is omitted. | [decode.rs:4176-4195][c06] | [Rust test][s06] |
| F07 | Behavior | [T.814 (2019), D.2-D.3][t814] | Unspecified-color JPH component-plane output does not require a color box. | Decode component planes from a JPH file without `colr`; assert success. | Decoding fails because a color box is required. | [jp2.rs:2276-2278][c07] | [Rust test][s07] |
| F08 | Behavior | [T.800 (2019), I.5.3.3][t800] | After a valid first color box, later color boxes that do not apply are ignored. | Supply a valid first `colr` followed by an ignorable `colr`; assert the first remains authoritative. | The later color box causes rejection. | [jp2.rs:2199-2207][c08] | [Rust test][s08] |
| F09 | Behavior | [T.800 (2019), I.5.3.6][t800] | Color associations are association values, not physical channel indices. | Use one-channel CDEF records with associations 2 and 3; assert both are accepted. | Both are rejected as out-of-range channel indices. | [refinement.rs:1049-1057][c09] | [Test patch][s09] |
| F10 | Behavior | [T.800 (2019), I.2.2; I.5.4][t800] | Additional contiguous JP2 codestream boxes are permitted. | Parse a JP2 file containing a permitted second contiguous codestream; assert success. | The complete file is rejected. | [jp2.rs:977-1090][c10] | [Rust test][s10] |
| F11 | Behavior | [T.808 (2022), A.2.1][t808] | The first dependent JPIP message defaults Class and CSn to zero. | Parse and ingest `[30,00,01,00]`; assert Class=0 and CSn=0 defaults. | The message is rejected because no initial defaults are installed. | [jpip.rs:262-277][c11] | [Parser test][s11] / [ingestion test][s11b] |
| F12 | Behavior | [T.808 (2022), A.3.3][t808] | SOD is optional in a complete JPP tile-header bin. | Normalize a complete tile-header bin without SOD; assert success. | Normalization rejects the bin because SOD is treated as mandatory. | [jpip.rs:4341-4348][c12] | [Rust test][s12] |
| F13 | Behavior | [T.808 (2022), A.3.6.1][t808] | Metadata-bin identity within one logical target is independent of CSn. | Ingest fragments that differ only by CSn; assert one metadata bin. | The fragments are split into separate bins. | [refinement.rs:2410-2421][c13] | [Rust test][s13] |
| F14 | Behavior | [T.800 (2019), B.6][t800] | A clipped empty subband produces no invalid code-block grid. | Construct clipped empty-subband geometry; assert packet construction remains valid. | A zero-width code-block grid is created and later rejected. | [decode.rs:2008-2046][c14] | [Shared geometry test][s14] |
| F15 | Behavior | [T.800 (2019), B.6/B-16][t800]; [T.808 (2022), A.3.2.1][t808] | Empty reduced resolutions do not create precincts; shifted 1x1 geometry maps once. | Evaluate shifted 1x1 reduced geometry; assert one precinct mapping. | Four mappings are produced. | [decode.rs:491-497][c15] | [Shared geometry test][s15] |
| F16 | Behavior | [T.808 (2022), A.3.6.2-A.3.6.3][t808] | Stream-equivalent replacement contents use the replacement box header and type. | Resolve an xref replacement; assert the resulting box has the replacement header/type. | The original xref header/type is retained. | [jpip.rs:2190-2210][c16] | [Rust test][s16] |
| F17 | Behavior | [T.808 (2022), A.2.2, Table A.2][t808] | Class identifiers 3 and 7 remain reserved unless an extension assigns them. | Parse class IDs 3 and 7 without an extension; assert no ordinary databin meaning is assigned. | They are assigned ordinary tile/main-header meanings. | [jpip.rs:33-44][c17] | [Rust test][s17] |
| F18 | Behavior | [T.808 (2022), A.3.6.3, Table A.3][t808] | A single-stream placeholder may carry NCS; the field is consumed and the stream count is one. | Parse a single-stream placeholder containing NCS; assert consumption and count=1. | The permitted NCS field is rejected. | [jpip.rs:3831-3865][c18] | [Rust test][s18] |
| F19 | Validation | [T.800 (2019), A.4.2][t800] | Only Psot=0 denotes unknown tile-part length; invalid nonzero lengths must not become that sentinel. | Supply an invalid nonzero tile-part length; assert rejection rather than unknown-length handling. | Arithmetic underflow converts it to the Psot=0 sentinel. | [jpip.rs:4026-4043][c19] | [Rust test][s19] |
| F20 | Validation | [T.800 (2019), A.6.1-A.6.2, Tables A.15/A.21][t800] | Part 1 COC precinct exponents above the lowest resolution must be nonzero. | Supply zero exponents above resolution zero; assert rejection. | The invalid COC is accepted. | [codestream.rs:8747-8749][c20] | [Rust test][s20] |
| F21 | Validation | [T.801 (2021), M.11.11][t801] | An association box contains at least two immediate children. | Parse association boxes with zero and one child; assert rejection. | Both invalid cardinalities are accepted. | [jp2.rs:1342-1377][c21] | [Rust test][s21] |
| F22 | Validation | [T.801 (2021), M.11.7.7][t801] | Layer-registration presence is consistent across compositing layers. | Supply one layer with `creg` and one without; assert rejection. | The mixed configuration is accepted. | [jp2.rs:1919-1961][c22] | [Rust test][s22] |
| F23 | Validation | [T.800 (2019), I.5.3.4][t800] | A palette contains at most 1024 entries. | Supply 1025 entries; assert rejection. | The oversized palette is accepted. | [jp2.rs:2535-2550][c23] | [Rust test][s23] |
| F24 | Validation | [T.801 (2021), M.11.6][t801] | Once codestream-header boxes are supplied, their count covers the codestreams. | Supply more codestreams than codestream-header boxes; assert rejection. | The incomplete header set is accepted. | [jp2.rs:1194-1219][c24] | [Test patch][s24] |
| F25 | Validation | [T.800 (2019), I.5.1-I.5.2][t800] | Signature and File Type top-level boxes are unique. | Independently duplicate each box; assert both files are rejected. | Both duplicate-box files are accepted. | [jp2.rs:977-1090][c10] | [Rust test][s25] |
| F29 | Behavior | [T.800 (2019), A.8.1][t800] | SOP remains optional when SOP markers are allowed. | Parse a complete empty packet with SOP enabled but absent; assert success. | The packet is rejected because SOP is treated as mandatory. | [packet.rs:3989-4009][c29] | [Rust test][s29] |
| F30 | Behavior | [T.800 (2019), B.10.1; A.8.2][t800] | A terminal stuffed header byte participates in boundary accounting before EPH. | Parse a stuffed terminal header byte followed by valid EPH; assert both boundaries are accepted. | The stuffed byte is omitted from accounting and EPH is rejected. | [packet.rs:4173-4194][c30] | [Rust test][s30] |
| F31 | Behavior | [T.808 (2022), A.2.1, Figures A.2-A.3][t808] | Leading-zero VBAS encodings are valid and preserve following bytes. | Parse `60 80 08 00 00 01 ab 5a` and `60 08 00 00 80 01 ab 5a`; assert Class=8, CSn=0, bin=0, offset=0, body=`ab`, sentinel=`5a` unconsumed. | Both fixtures fail with `InvalidJpipVbas`. | [jpip.rs:361-389][c31] | [Rust test][s31] |
| T1 | Behavior | [T.801 (2021), H.2.5.1/H-3; H.2.4.3][t801] | Odd-origin arbitrary lifting selects the standard neighbour; even-origin behavior remains unchanged. | Run odd-origin and even-origin controls; assert odd output `[118,208,168]`. | Odd output is `[218,308,268]`; the even-origin control passes. | [transform.rs:846-905][ct1] | [Rust test][st1] |
| T2 | Behavior | [T.800 (2019), F.3.6][t800] | An odd-origin irreversible singleton is halved. | Transform a one-sample Q16 value 131072; assert 65536. | The value remains 131072. | [transform.rs:461-469][ct2] | [Rust test][st2] |

[t800]: https://www.itu.int/rec/T-REC-T.800
[t801]: https://www.itu.int/rec/T-REC-T.801
[t808]: https://www.itu.int/rec/T-REC-T.808
[t814]: https://www.itu.int/rec/T-REC-T.814

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
[c29]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/packet.rs#L3989-L4009
[c30]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/packet.rs#L4173-L4194
[c31]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L361-L389
[ct1]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/transform.rs#L846-L905
[ct2]: https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/transform.rs#L461-L469

[s01]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/2bbb4b7f6d9814081dc6d341/artifacts/validation/nonlinear_lookup_contract.rs
[s02]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/c7779586ba7e5308becdb3ab/artifacts/src/codestream/tests/standard_plm.rs
[s03]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/a352b8922cbf51067edebe9c/artifacts/src/codestream/resolve_mct_contract.rs
[s04]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/9a82786057472396f5b9ef42/artifacts/src/codestream/read_tile_part_contract.rs
[s05]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/19e89c8564c4bcac4d2588b1/artifacts/src/typed_layout_contract_tests.rs
[s06]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/166619668a5d7e448c6c7ed7/artifacts/src/decode_crg_tests.rs
[s07]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/dcdc31654724c4c2ad030fd2/artifacts/src/decode/tests/jph_contract.rs
[s08]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/56c3811ca88c040d23aa7d2a/artifacts/src/jp2_header_spec_tests.rs
[s09]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/d558f26e6e40b685f1a86c0c/artifacts/cdef-spec-tests.patch
[s10]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/artifacts/src/jp2/spec_tests.rs
[s11]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/5419dc777157dc2a298419c7/artifacts/src/jpip_header_context_contract_tests.rs
[s11b]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-final-jpip-witnesses-20261030/cases/66aff522bc300198b9d75ec3/artifacts/src/jpip_ingest_contract_tests.rs
[s12]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/944e3d6310a427b8a34ed222/artifacts/src/jpip_normalization_contract_tests.rs
[s13]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/baf1d4fa2691d414510aeaae/artifacts/src/jpip_databin_contract_tests.rs
[s14]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/8a33f3c4299ccaa7ac8de80c/artifacts/src/jpip_precinct_map_contract.rs
[s15]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/8a33f3c4299ccaa7ac8de80c/artifacts/src/jpip_precinct_map_contract.rs
[s16]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/e1f075d7b5416d4f472c0f9a/artifacts/src/jpip_metadata_contract_tests.rs
[s17]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/baf1d4fa2691d414510aeaae/artifacts/src/jpip_databin_contract_tests.rs
[s18]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/afef8cd7d6537e8a08814419/artifacts/src/jpip_placeholder_contract_tests.rs
[s19]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/32cc62dbf63c6da1321051c0/artifacts/src/jpip_transition_contract_tests.rs
[s20]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/7b34b23231926ec82a67e1e9/artifacts/src/codestream/tests/coc_clause_tests.rs
[s21]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/9970988c331d1d0c2b443c1d/artifacts/src/jp2_metadata_cardinality_test.rs
[s22]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/bb705c76759f74c8112ad3a5/artifacts/src/jp2_layer_reference_tests.rs
[s23]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/b22776d76bcd039a5c524749/artifacts/src/jp2_palette_contract_tests.rs
[s24]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/111437184a2d565c3ed13b32/artifacts/deepspec_codestream.patch
[s25]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-package-20261029/cases/ea8d9c3e31baf68e430c41aa/artifacts/src/jp2/spec_tests.rs
[s29]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-supplement-20261030/cases/f71c07d698d8441f7f243d58/artifacts/src/packet_progression_contract_tests.rs
[s30]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-supplement-20261030/cases/f68e289bc92b358b513e6f20/artifacts/src/packet_jpip_contract_tests.rs
[s31]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-final-jpip-witnesses-20261030/cases/55730ec94ef7342e41f0ebc5/artifacts/validation/jpip_vbas_contract.rs
[st1]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-test-sources/wave86_transform_witnesses.rs#L42-L90
[st2]: https://github.com/microsoft/jpeg2000_rs/blob/97e293a4ac0cb28cfdd6d56283fc34e319cc8cc6/doc_deviations/jpeg2000-review-test-sources/wave86_transform_witnesses.rs#L93-L121
