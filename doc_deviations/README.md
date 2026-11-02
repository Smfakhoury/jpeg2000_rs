# Standards-tested deviations

Source: `jpeg2000-doc-deviations-20261102@1393e11520fa`.

**Read across a row:** requirement → test input → observed deviation → implementation → test. Click its **ID** to reach the expandable reproduction instructions below. You do not need to browse `support/`.

53 reproduced clause-level findings joined to standards mappings (not necessarily distinct defects). A mapping is not by itself a conformance proof.

Requirement/input cells show the opening recorded sentence; the complete text and failure output are preserved in each expandable section.

| ID | Class | Standard | Required behavior | Test input / assertion | Observed deviation (why) | Production code | Test source |
|---|---|---|---|---|---|---|---|
| [DEV-0A9BF676EF](#reproduce-dev-0a9bf676ef) | Code deviation | t800-201906.txt § A.4.2 | Produce the documented single-tile-part quality-prefix transcode using reframed headers rather than retaining stale original packet-length declarations. | Safe JpipPrecinctCodestreamPlan::new, JpipPrecinctPacket::new, DatabinId::new, JpipFragment::new and DatabinCache::insert establish an admissible public unit-contract entry. | Explicit precinct transcode publishes non-LRCP effective tile coding styles | [`jpip::DatabinCache::transcode_precinct_codestream`](../src/jpip.rs#L3088) | [`explicit_profile_rejects_all_non_lrcp_and_attached_sources`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L609) |
| [DEV-926EF1A301](#reproduce-dev-926ef1a301) | Code deviation | t800-201906.txt § A.4.2 | For every SOT-bearing header with SOT offset p and retained header length H (SOT through SOD inclusive), after its framing and earlier marker checks succeed: if its unsigned Psot value L is nonzero and L<H, parsing must return MalformedInput/InvalidTilePart at offset p, not a transition with an unspecified data length. | A safe direct unit-contract invocation supplies a live immutable slice and matching expected_tile. | Nonzero Psot shorter than retained tile-part header is accepted as an unspecified data length | [`jpip::parse_tile_part_transitions`](../src/jpip.rs#L3874) | [`contract_transition_psot_lengths`](support/tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L201) |
| [DEV-64FD2DDB2B](#reproduce-dev-64fd2ddb2b) | Code deviation | t800-201906.txt § A.6.2 | Decode the COC component index using the Csiz-dependent width, reject nonexistent components and malformed ordinary COC field/length encodings, and return the selected component with the parsed component style. | The harness uses ResourceLimits::HARDENED, the existing SIZ fixture (64x32, one component, Rsiz=0), one valid COD with one layer/NL=1/4x4 reversible code blocks and implicit precincts, compatible QCD, one main-header COC for component zero with Scoc=1 and NL=1, no CAP/Part 2 selectors or overrides/duplicates, and a properly framed first tile part. | Ordinary COC accepts zero precinct exponents above resolution zero | [`codestream::Parser::parse_coc`](../src/codestream.rs#L8739) | [Cited check in `coc_clause_tests.rs`](support/tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L357-L362) |
| [DEV-C56978D32B](#reproduce-dev-c56978d32b) | Code deviation | t800-201906.txt § A.7.2 | PLM parsing shall interpret encoded per-tile-part packet-length groups from the main-header marker bytes and preserve their wire order for tile-part consumers. | This is a permitted direct unit-contract test of metadata interpretation. | PLM parser rejects standard one-octet Nplm framing as InvalidMarkerLength | [`codestream::Parser::parse_plm`](../src/codestream.rs#L9168) | [`standard_plm_octet_counts_and_wire_order`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L35) |
| [DEV-4F2CFF1E64](#reproduce-dev-4f2cff1e64) | Code deviation | t800-201906.txt § A.7.2 | Metadata inspection must structurally parse its codestream layout, including supported PLM packet-length metadata, and propagate layout-admission errors rather than return partial success. | The harness uses ResourceLimits::default() and byte constructors, not fabricated Parser internals. | Metadata inspection rejects valid T.800 (06/2019) PLM framing by treating Nplmi as four bytes | [`codestream::parse`](../src/codestream.rs#L3633) | [`metadata_structural_admission_includes_standard_plm`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L220) |
| [DEV-B6570A1D68](#reproduce-dev-b6570a1d68) | Code deviation | t800-201906.txt § A.7.2 | Codestream metadata inspection must include supported PLM/PLT layout admission and preserve layout-parser errors without fabricating successful metadata. | Safe Rust byte vectors and ResourceLimits::default(); no private parser state fabricated. | Metadata inspection rejects standard 8-bit PLM length framing | [`codestream::parse`](../src/codestream.rs#L3633) | [`metadata_plm_plt_admission_and_error_propagation`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L143) |
| [DEV-80E8727274](#reproduce-dev-80e8727274) | Code deviation | t800-201906.txt § A.7.3 | For every input byte slice, requested u16 layer count, and ResourceLimits value, observe a normal return for which parsing the slice with those limits succeeds as L and L.info.image_size.tile_count() succeeds as T. | A safe direct unit-contract entry with real parsed bytes, not a synthetic CodestreamLayout: 96-byte 1x1 single-component, single-tile, single-part stream, one requested and declared layer, default ResourceLimits, valid tile COD override, no progression-change or attached packet-header/length sources. | Precinct validator accepts non-LRCP effective tile COD overrides | [`jpip::validate_precinct_codestream`](../src/jpip.rs#L3740) | [`precinct_contract_structural_eligibility`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L57) |
| [DEV-C36C516466](#reproduce-dev-c36c516466) | Code deviation | t800-201906.txt § A.7.3 | Provide the documented PLM/PLT packet-length support and associated tile-part metadata checks; the implementation's exclusion of combined sources requires reconciliation with that feature intent and T.800 A.7.3. | One component/tile at 1x1, reversible Part1 style, zero decompositions, compatible one-step QCD and initialized component tables; first tile-part with exact Psot and trailing EOC. | Tile-header parser rejects consistent combined PLM and PLT metadata permitted by T.800 A.7.3 | [`codestream::Parser::read_tile_part`](../src/codestream.rs#L9830) | [Cited check in `read_tile_part_contract.rs`](support/tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L52-L60) |
| [DEV-709CD8127B](#reproduce-dev-709cd8127b) | Code deviation | t800-201906.txt § A.8.1 | Progression precinct decoding routes the tile-part coding style and expected tile packet sequence through the documented packet-decoding model, including its SOP-prefix validation policy. | A tiny valid first-packet unit entry: a parsed single-tile Part-1 stream, legal COD Scod=2, one empty header byte 0x00, no EPH or packed source, all initial cursors/counters unchanged at zero, coherent one-block literal precinct, valid component/style and constructor-derived grid/input/progress, default resource limits. | SOP-enabled progression decoding rejects standards-permitted omission of SOP | [`packet::decode_progression_precinct_packet`](../src/packet.rs#L2965) | [`progression_contract_sop_policy_and_optional_marker`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L417) |
| [DEV-EA51ACDA28](#reproduce-dev-ea51acda28) | Code deviation | t800-201906.txt § A.8.2 | Apply the template coding-style EPH policy during inline packet inspection, propagate marker errors, and include a validated EPH in the successful consumed prefix. | A bounded safe-Rust inspection-unit fixture: one fresh packet state, one context and decoder, one valid 1x1 LL subband with fresh inclusion/zero-bitplane trees. | Inline EPH inspection rejects a valid header with terminal FF stuffing | [`packet::decode_jpip_precinct_packet_inner`](../src/packet.rs#L972) | [`jpip_contract_eph_policy_errors_and_prefix`](support/tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L175) |
| [DEV-2F140D1710](#reproduce-dev-2f140d1710) | Code deviation | t800-201906.txt § A.9.1 | Selected-color context preparation shall honor component registration and subsampling when retaining the full-image sampling phase before cropping; registration-bearing metadata from a CRG marker must retain its defined physical offset meaning. | Safe native-resolution direct helper invocation with parser-derived ImageSize, positive sampling, origin zero, a nonempty in-image ROI and a validator-produced tile list [0]. | Native selected-color context excludes the CRG-registered predecessor sample | [`decode::color_selection_from_validated`](../src/decode.rs#L4113) | [`selected_color_context_uses_crg_component_separation`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L60) |
| [DEV-355FE751F9](#reproduce-dev-355fe751f9) | Code deviation | t800-201906.txt § B.1 | Express selected component nonlinear-transform regions as nonempty half-open intersections of tile-component bounds and output extents, translated to output-local sample coordinates. | Safe Rust slices and default options; actual encoded SIZ/COD/QCD/SOT/SOD/NLT bytes, not patched layout or reconstruction state. | Reduced selected decode applies tile-local nonlinear transforms on unreduced tile coordinates | [`decode::decode_codestream_selected`](../src/decode.rs#L3588) | [`selected_contract_geometry_and_native_samples`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L108) |
| [DEV-404E919A62](#reproduce-dev-404e919a62) | Code deviation | t800-201906.txt § F.3.2 | Reconstruct one two-dimensional wavelet level with the supplied arbitrary kernel, using its reversibility-derived precision mode, the level's horizontal and vertical phases and the declared output geometry. | Three ordinary borrowed samples, mathematically correct parity-partitioned dimensions and exact sample lengths, W=1,H=3, and decoded-byte limit 24 satisfy admission. | Arbitrary-filter lifting selects the wrong source neighbour for odd-origin axes | [`transform::inverse_level_arbitrary`](../src/transform.rs#L1274) | [`arbitrary_level_matches_standard_equations_and_rejects_bad_geometry_or_budget`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L118) |
| [DEV-37A5BF80E8](#reproduce-dev-37a5bf80e8) | Code deviation | t800-201906.txt § F.3.5 | The Part 1 vertical-column adapter extracts matching low/high columns at x and returns their phase-sensitive one-dimensional inverse synthesis for placement in that output column. | For extent [1,2), the phase formula produces low_height=0 and high_height=1; both bands have width=1, x=0, exact row-major slice lengths zero and one, respectively. | Part 1 vertical 9/7 synthesis omits odd-origin singleton halving | [`transform::inverse_column_pair`](../src/transform.rs#L1700) | [`part1_column_pair_contract`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L2) |
| [DEV-2D75F3E8F6](#reproduce-dev-2d75f3e8f6) | Code deviation | t800-201906.txt § I.5.1 | JP2 container validation shall recognize the fixed JPEG 2000 signature as the first box and validate the file rule that exactly one such signature exists. | A direct public parser unit test supplies owned, fully framed bytes and a standard immutable ResourceLimits::default(), with no forged internal state or bypassed owning caller. | JP2 validation accepts additional top-level JPEG 2000 signature boxes | [`jp2::parse`](../src/jp2.rs#L977) | [`spec_signature_first_fixed_and_unique`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L64) |
| [DEV-056D9FBFB2](#reproduce-dev-056d9fbfb2) | Code deviation | t800-201906.txt § I.5.2 | JP2 container validation shall require the File Type box immediately after the signature and validate that the file contains exactly one File Type box. | A safe direct call to the actual parse implementation with a fully owned 187-byte buffer and valid default ResourceLimits. | JP2 validation accepts duplicate File Type boxes as opaque metadata | [`jp2::parse`](../src/jp2.rs#L977) | [`spec_file_type_immediate_and_unique`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L100) |
| [DEV-629A0A9C39](#reproduce-dev-629a0a9c39) | Code deviation | t800-201906.txt § I.5.3.3 | In JP2-reader mode, after selecting the first Colour Specification box, ignore later Colour Specification boxes rather than requiring their method-specific semantic parsing to succeed. | The tested entry is the explicitly available direct unit contract, with initialized safe Rust byte vectors and ResourceLimits::default(), not fabricated parser internals. ihdr is 13x17, three components, unsigned depth8, compression7, known colourspace/no-IP flags. | JP2 header rejects reserved-method later colr boxes that the reader must ignore | [`jp2::parse_jp2_header`](../src/jp2.rs#L2139) | [`spec_header_ignores_later_colour_semantics`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L105) |
| [DEV-2032F100C3](#reproduce-dev-2032f100c3) | Code deviation | t800-201906.txt § I.5.3.4 | JP2 palette validation against the local T.800 (06/2019) contract should admit NE only in 1..1024, while preserving the validated entry count and complete table. | The assessed parser unit entry has no requires. | JP2 palette validator accepts NE above the T.800 (06/2019) limit of 1024 | [`jp2::parse_palette`](../src/jp2.rs#L2535) | [`palette_contract_t800_entry_count_domain_and_complete_table`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L147) |
| [DEV-3DB90259D8](#reproduce-dev-3db90259d8) | Code deviation | t800-201906.txt § I.5.3.6 | Preserve the distinction between a cdef channel index and its color association: validate the former against output channels and interpret the latter as a color-space index or special association value, without requiring the color index to fit the physical channel count. | Admissible direct unit input: genuine framed cdef BoxRef; N=3 and exact 2+6*N payload; one physical output channel with every Cni=0; supported Color type; RGB associations 1,2,3 each represented exactly once; unsigned 1x1 component, sRGB colr, no cmap/palette/MCT, one cdef after ihdr. | cdef parser rejects valid RGB colour associations exceeding physical channel count | [`jp2::parse_channel_definition`](../src/jp2.rs#L2755) | [`spec_cdef_separates_channel_and_colour_domains`](support/tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L154) |
| [DEV-457ACAB2C3](#reproduce-dev-457acab2c3) | Code deviation | t800-201906.txt § I.5.4 | In support of ordinary JP2 display, the container reader shall permit later well-framed Contiguous Codestream boxes after the JP2 Header and allow display of the first codestream while ignoring subsequent codestreams, subject to resource admission. | Public parse accepts immutable bytes and a validated ResourceLimits without requiring an owning caller or private object invariants. | Ordinary JP2 rejects a resource-admitted second contiguous codestream instead of allowing first-codestream display | [`jp2::parse`](../src/jp2.rs#L977) | [`spec_later_jp2_codestreams_preserve_first_display`](support/tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L296) |
| [DEV-8825E34C8B](#reproduce-dev-8825e34c8b) | Code deviation | t801-202106.txt § A.3.10 | Select a retained component-specific NLT before a retained default, as documented by the current local model; standards-compliant use must additionally preserve tile-header precedence over main-header selectors. | Parser-created owned receiver; valid component 0 of three; first and only tile-part; unique per-header selectors; bounded native 64x32 fixture; Rsiz=0x8200; legal Tnlt=2 LUT payloads and marker lengths. | Inherited main-header component NLT overrides first-tile-header default | [`codestream::Part2Extensions::nonlinear_transform_for`](../src/codestream.rs#L1801) | [`nonlinear_lookup_standard_header_precedence`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L60) |
| [DEV-49C7BAFDE3](#reproduce-dev-49c7bafde3) | Code deviation | t801-202106.txt § J.3.1.1.3 | Resolve array-backed collections according to the documented graph-collection model, rejecting coefficient vectors whose length differs from the computed expected element count and enforcing reversible precision checks. | A valid direct unit entry, not a claimed complete codestream. | Reversible decorrelation resolver rejects normative N(N+1) coefficient arrays using an N² count | [`codestream::Parser::resolve_mct_collection`](../src/codestream.rs#L7676) | [`resolve_mct_collection_clause_7ca4daea`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L5) |
| [DEV-61C38E5752](#reproduce-dev-61c38e5752) | Code deviation | t801-202106.txt § K | Produce typed decoder output by applying admitted inverse nonlinear transformations after reconstructed samples and inverse component processing, preserving the selected reconstruction's geometry. | A bounded valid direct unit entry, not an entropy-decoding claim: 3 equal four-bit source components, equal subsampling, one tile, matching selected reconstruction bounds and sample counts, ResourceLimits::HARDENED. | Inverse LUT NLT normalizes reconstructed samples by 2^precision instead of 2^precision−1 | [`decode::typed_layout`](../src/decode.rs#L3698) | [`typed_layout_inverse_nlt_after_reconstruction_preserves_geometry`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L289) |
| [DEV-AFC4DEDE80](#reproduce-dev-afc4dede80) | Code deviation | t801-202106.txt § M.11.11 | When validating an Association superbox as JPX metadata, require at least two immediate child boxes before returning a successful structural inspection; recursion and aggregate node counts do not replace immediate-child cardinality. | A direct unit-contract entry has no requires predicates. | JPX Association inspection accepts fewer than two immediate children | [`jp2::inspect_metadata_box`](../src/jp2.rs#L1323) | [`association_requires_two_immediate_children`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L2) |
| [DEV-AACA35161C](#reproduce-dev-aaca35161c) | Code deviation | t801-202106.txt § M.11.6 | For supported top-level JPX codestream representations, header i shall describe codestream i, and container validation shall check that a nonempty header sequence covers every codestream; if there are no headers, JP2 defaults apply to all streams. | This is a contract-valid direct unit call with ordinary immutable borrowed bytes and a valid ResourceLimits value. | JPX container validation accepts a nonempty codestream-header sequence shorter than the codestream sequence | [`jp2::parse`](../src/jp2.rs#L977) | [`spec_jpx_header_bijection_and_defaults`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L379) |
| [DEV-CBF63FCFA1](#reproduce-dev-cbf63fcfa1) | Code deviation | t801-202106.txt § M.11.6 | For admitted JPX files with explicit codestream headers, expose each codestream together with its ordinal header association; nonzero header/codestream cardinality mismatches should not be admitted as a valid association set. | Public byte-parser entry accepts a borrowed byte slice and valid ResourceLimits; it need not assume cardinality-valid input because rejection of that malformed structure is precisely the tested obligation. | JPX parser admits nonzero codestream-header deficits and exposes unassociated codestreams | [`jp2::Jp2File::codestream`](../src/jp2.rs#L653) | [Cited check in `deepspec_codestream.patch`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L14) |
| [DEV-7F4AE3ED02](#reproduce-dev-7f4ae3ed02) | Code deviation | t801-202106.txt § M.11.7.7 | A complete JPX layer-registration structural validation must account for file-wide all-or-none registration-box presence as well as whether each referenced codestream exists. | A direct unit-contract test is available with no requires predicates. | JPX layer-reference validation accepts mixed registration-box presence | [`jp2::validate_layer_references`](../src/jp2.rs#L1919) | [`registration_presence_is_file_wide_and_references_exist`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L90) |
| [DEV-C635AC0588](#reproduce-dev-c635ac0588) | Code deviation | t801-202106.txt § M.5.3.2.1 | JPX frame inspection and indexed rendering preserve the frame-closing persistence and timing semantics for the collected composition instructions. | Valid direct normalized unit entry, not claimed public-byte reachability: four layers and four in-bounds disjoint 1x1 placements on a 4x1 canvas; sequential layer assignment, no reuse/crop, one unrepeated instruction set, tick=1, loop_count=1, two requested in-range frames. build_timeline owns creation of all inspected frame objects. | Indexed JPX composition paints a zero-LIFE nonpersistent instruction that should be skipped | [`jpx::JpxFrameInfo::persistent`](../src/jpx.rs#L169) | [Cited check in `frame-semantics.patch`](support/tests/DEV-C635AC0588/files/frame-semantics.patch#L16) |
| [DEV-9A1ABE441F](#reproduce-dev-9a1abe441f) | Code deviation | t808-202212.txt § A.2.1 | Decode bounded standard binary JPIP header fields by preserving their payload value and field boundary, with checked rejection of unsupported numeric extents. | This is a real eight-byte slice with fresh JpipHeaderContext and default ResourceLimits, not fabricated parser internals. | Bounded standard JPIP VBAS fields with redundant zero groups are rejected as malformed | [`jpip::read_vbas`](../src/jpip.rs#L361) | [`bounded_standard_vbas_contract`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L5) |
| [DEV-A47C95C101](#reproduce-dev-a47c95c101) | Code deviation | t808-202212.txt § A.2.1 | Consume standard binary JPIP message-header forms, including inherited identifiers, using response-local parsing context. | Safe DatabinCache::new(ResourceLimits::default()) and ordinary Rust slices/borrows; no private state mutation. | F11 root overlap: ingestion rejects an initial dependent header instead of applying Annex A identifier defaults | [`jpip::ingest_jpip_response`](../src/jpip.rs#L3504) | [`contract_ingest_header_forms_response_local`](support/tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L180) |
| [DEV-34CC912F98](#reproduce-dev-34cc912f98) | Code deviation | t808-202212.txt § A.2.1 | Create a fresh, independent message-history context for the supported standard inherited and explicit JPIP message-header forms; absence of previous messages must be distinguishable from a previously selected identity. | Public constructor only; no private-state writes, injected faults, or fabricated history. | Fresh JPIP history rejects the standard initial inherited header instead of selecting class 0 and codestream 0 | [`jpip::JpipHeaderContext::new`](../src/jpip.rs#L109) | [`fresh_header_context_preserves_absence_and_independent_standard_history`](support/tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L2) |
| [DEV-30865B7D2D](#reproduce-dev-30865b7d2d) | Code deviation | t808-202212.txt § A.2.1 | Accept the standard binary message-header forms with inherited, explicit-class, or explicit-class-and-codestream identifiers, subject to the bounded parser profile. | The context is constructed by JpipHeaderContext::new(), not fabricated or privately mutated; there is deliberately no previous message. | Fresh JPIP form-01 messages reject standard default class and codestream identifiers | [`jpip::parse_jpip_message`](../src/jpip.rs#L235) | [Cited check in `jpip_contract_tests.rs`](support/tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L354-L361) |
| [DEV-F1E3AE04A3](#reproduce-dev-f1e3ae04a3) | Code deviation | t808-202212.txt § A.2.1 | Accept the documented standard inherited and explicit binary message-header forms within the bounded JPIP profile. | Public constructor establishes an ordinary valid fresh parser, without private-field mutation or skipped initialization. | Fresh JPIP parser rejects standard inherited headers instead of applying initial zero defaults | [`jpip::JpipMessageParser::parse`](../src/jpip.rs#L230) | [`deepspec_standard_header_forms`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L115) |
| [DEV-772E004106](#reproduce-dev-772e004106) | Code deviation | t808-202212.txt § A.2.1 | Consume standard binary message-header forms, including inherited identifiers, using a fresh response-local parsing context. | The caller uses DatabinCache::new(ResourceLimits::default()), DatabinId::new(0,0,0), and safe ingestion only. | Initial standard dependent JPIP header rejected instead of using zero identifier defaults | [`jpip::ingest_jpip_response`](../src/jpip.rs#L3504) | [`contract_ingest_standard_headers_and_response_local_defaults`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L78) |
| [DEV-CA85C0D7B5](#reproduce-dev-ca85c0d7b5) | Code deviation | t808-202212.txt § A.2.1 | The JPIP header context should support the documented inherited-class binary message-header form. | Fresh safe public new() context has no predecessor, exactly the initial-message condition specified by T.808 A.2.1. | Initial dependent JPIP header is rejected instead of applying class-zero default | [`jpip::JpipHeaderContext::previous_class`](../src/jpip.rs#L116) | [Cited check in `jpip_inherited_class_contract.rs`](support/tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L119-L124) |
| [DEV-77C8A9E6C3](#reproduce-dev-77c8a9e6c3) | Code deviation | t808-202212.txt § A.2.2 | The project's bounded JPIP profile classifies accepted message class identifiers into the semantic data-bin class used by parsed identifiers and cache lookup. | No owning-caller fixture is bypassed: this is a direct unit_contract test of a pure scalar function with an unrestricted u64 entry domain. | JPIP classifier assigns standard data-bin semantics to reserved class identifiers 3 and 7 | [`jpip::DatabinClass::from_identifier`](../src/jpip.rs#L33) | [`class_identifiers_follow_t808_table_a2`](support/tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L4) |
| [DEV-E9BA9C12DC](#reproduce-dev-e9ba9c12dc) | Code deviation | t808-202212.txt § A.2.2 | The supported binary-header forms preserve inspectable message identifiers while deterministic reassembly recognizes extended and ordinary precinct messages as addressing the same logical precinct data-bin. | Public JpipMessageParser::new(ResourceLimits::default()) provides a valid fresh context, with neither previous class nor previous codestream. | JPIP parser rejects legal initial inherited-form header instead of defaulting Class and CSn to zero | [`jpip::JpipMessageHeader::databin`](../src/jpip.rs#L137) | [Cited check in `jpip_databin_contract.patch`](support/tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L280-L281) |
| [DEV-2D35E4323C](#reproduce-dev-2d35e4323c) | Code deviation | t808-202212.txt § A.2.2 | When constructing semantic data-bin identities for standard T.808 (12/2022) messages, the defined class values are 0,1,2,4,5,6,8. | A direct public unit-contract entry accepting three by-value u64 arguments and returning Result, with no safety or caller preconditions. | Reserved T.808 (12/2022) classes 3 and 7 receive standard TileHeader and MainHeader semantics | [`jpip::DatabinId::new`](../src/jpip.rs#L62) | [`standard_databin_class_assignment`](support/tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L35) |
| [DEV-7A30EEC46F](#reproduce-dev-7a30eec46f) | Code deviation | t808-202212.txt § A.2.2 | For the supported standard binary-message profile, expose semantic header classes only for assigned message identifiers; reserved signaling is not to be admitted as a supported extension. | Public scalar-only checked-constructor inputs, initially codestream=0 and in_class=0, with no private-state edits, payloads, parser context or downstream plan invariants. | Reserved JPIP class identifiers 3 and 7 admitted as semantic header classes | [`jpip::DatabinId::class`](../src/jpip.rs#L77) | [`standard_assigned_classes_exclude_reserved_signaling`](support/tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L32) |
| [DEV-76BF9F9717](#reproduce-dev-76bf9f9717) | Code deviation | t808-202212.txt § A.3.2.1 | Derive precinct coordinates and JPIP in-class identifiers from SIZ/COD/COC geometry, with resolution-prefix numbering and checked tile/component interleaving. | This is a direct public unit-contract entry with an API-reachable cache, not an internal planner fixture. | JPIP geometric mapping rejects admissible clipped precincts and invents empty-resolution precincts | [`jpip::DatabinCache::derive_precinct_map`](../src/jpip.rs#L1845) | [`derive_precinct_map_matches_standard_geometry_contract`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L258) |
| [DEV-E2DF8B9FAC](#reproduce-dev-e2df8b9fac) | Code deviation | t808-202212.txt § A.3.3 | Provide automatic precinct reconstruction with automatic normalization of the cached headers, as distinct from the explicitly caller-reframed plan path. | Safe-Rust unit test of the actual owning automatic_headers function, with test-only private access and no production behavior changes. | Automatic JPIP header normalization rejects legal SOD-omitted tile-header bins | [`jpip::DatabinCache::automatic_headers`](../src/jpip.rs#L2843) | [`automatic_headers_optional_sod_contract`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L188) |
| [DEV-DFA2D383B6](#reproduce-dev-dfa2d383b6) | Code deviation | t808-202212.txt § A.3.3 | Automatically normalize received JPIP header bins into reconstructed decoder inputs without imposing the explicit caller-planned header preparation contract. | The tested header is 22 actual bytes: a 12-byte SOT with Lsot=10, tile=7, Psot=12, TPsot=0 and TNsot=1; an 8-byte COM segment with valid length and registration; and 2-byte SOD. | Complete tile headers with undersized nonzero Psot are reported Available with unspecified data length | [`jpip::DatabinCache::tile_part_transitions`](../src/jpip.rs#L1912) | [`contract_exact_parse_and_errors`](support/tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L202) |
| [DEV-8115D45404](#reproduce-dev-8115d45404) | Code deviation | t808-202212.txt § A.3.3 | Automatically normalize received JPIP header bins into reconstructed decoder inputs without imposing the explicit caller-planned header preparation contract. | All state is constructed with validated public constructors and cache insertion, without private-field mutation. | Automatic JPIP reconstruction rejects complete legal tile-header bins without optional SOD | [`jpip::DatabinCache::build_automatic_snapshot`](../src/jpip.rs#L2229) | [`snapshot_spec_received_header_normalization`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L119) |
| [DEV-76187B67F4](#reproduce-dev-76187b67f4) | Code deviation | t808-202212.txt § A.3.3 | For the documented automatic reconstruction consumer, accept a complete valid JPP tile-header data-bin without requiring an optional on-wire SOD marker, and synthesize the SOT/SOD framing needed by the derived codestream. | Admissible direct unit-contract input: tile 0, valid empty immutable Rust slice obtained from a complete, present class-2 bin, no SOT, omitted optional SOD, one transition, no malformed marker segments, and ordinary successful cache allocation/insertion. | Automatic tile-header normalization rejects complete JPP bins without optional SOD | [`jpip::normalize_automatic_tile_headers`](../src/jpip.rs#L4269) | [`automatic_normalization_accepts_optional_wire_sod_contract`](support/tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L23) |
| [DEV-EFECF7D00D](#reproduce-dev-efecf7d00d) | Code deviation | t808-202212.txt § A.3.6.1 | Provide deterministic data-bin reassembly and availability/missing-range results for the supported JPIP data-bin classes, so incomplete cached data is not mistaken for successful reconstruction. | An admissible safe-public-API unit entry: a single logical target's root metadata bin 0, two non-overlapping eight-byte fragments of one 16-byte free box, valid explicit class/CSn framing, offsets 0 and 8, final endpoint 16, default resource limits, one checked positive-length 16-byte request. | Metadata-bin availability incorrectly partitions cached bytes by codestream index | [`jpip::DatabinCache::availability`](../src/jpip.rs#L2937) | [Cited check in `jpip_availability_contract_tests.rs`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L322-L326) |
| [DEV-F487B6A88C](#reproduce-dev-f487b6a88c) | Code deviation | t808-202212.txt § A.3.6.1 | For metadata messages belonging to one logical target, constructed class-8 identities used for semantic cache lookup shall identify the metadata bin independently of the message codestream index. | A direct public unit-contract test with no constructor preconditions, supported metadata class 8 and ordinary CSn 0/1, metadata bin 0, one logical target per fresh cache, complete well-framed LBox=12 XML payload, offset 0, final length 12, no auxiliary field, and default resource limits. | Metadata cache identity incorrectly depends on message codestream index | [`jpip::DatabinId::new`](../src/jpip.rs#L62) | [`metadata_databin_identity_ignores_codestream`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L197) |
| [DEV-51871F3EEE](#reproduce-dev-51871f3eee) | Code deviation | t808-202212.txt § A.3.6.1 | JPIP cached-payload accounting supports deterministic sparse data-bin reassembly and duplicate/overlap consistency, including metadata used for placeholder reconstruction; payload identity must follow the supported bin semantics rather than treating an irrelevant metadata codestream field as a new logical bin. | One logical target per cache, metadata class 8 and root ID 0, valid 8/9/15/39-byte free boxes, bounded nonempty spans or legal empty final announcements, consistent overlap bytes and final lengths. | Metadata CSn incorrectly partitions logical bins and double-counts cached payload | [`jpip::DatabinCache::stored_bytes`](../src/jpip.rs#L1429) | [`stored_bytes_obeys_metadata_bin_identity_and_sparse_union`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L2) |
| [DEV-AAF82B7313](#reproduce-dev-aaf82b7313) | Code deviation | t808-202212.txt § A.3.6.1 | Reassemble and resolve the metadata bins referenced by JPIP placeholders for semantic container reconstruction. | One logical target per freshly initialized default-limit cache; four metadata IDs 0..3; valid JP2 root signature/brand, original-content placeholders and independently validated reconstruction; contents-only non-root bins; complementary out-of-order fragments with consistent final lengths; at most 342 transported bytes, eight insertions and depth 2. | Metadata-bin lookup incorrectly partitions one logical target by message codestream index | [`jpip::DatabinCache::expand_metadata_bin`](../src/jpip.rs#L2021) | [`metadata_identity_ignores_message_csn`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L150) |
| [DEV-9D6F9C61DE](#reproduce-dev-9d6f9c61de) | Code deviation | t808-202212.txt § A.3.6.1 | Provide complete metadata-bin views through the documented sparse JPIP cache; for standards-facing class-8 identity within one logical target, ignore the message codestream index. | One fresh public cache represents one logical target. | Class-8 metadata lookup incorrectly includes the codestream index in data-bin identity | [`jpip::DatabinCache::databin_view`](../src/jpip.rs#L1741) | [Cited check in `databin_view_contract_tests.patch`](support/tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L151-L154) |
| [DEV-A64A9EFAD1](#reproduce-dev-a64a9efad1) | Code deviation | t808-202212.txt § A.3.6.1 | Expose outstanding dependencies of the documented metadata-placeholder assembly facility so incomplete data is distinguishable from successful reconstruction. | One cache contains exactly one logical JP2 target; there is no cross-target mixing. | Metadata cache identity incorrectly includes CSn, producing spurious semantic dependencies | [`jpip::JpipContainerAssembly::semantic_needs`](../src/jpip.rs#L1200) | [`semantic_metadata_needs_ignore_metadata_codestream_index`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L149) |
| [DEV-B80F74BB23](#reproduce-dev-b80f74bb23) | Code deviation | t808-202212.txt § A.3.6.3 | Extract incremental codestream identifiers and stream counts from received metadata placeholders for the semantic assembler. | First rejected payload: flags=4, OrigID=7, ordinary OrigBH jp2c length 256, intermediate EquivID=9 and ordinary EquivBH jp2c length 256, CSID=0, present NCS=0. | JPIP single-codestream placeholders reject a permitted present-but-ignored NCS field | [`jpip::parse_jpip_placeholder`](../src/jpip.rs#L3775) | [`placeholder_contract_stream_identifiers_and_counts`](support/tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L89) |
| [DEV-6744E122C7](#reproduce-dev-6744e122c7) | Code deviation | t808-202212.txt § A.3.6.3 | Reconstruct the supported metadata placeholder representation into correctly identified container boxes. | A direct helper unit entry over public-API-established cache state, with a legal compact phld, complete bin 1, complete contents-only bin 2 on the failing phase, CSn 0 throughout, flags=2, ignored unavailable OrigID 99, EquivID 2, original xref header, equivalent colr header, consistent depth/budget and noncyclic ancestor identity [0]. | Stream-equivalent metadata payload is reconstructed with the original box identity | [`jpip::DatabinCache::expand_metadata_bin`](../src/jpip.rs#L2021) | [`stream_equivalent_uses_equivalent_box_identity`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L258) |
| [DEV-174EC5FEE4](#reproduce-dev-174ec5fee4) | Code deviation | t814-201906.txt § D.2 | Decode supported JPH containers to typed component sample planes, preserving component properties independently of the separate explicit color-conversion APIs. | Rust-valid byte slices and default options enter the public API, with no private-state corruption, bypassed owning caller or production modifications. | JPH component decoding incorrectly requires colr when UnkC=1 | [`decode::decode_jph`](../src/decode.rs#L3975) | [`typed_planes_independent_of_colour_conversion`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L120) |

## Reproduce a finding

Run commands from the repository root with its native build dependencies and a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

Each command selects one recipe and builds in a temporary copy; it does not modify your checkout. **Reproduced means the test failed in the recorded way**, not that the underlying test passed. The runner reports `reproduced` and exits 0. Output directories must be new; choose another `--output` path when repeating a run.

<a id="reproduce-dev-0a9bf676ef"></a>
<details>
<summary>DEV-0A9BF676EF — Explicit precinct transcode publishes non-LRCP effective tile coding styles</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-0A9BF676EF --output rfc-results/DEV-0A9BF676EF
```


**Standard:** t800-201906.txt § A.4.2

**Required behavior:** Produce the documented single-tile-part quality-prefix transcode using reframed headers rather than retaining stale original packet-length declarations.

**Observed deviation:** Explicit precinct transcode publishes non-LRCP effective tile coding styles

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

[`src/jpip/transcode_contract_tests.rs:L53-L53`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L53)

```text
    let result = cache.transcode_precinct_codestream(plan, k);
```

[`src/jpip/transcode_contract_tests.rs:L622-L629`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L622-L629)

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

[`src/jpip/transcode_contract_tests.rs:L667-L668`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L667-L668)

```text
        let layout = crate::codestream::parse_layout(&expected, &limits)
            .unwrap_or_else(|e| panic!("profile fixture {name} must parse: {e:?}"));
```

[`src/jpip/transcode_contract_tests.rs:L684-L686`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L684-L686)

```text
        assert_eq!(explicit, eligible, "independent fixture premises {name}");
        let c = complete_cache(&main, &tile, &[0], ResourceLimits::default(), false);
        match call(&c, &p, 1) {
```

[`src/jpip/transcode_contract_tests.rs:L688-L693`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L688-L693)

```text
            Err(e) if !eligible && e == malformed(0, MalformedKind::InvalidJpipLayout) => {}
            Ok(r) => failures.push(format!(
                "{name}: published output={}, status={:?}",
                r.codestream().is_some(),
                r.availability().status()
            )),
```

[`src/jpip/transcode_contract_tests.rs:L697-L701`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L697-L701)

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

- [`src/jpip/transcode_contract_tests.rs`](support/tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs)
- [`validation/jpip-transcode-registration.patch`](support/tests/DEV-0A9BF676EF/files/validation/jpip-transcode-registration.patch)
- [`validation/jpip-transcode-suite.py`](support/tests/DEV-0A9BF676EF/files/validation/jpip-transcode-suite.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/jpip-transcode-registration.patch
```

```sh
python3 validation/jpip-transcode-suite.py build
```

**Exact test invocation:**

```sh
python3 validation/jpip-transcode-suite.py run explicit_profile_rejects_all_non_lrcp_and_attached_sources
```

[Machine-readable recipe](support/tests/DEV-0A9BF676EF/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is failures.is_empty() at replayed src/jpip/transcode_contract_tests.rs:697, reporting effective tile progression 1..4: published output=true, status=Available. This specifically refutes clause f2c61ee4ddec8a88d2a0a9e7's effective-tile LRCP publication condition, not an arbitrary nonzero exit. clause_origin is infer; SUPPORTED.md independently restricts explicit plans to a single LRCP tile-part, supporting the clause rather than revision to match the missing check. The needs_review label concerns retained dependency-summary gaps and does not invalidate this independently traced profile guarantee.

[Standards quote, production lines, and provenance](support/evidence.md#dev-0a9bf676ef)

</details>

</details>

<a id="reproduce-dev-926ef1a301"></a>
<details>
<summary>DEV-926EF1A301 — Nonzero Psot shorter than retained tile-part header is accepted as an unspecified data length</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-926EF1A301 --output rfc-results/DEV-926EF1A301
```


**Standard:** t800-201906.txt § A.4.2

**Required behavior:** For every SOT-bearing header with SOT offset p and retained header length H (SOT through SOD inclusive), after its framing and earlier marker checks succeed: if its unsigned Psot value L is nonzero and L<H, parsing must return MalformedInput/InvalidTilePart at offset p, not a transition with an unspecified data length. On successful return, every such transition with nonzero Psot has data_length=Some(L-H), including Some(0) when L=H. A zero Psot has data_length=None and is legal only for the final header. Thus, in this SOT-bearing representation, None never hides an impossible nonzero declared length. These claims concern original packet-data length, not the number of supplied databin header bytes and not the PLT packet count.

**Observed deviation:** Nonzero Psot shorter than retained tile-part header is accepted as an unspecified data length

A safe direct unit-contract invocation supplies a live immutable slice and matching expected_tile. SOT's length field is 10, so the complete SOT occupies 12 bytes; SOD occupies bytes 12..14. There are no intermediate segments or packet-data requirements. Psot=13 fits usize and is intentionally semantically malformed, not invalid fixture memory: validating this condition belongs to the target, whose entry has no precondition excluding malformed lengths. Part index 5 and total count 0 do not violate this helper's entry requirements. The single 14-byte header and tiny associated vectors are comfortably within resource limits; no production mutation or unsafe access is introduced.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: route=parse_tile_part_transitions(tile=7); input=[ff, 90, 00, 0a, 00, 07, 00, 00, 00, 0d, 05, 00, ff, 93]
  left: Ok([JpipTilePartTransition { tile: 7, tile_part: 5, packet_count: None, data_length: None, functional: false, header: [255, 144, 0, 10, 0, 7, 0, 0, 0, 13, 5, 0, 255, 147] }])
 right: Err(MalformedInput { offset: 0, kind: InvalidTilePart })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_transition_contract_tests.rs:L3-L11`](support/tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L3-L11)

```text
fn contract_part(tile: u16, part: u8, psot: u32, markers: &[u8]) -> Vec<u8> {
    let mut payload = tile.to_be_bytes().to_vec();
    payload.extend_from_slice(&psot.to_be_bytes());
    payload.extend_from_slice(&[part, 0]);
    let mut header = segment(0xff90, &payload);
    header.extend_from_slice(markers);
    header.extend_from_slice(&[0xff, 0x93]);
    header
}
```

[`src/jpip_transition_contract_tests.rs:L234-L245`](support/tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L234-L245)

```text
    for markers in [vec![], segment(0xff64, &[0, 0, 3])] {
        let h = 14 + markers.len();
        for length in [h as u32 - 1, 1] {
            let bad = contract_part(7, 5, length, &markers);
            for prefix in [vec![], contract_part(7, 0, 14, &[])] {
                let offset = prefix.len();
                contract_error(
                    &[prefix, bad.clone()].concat(),
                    7,
                    offset,
                    MalformedKind::InvalidTilePart,
                );
```

[`src/jpip_transition_contract_tests.rs:L50-L60`](support/tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L50-L60)

```text
    let result = parse_tile_part_transitions(input, tile);
    assert_eq!(input, before, "error path mutated input");
    assert_eq!(sentinel, vec![0x31; 97], "error path mutated caller state");
    assert_eq!(
        result,
        Err(DecodeError::MalformedInput {
            offset: offset as u64,
            kind
        }),
        "route=parse_tile_part_transitions(tile={tile}); input={input:02x?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_transition_contract_tests.rs`](support/tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs)
- [`validation/tile_transition_access.patch`](support/tests/DEV-926EF1A301/files/validation/tile_transition_access.patch)
- [`validation/tile_transition_suite.py`](support/tests/DEV-926EF1A301/files/validation/tile_transition_suite.py)

**Build commands** (in order):

```sh
python3 validation/tile_transition_suite.py build
```

**Exact test invocation:**

```sh
python3 validation/tile_transition_suite.py run contract_transition_psot_lengths
```

[Machine-readable recipe](support/tests/DEV-926EF1A301/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failure is the result equality in contract_error at src/jpip_transition_contract_tests.rs:53: a complete 14-byte SOT/SOD header declaring Psot=13 returns Ok with data_length=None instead of Err(MalformedInput { offset: 0, kind: InvalidTilePart }). The earlier input-preservation and sentinel assertions passed. This directly refutes clause 20f954947eed5f28a085ba7e's nonzero L<H rejection, not merely its inferred choice of error category: any successful transition here violates the documented distinction between a nonzero original length and the legal zero-Psot unknown-length form.

[Standards quote, production lines, and provenance](support/evidence.md#dev-926ef1a301)

</details>

</details>

<a id="reproduce-dev-64fd2ddb2b"></a>
<details>
<summary>DEV-64FD2DDB2B — Ordinary COC accepts zero precinct exponents above resolution zero</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-64FD2DDB2B --output rfc-results/DEV-64FD2DDB2B
```


**Standard:** t800-201906.txt § A.6.2

**Required behavior:** Decode the COC component index using the Csiz-dependent width, reject nonexistent components and malformed ordinary COC field/length encodings, and return the selected component with the parsed component style.

**Observed deviation:** Ordinary COC accepts zero precinct exponents above resolution zero

The harness uses ResourceLimits::HARDENED, the existing SIZ fixture (64x32, one component, Rsiz=0), one valid COD with one layer/NL=1/4x4 reversible code blocks and implicit precincts, compatible QCD, one main-header COC for component zero with Scoc=1 and NL=1, no CAP/Part 2 selectors or overrides/duplicates, and a properly framed first tile part. COC payload [0,1,1,0,0,0,1,0x11,0xf0] has exactly two precinct bytes. All entry requirements and limits are respected; only the second precinct's zero width exponent is intentionally malformed, which this parser is required to reject.

**Recorded failure** (matched by the replay runner):

```text
main-header parse_layout accepted illegal high-resolution precinct: [(1, 1), (0, 15)]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/tests/coc_clause_tests.rs:L320-L324`](support/tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L320-L324)

```text
let cod = segment(COD, &[0, 0, 0, 1, 0, 1, 0, 0, 0, 1]);
    let qcd = segment(QCD, &[0, 0, 0, 0, 0]);
    let coc = segment(COC, &[0, 1, 1, 0, 0, 0, 1, 0x11, 0xf0]);
    let bytes = codestream_with_headers(1, &[cod, qcd, coc], &[], &[0, 0]);
    let limits = ResourceLimits::HARDENED;
```

[`src/codestream/tests/coc_clause_tests.rs:L341-L341`](support/tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L341)

```text
match parse_layout(&bytes, &limits) {
```

[`src/codestream/tests/coc_clause_tests.rs:L346-L347`](support/tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L346-L347)

```text
Ok(layout) => failures.push(format!(
            "main-header parse_layout accepted illegal high-resolution precinct: {:?}",
```

[`src/codestream/tests/coc_clause_tests.rs:L357-L362`](support/tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L357-L362)

```text
assert!(
        failures.is_empty(),
        "{} COC contract mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/tests/coc_clause_tests.rs`](support/tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs)
- [`coc_clause_harness.py`](support/tests/DEV-64FD2DDB2B/files/coc_clause_harness.py)

**Build commands** (in order):

```sh
python3 coc_clause_harness.py build
```

**Exact test invocation:**

```sh
python3 coc_clause_harness.py run admission
```

[Machine-readable recipe](support/tests/DEV-64FD2DDB2B/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the admission test's final `failures.is_empty()` assertion, which aggregates mismatches rather than stopping at each comparison. The retained output begins partway through that diagnostic, so its first historical mismatch is unavailable; however, it explicitly records both erroneous Ok results for malformed precinct fields and the within-policy main-header acceptance quoted in observation. This is not an arbitrary exit-101 attribution. Clause af4bc1414e81593a62244981 requires rejecting malformed ordinary COC fields. Its origin is rfc_map, not infer. I read the parent evidence and the assigned T.800 (06/2019) A.6.1 and A.6.2 through read_rfc_section using origin item 8ff9f4815b9913140344f4a4. A.6.2 assigns SPcoc the Table A.15 parameters; A.15 orders precinct entries from NLLL through successive resolutions; Table A.21 states separately for PPx and PPy: `This value may only equal zero at the resolution level corresponding to the NLLL band.` With NL=1, the second entry 0xf0 therefore illegally specifies PPx=0 at resolution 1. This is ordinary Part 1, Rsiz=0, no CAP and no Part 2 decomposition reference, so the parent finding's unresolved directional/Part 2 applicability does not apply. This conclusion is scoped to the assigned 2019 edition; repository newer-edition references remain unreconciled and no broader conformance claim is made. The implementation and stored parse_cod_payload contract preserve zeros, but that correlated model/implementation agreement does not invalidate this standards-derived clause.

[Standards quote, production lines, and provenance](support/evidence.md#dev-64fd2ddb2b)

</details>

</details>

<a id="reproduce-dev-c56978d32b"></a>
<details>
<summary>DEV-C56978D32B — PLM parser rejects standard one-octet Nplm framing as InvalidMarkerLength</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-C56978D32B --output rfc-results/DEV-C56978D32B
```


**Standard:** t800-201906.txt § A.7.2

**Required behavior:** PLM parsing shall interpret encoded per-tile-part packet-length groups from the main-header marker bytes and preserve their wire order for tile-part consumers.

**Observed deviation:** PLM parser rejects standard one-octet Nplm framing as InvalidMarkerLength

This is a permitted direct unit-contract test of metadata interpretation. The real constructor supplies valid parser references, lifetime, limits and zero counters; the real segment reader supplies a correctly bounded raw PLM segment at offset 0. Zplm and next_index both equal 0 and groups starts empty, matching the actual main-header owner's state. Case 1 has Lplm=5, a one-byte Nplm value 1 and one completed positive codeword 1, conforming to pinned T.800 (06/2019) A.7.2 Tables A.35/A.36. Its seven total bytes, one packet and one group are well within platform/configuration limits. No owning-caller normalization is skipped, and production code is unchanged by the test-only child-module patch.

**Recorded failure** (matched by the replay runner):

```text
case 1: valid A.35 octet framing rejected: MalformedInput { offset: 0, kind: InvalidMarkerLength }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/tests/standard_plm.rs:L6-L8`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L6-L8)

```text
let encoded = encode_packet_lengths(lengths);
    let mut group = vec![u8::try_from(encoded.len()).expect("Nplm <= 255")];
    group.extend(encoded);
```

[`src/codestream/tests/standard_plm.rs:L36-L38`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L36-L38)

```text
let mut cases = vec![
        vec![vec![]],
        vec![vec![1]],
```

[`src/codestream/tests/standard_plm.rs:L66-L72`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L66-L72)

```text
let bytes = standard_marker(0, &body);
        let mut parser = new_parser(&bytes, &limits).unwrap();
        let raw = parser.read_segment().unwrap();
        assert_eq!(raw.marker, PLM);
        assert_eq!(raw.offset, 0);
        let mut next = 0;
        let mut groups = Vec::new();
```

[`src/codestream/tests/standard_plm.rs:L74-L79`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L74-L79)

```text
if let Err(error) = parser.parse_plm(raw, &mut next, &mut groups) {
            failures.push(format!(
                "case {case}: valid A.35 octet framing rejected: {error:?}"
            ));
            continue;
        }
```

[`src/codestream/tests/standard_plm.rs:L97-L97`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L97)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/tests/standard_plm.rs`](support/tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs)
- [`validation/plm-standard-tests.patch`](support/tests/DEV-C56978D32B/files/validation/plm-standard-tests.patch)

**Build commands** (in order):

```sh
bash -c 'set -e; patch --batch --fuzz=0 -p1 < validation/plm-standard-tests.patch; cargo test --locked --lib --no-run; for binary in target/debug/deps/jpeg2000_rs-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" target/plm-standard-tests; exit 0; fi; done; exit 127'
```

**Exact test invocation:**

```sh
target/plm-standard-tests codestream::tests::standard_plm::standard_plm_octet_counts_and_wire_order --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-C56978D32B/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing check in this replayed test is its final assert!(failures.is_empty(), "{}", failures.join("\n")); the preceding framing/read_segment assertions succeeded. The host tail truncates the case-0 diagnostic, so the grounded concrete witness is case 1: a single packet of length 1, encoded as marker bytes FF 57 00 05 00 01 01 (payload [Zplm=0,Nplm=1,Iplm=1]). This is a valid direct unit-contract entry, not a fixture bypassing an owning caller. new_parser initializes the actual parser and counters, read_segment returns the actual PLM Segment, next_index is zero, and groups is empty, exactly as initialized by the main-header owner. No full packet body is required by this metadata helper. Real parse_layout dispatch passes the raw segment to parse_plm with no conversion of Nplm to four bytes.

[Standards quote, production lines, and provenance](support/evidence.md#dev-c56978d32b)

</details>

</details>

<a id="reproduce-dev-4f2cff1e64"></a>
<details>
<summary>DEV-4F2CFF1E64 — Metadata inspection rejects valid T.800 (06/2019) PLM framing by treating Nplmi as four bytes</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-4F2CFF1E64 --output rfc-results/DEV-4F2CFF1E64
```


**Standard:** t800-201906.txt § A.7.2

**Required behavior:** Metadata inspection must structurally parse its codestream layout, including supported PLM packet-length metadata, and propagate layout-admission errors rather than return partial success.

**Observed deviation:** Metadata inspection rejects valid T.800 (06/2019) PLM framing by treating Nplmi as four bytes

The harness uses ResourceLimits::default() and byte constructors, not fabricated Parser internals. Existing siz/codestream_with_headers helpers emit SOC, ordinary Part-1 SIZ with one unsigned 8-bit 64x32 component/tile, COD with nonzero LRCP layers and zero decomposition levels, QCD, a main-header PLM, one SOT with correct Psot/Isot/TPsot/TNsot, SOD, one padded empty packet byte per layer, and EOC. The harness independently checks critical header fields and Psot. No PPM/PPT/SOP/EPH changes packet-length semantics. standard_plm encodes Zplm=0, one 8-bit Nplmi equal to the number of packet-length bytes, and one terminating Iplm=1 per actual packet. Maximum 255 layers/packets and 597-byte input fit default limits. For the first failing case, one packet, Nplmi=1 and an 89-byte stream establish a legal, small direct parse entry.

**Recorded failure** (matched by the replay runner):

```text
legal T.800 PLM=true, PLT=false, layers=1: expected metadata, got MalformedInput { offset: 65, kind: InvalidMarkerLength }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/metadata_contract_tests.rs:L20-L22`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L20-L22)

```text
let mut payload = vec![0, layers];
        payload.extend(vec![1; usize::from(layers)]);
        let result = segment(PLM, &payload);
```

[`src/codestream/metadata_contract_tests.rs:L46-L46`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L46)

```text
let bytes = codestream_with_headers(1, &main, &tile, &vec![0; usize::from(layers)]);
```

[`src/codestream/metadata_contract_tests.rs:L121-L122`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L121-L122)

```text
let limits = ResourceLimits::default();
        let bytes = fixture(layers, plm, plt);
```

[`src/codestream/metadata_contract_tests.rs:L124-L124`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L124)

```text
match parse(&bytes, &limits) {
```

[`src/codestream/metadata_contract_tests.rs:L240-L243`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L240-L243)

```text
for layers in layers_to_check() {
            accepted(&mut failures, layers, true, false);
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/metadata_contract_tests.rs`](support/tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs)

**Build commands** (in order):

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/codestream/tests.rs'"'"'); s=p.read_text(); line='"'"'\ninclude!("metadata_contract_tests.rs");\n'"'"'; p.write_text(s+line) if line not in s else None'
```

```sh
cargo test --lib --locked --no-run
```

**Exact test invocation:**

```sh
cargo test --lib --locked codestream::tests::metadata_contract::metadata_structural_admission_includes_standard_plm -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-4F2CFF1E64/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first reported failing check is the accumulated acceptance check for layers=1, PLM=true, PLT=false: parse returns MalformedInput { offset: 65, kind: InvalidMarkerLength } rather than metadata. This is an admissible direct unit-contract entry, not a bypass of an owning caller. Entry realism was checked before tracing: the failing acceptance sweep uses 1..255 layers, one component, one 64x32 tile (2048 pixels), one tile-part, at most 255 one-byte packets, and codestreams of 89..597 bytes; it has no recursive nesting or large fixture arrays. Defaults allow 256 layers, 64 components, 16384 tiles/tile-parts, 67108864 packets, 100000000 pixels and 2147483648 input bytes. These are ordinary bounded codestream inputs, not an implicit-capacity stress case. The real jp2::parse_and_inspect caller similarly passes a codestream box's byte slice and caller limits to this public function.

[Standards quote, production lines, and provenance](support/evidence.md#dev-4f2cff1e64)

</details>

</details>

<a id="reproduce-dev-b6570a1d68"></a>
<details>
<summary>DEV-B6570A1D68 — Metadata inspection rejects standard 8-bit PLM length framing</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-B6570A1D68 --output rfc-results/DEV-B6570A1D68
```


**Standard:** t800-201906.txt § A.7.2

**Required behavior:** Codestream metadata inspection must include supported PLM/PLT layout admission and preserve layout-parser errors without fabricating successful metadata.

**Observed deviation:** Metadata inspection rejects standard 8-bit PLM length framing

Safe Rust byte vectors and ResourceLimits::default(); no private parser state fabricated. Existing baseline helpers construct SOC/SIZ/COD/QCD, exactly one SOT/SOD/EOC tile-part with patched Psot and valid 64x32 one-component geometry. The added standard_plm uses 8-bit Nplmi per pinned T.800 A.7.2. At layers=1 the payload [0,1,1] describes the same one-byte packet as PLT [0,1], and inline tile data [0] encodes an empty, byte-aligned packet under B.9/B.10.3. All fixture and Psot assertions completed. The failing stream is 95 bytes and all positive sweep cases are at most 862 bytes and 255 layers/packets, below default resource limits. Test-only inclusion changes no production behavior.

**Recorded failure** (matched by the replay runner):

```text
legal T.800 PLM=true, PLT=true, layers=1: expected metadata, got MalformedInput { offset: 65, kind: InvalidMarkerLength }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/metadata_contract_tests.rs:L20-L22`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L20-L22)

```text
let mut payload = vec![0, layers];
        payload.extend(vec![1; usize::from(layers)]);
        let result = segment(PLM, &payload);
```

[`src/codestream/metadata_contract_tests.rs:L46-L46`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L46)

```text
let bytes = codestream_with_headers(1, &main, &tile, &vec![0; usize::from(layers)]);
```

[`src/codestream/metadata_contract_tests.rs:L121-L122`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L121-L122)

```text
let limits = ResourceLimits::default();
        let bytes = fixture(layers, plm, plt);
```

[`src/codestream/metadata_contract_tests.rs:L124-L124`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L124)

```text
match parse(&bytes, &limits) {
```

[`src/codestream/metadata_contract_tests.rs:L173-L173`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L173)

```text
accepted(&mut failures, layers, true, true);
```

[`src/codestream/metadata_contract_tests.rs:L175-L175`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L175)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/metadata_contract_tests.rs`](support/tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs)

**Build commands** (in order):

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/codestream/tests.rs'"'"'); s=p.read_text(); line='"'"'\ninclude!("metadata_contract_tests.rs");\n'"'"'; p.write_text(s+line) if line not in s else None'
```

```sh
cargo test --lib --locked --no-run
```

**Exact test invocation:**

```sh
cargo test --lib --locked codestream::tests::metadata_contract::metadata_plm_plt_admission_and_error_propagation -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-B6570A1D68/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing check is the aggregated acceptance assertion at metadata_contract_tests.rs:175, whose first recorded mismatch is layers=1, PLM=true, PLT=true: expected metadata but received MalformedInput { offset: 65, kind: InvalidMarkerLength }. This is a valid direct unit-contract entry to parse, not a bypass of a required owning caller. Real callers jp2::parse_and_inspect and decode::decode_jpx_file likewise pass a raw codestream byte slice and resource limits directly; neither imposes an alternate PLM encoding. Realism was checked before tracing: the first failing stream is only 95 bytes, 64x32 (2048 pixels), one unsigned 8-bit component, one tile/part/resolution/precinct/layer/packet, with one inline empty packet byte. The complete sweep has at most 862 input bytes, 255 layers/packets, one tile-part and no nesting; defaults allow 2147483648 input bytes, 16777216 metadata bytes, 256 layers, 67108864 packets, 16384 tile-parts, and 100000000 pixels. These do not approach an implicit platform, stack, pool or configured limit. The separately tightened packet-limit negative case is intentional and is not the observed failure.

[Standards quote, production lines, and provenance](support/evidence.md#dev-b6570a1d68)

</details>

</details>

<a id="reproduce-dev-80e8727274"></a>
<details>
<summary>DEV-80E8727274 — Precinct validator accepts non-LRCP effective tile COD overrides</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-80E8727274 --output rfc-results/DEV-80E8727274
```


**Standard:** t800-201906.txt § A.7.3

**Required behavior:** For every input byte slice, requested u16 layer count, and ResourceLimits value, observe a normal return for which parsing the slice with those limits succeeds as L and L.info.image_size.tile_count() succeeds as T. The result is Ok(()) if and only if all of the following hold: T=1; L.tile_parts.len()=1; the main-header coding style declares exactly the requested number of layers and LRCP progression; the sole tile part's effective coding style also uses LRCP progression; neither the main header nor the sole tile part has any progression-change records; and the sole tile part has neither an attached packet-length source nor an attached packed-packet-header source. Otherwise the result is Err(DecodeError::MalformedInput { offset: 0, kind: MalformedKind::InvalidJpipLayout }). An attached packet-length or packed-header source is forbidden even when it represents zero entries or zero bytes. This is structural eligibility for explicit-plan reconstruction, not a guarantee of successful entropy decoding.

**Observed deviation:** Precinct validator accepts non-LRCP effective tile COD overrides

A safe direct unit-contract entry with real parsed bytes, not a synthetic CodestreamLayout: 96-byte 1x1 single-component, single-tile, single-part stream, one requested and declared layer, default ResourceLimits, valid tile COD override, no progression-change or attached packet-header/length sources. A zero-byte packet header represents an empty packet; full entropy decoding is not required by this structural clause. The owning caller need not execute for this unit contract, and only the direct-validator false acceptance is claimed.

**Recorded failure** (matched by the replay runner):

```text
main progression=0, effective tile progression=1: expected Err(MalformedInput { offset: 0, kind: InvalidJpipLayout }), got Ok(())
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_precinct_validation_tests.rs:L59-L59`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L59)

```text
let limits = ResourceLimits::default();
```

[`src/jpip_precinct_validation_tests.rs:L113-L114`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L113-L114)

```text
let override_cod = segment(0xff52, &[0, tile, 0, 1, 0, 0, 0, 0, 0, 1]);
            let bytes = precinct_contract_stream(1, main, &[], &[override_cod], &[0]);
```

[`src/jpip_precinct_validation_tests.rs:L120-L123`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L120-L123)

```text
assert_eq!(
                layout.tile_parts[0].coding_style.progression_order,
                orders[usize::from(tile)]
            );
```

[`src/jpip_precinct_validation_tests.rs:L124-L130`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L124-L130)

```text
check(
                &format!("main progression={main}, effective tile progression={tile}"),
                &bytes,
                1,
                &limits,
                main == 0 && tile == 0,
            );
```

[`src/jpip_precinct_validation_tests.rs:L71-L71`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L71)

```text
let actual = validate_precinct_codestream(bytes, layers, limits);
```

[`src/jpip_precinct_validation_tests.rs:L31-L33`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L31-L33)

```text
let length = u32::try_from(bytes.len() - start).unwrap();
    bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&EOC);
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_precinct_validation_tests.rs`](support/tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs)
- [`validation/precinct-contract-alloc/Cargo.toml`](support/tests/DEV-80E8727274/files/validation/precinct-contract-alloc/Cargo.toml)
- [`validation/precinct-contract-alloc/lib.rs`](support/tests/DEV-80E8727274/files/validation/precinct-contract-alloc/lib.rs)
- [`validation/build-precinct-contract-tests.py`](support/tests/DEV-80E8727274/files/validation/build-precinct-contract-tests.py)

**Build commands** (in order):

```sh
python3 validation/build-precinct-contract-tests.py
```

**Exact test invocation:**

```sh
target/precinct-contract-tests --exact jpip::tests::precinct_contract_structural_eligibility --nocapture --test-threads=1
```

[Machine-readable recipe](support/tests/DEV-80E8727274/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed eligibility comparison is main progression=0, effective tile progression=1: the host observed Ok(()) instead of InvalidJpipLayout at offset 0. The final assertion aggregates that comparison and the analogous tile orders 2, 3 and 4; fixture parsing and parsed-order assertions completed successfully before that assertion. This violates exactly the clause's effective-tile LRCP requirement. The clause is sound: SUPPORTED.md restricts explicit-plan reconstructed input to one tile-part with LRCP progression, not merely an LRCP main-header default. The parser intentionally accepts a tile COD override and exposes it as the tile part's effective coding_style.

[Standards quote, production lines, and provenance](support/evidence.md#dev-80e8727274)

</details>

</details>

<a id="reproduce-dev-c36c516466"></a>
<details>
<summary>DEV-C36C516466 — Tile-header parser rejects consistent combined PLM and PLT metadata permitted by T.800 A.7.3</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-C36C516466 --output rfc-results/DEV-C36C516466
```


**Standard:** t800-201906.txt § A.7.3

**Required behavior:** Provide the documented PLM/PLT packet-length support and associated tile-part metadata checks; the implementation's exclusion of combined sources requires reconciliation with that feature intent and T.800 A.7.3.

**Observed deviation:** Tile-header parser rejects consistent combined PLM and PLT metadata permitted by T.800 A.7.3

One component/tile at 1x1, reversible Part1 style, zero decompositions, compatible one-step QCD and initialized component tables; first tile-part with exact Psot and trailing EOC. Counts are 1,2,127,128,255, within default 256-layer admission and the cited edition's 8-bit Nplm bound. PLM has Zplm=0 and Nplm=count followed by count complete positive one-byte length codewords; the borrowed selected descriptor has exact count/size. PLT has Zplt=0 and identical codewords. Payload consists of count zero bytes, each an empty inline packet, so declared lengths sum to payload length. No packed headers, transforms, extension state, malformed private parser fields, oversized allocations or injected downstream failure. This is valid direct-unit evidence, explicitly not a claim that the separately discrepant native parse_plm accepts standard 8-bit framing.

**Recorded failure** (matched by the replay runner):

```text
consistent PLM/PLT must work separately AND together: ["count=1 sources=3: MalformedInput { offset: 84, kind: InvalidMarkerOrder }", "count=2 sources=3: MalformedInput { offset: 85, kind: InvalidMarkerOrder }", "count=127 sources=3: MalformedInput { offset: 210, kind: InvalidMarkerOrder }", "count=128 sources=3: MalformedInput { offset: 211, kind: InvalidMarkerOrder }", "count=255 sources=3: MalformedInput { offset: 338, kind: InvalidMarkerOrder }"]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/read_tile_part_contract.rs:L52-L60`](support/tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L52-L60)

```text
        assert_eq!(plm.payload[0], 0);
        assert_eq!(usize::from(plm.payload[1]), plm.payload.len() - 2);
        let encoded = &plm.payload[2..];
        assert!(encoded.iter().all(|&b| b == 1));
        main_lengths = PacketLengths {
            source: PacketLengthSource::Plm(encoded),
            packet_count: encoded.len() as u64,
            encoded_bytes: encoded.len() as u64,
        };
```

[`src/codestream/read_tile_part_contract.rs:L62-L79`](support/tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L62-L79)

```text
    let sot = parser.read_sot(1).unwrap();
    let result = parser.read_tile_part(
        sot,
        (style, quant),
        ComponentCodingStyles::new(size.components, style).unwrap(),
        ComponentQuantizations::new(size.components, quant).unwrap(),
        RoiShifts::none(),
        ProgressionChanges::none(),
        MctParameters {
            arrays: vec![],
            records: vec![],
            order: vec![],
        },
        Part2Extensions::default(),
        size.components,
        main_lengths,
        PackedPacketHeaders::none(),
    );
```

[`src/codestream/read_tile_part_contract.rs:L324-L339`](support/tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L324-L339)

```text
    for count in [1_u16, 2, 127, 128, 255] {
        for sources in 1..=3 {
            let lengths = vec![1; usize::from(count)];
            let mut main = vec![tile_contract_cod(count, 0, 2, 0), segment(QCD, &[0, 0])];
            if sources & 1 != 0 {
                let mut payload = vec![0, count as u8]; // Zplm, 8-bit Nplm
                payload.extend(vec![1; usize::from(count)]);
                main.push(segment(PLM, &payload));
            }
            let tile = if sources & 2 != 0 {
                vec![plt(0, &lengths)]
            } else {
                vec![]
            };
            let bytes =
                tile_contract_stream(1, 0, &main, &tile, &vec![0; usize::from(count)], false);
```

[`src/codestream/read_tile_part_contract.rs:L350-L353`](support/tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L350-L353)

```text
    assert!(
        failures.is_empty(),
        "consistent PLM/PLT must work separately AND together: {failures:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/read_tile_part_contract.rs`](support/tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs)
- [`validation/read-tile-part-contract.py`](support/tests/DEV-C36C516466/files/validation/read-tile-part-contract.py)

**Build commands** (in order):

```sh
python3 validation/read-tile-part-contract.py build
```

**Exact test invocation:**

```sh
python3 validation/read-tile-part-contract.py run plm_plt
```

[Machine-readable recipe](support/tests/DEV-C36C516466/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the final failures.is_empty() check at src/codestream/read_tile_part_contract.rs:350, reporting 'consistent PLM/PLT must work separately AND together'. The first collected mismatch is count=1 sources=3: MalformedInput { offset: 84, kind: InvalidMarkerOrder }; all five combined-source cases fail, whereas the same replay reports no failures for either source separately. This is a specific rejection returned by the subject, not a setup panic or merely exit 101.

[Standards quote, production lines, and provenance](support/evidence.md#dev-c36c516466)

</details>

</details>

<a id="reproduce-dev-709cd8127b"></a>
<details>
<summary>DEV-709CD8127B — SOP-enabled progression decoding rejects standards-permitted omission of SOP</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-709CD8127B --output rfc-results/DEV-709CD8127B
```


**Standard:** t800-201906.txt § A.8.1

**Required behavior:** Progression precinct decoding routes the tile-part coding style and expected tile packet sequence through the documented packet-decoding model, including its SOP-prefix validation policy.

**Observed deviation:** SOP-enabled progression decoding rejects standards-permitted omission of SOP

A tiny valid first-packet unit entry: a parsed single-tile Part-1 stream, legal COD Scod=2, one empty header byte 0x00, no EPH or packed source, all initial cursors/counters unchanged at zero, coherent one-block literal precinct, valid component/style and constructor-derived grid/input/progress, default resource limits. A.8.1 in the assigned 2019 edition permits SOP omission on this packet. Absence of SOP is the legal input being exercised, not a violated entry requirement. No body decoding or unsafe state fabrication is involved.

**Recorded failure** (matched by the replay runner):

```text
T.800 A.8.1 permits absent SOP with Scod bit 1 set: packed=false, flags=2, actual=Err(MalformedInput { offset: 0, kind: Truncated }), expected=Ok((PacketBodyDecode { present: false, code_blocks: 1, contributions: 0, segments: 0, passes: 0, header_bytes: 1, body_bytes: 0, bytes_consumed: 1 }, 1, 0))
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/packet_progression_contract_tests.rs:L421-L430`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L421-L430)

```text
    for packed in [false, true] {
        for flags in [0_u8, 2, 4, 6] {
            let eph = flags & 4 != 0;
            let mut header = vec![0];
            if eph {
                header.extend_from_slice(&[0xff, 0x92]);
            }
            let tile: &[u8] = if packed { &[] } else { &header };
            let stream = contract_codestream(flags, 1, tile, packed.then_some(header.as_slice()));
            let layout = contract_layout(&stream, flags, 1, packed.then_some(header.as_slice()));
```

[`src/packet_progression_contract_tests.rs:L433-L442`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L433-L442)

```text
            let actual = contract_progression_call(
                &layout.tile_parts,
                None,
                contract_coordinate(0),
                0,
                0,
                0,
                None,
                &mut precinct,
            );
```

[`src/packet_progression_contract_tests.rs:L97-L97`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L97)

```text
    let input = prepare_progression_payloads(parts, parts[0].tile_index, &limits).unwrap();
```

[`src/packet_progression_contract_tests.rs:L117-L117`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L117)

```text
    let mut progress = progression_tile_progress_new(&params).unwrap();
```

[`src/packet_progression_contract_tests.rs:L127-L129`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L127-L129)

```text
    decode_progression_precinct_packet(
        &params, &parts[0], &progress, coordinate, expected, precinct,
    )
```

[`src/packet_progression_contract_tests.rs:L522-L522`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L522)

```text
    assert!(omissions.is_empty(), "{}", omissions.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/packet_progression_contract_tests.rs`](support/tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs)
- [`validation/deepspec-progression-registration.patch`](support/tests/DEV-709CD8127B/files/validation/deepspec-progression-registration.patch)
- [`validation/build-progression-contract.py`](support/tests/DEV-709CD8127B/files/validation/build-progression-contract.py)

**Build commands** (in order):

```sh
python3 validation/build-progression-contract.py
```

**Exact test invocation:**

```sh
target/deepspec-progression-suite --exact packet::tests::progression_contract_sop_policy_and_optional_marker --nocapture
```

[Machine-readable recipe](support/tests/DEV-709CD8127B/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the final omissions.is_empty() check at src/packet_progression_contract_tests.rs:522. Its first collected mismatch is packed=false, flags=2: a complete one-byte empty-packet header produces Truncated instead of Ok with one tile byte consumed. This is a code deviation under the clause's rfc_map authority, not a bad oracle inferred from native/model agreement. I read clause_origin and independently retrieved its original T.800 (06/2019), ISO/IEC 15444-1:2019 A.8.1 passage, version 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, through the originating rfc_map item. It explicitly says that when COD allows SOP, each packet may or may not have an SOP segment; Nsop still counts every packet. It also explicitly permits SOP placement before tile-part packet bodies with PPT/PPM. Thus the intended SOP policy is permission, not mandatory presence. The clause's generic model-routing wording is interpreted with that parent finding; the existing model/callee summaries that charge six SOP bytes whenever enabled share the implementation restriction and are not caller preconditions overriding the standard. SUPPORTED.md advertises SOP/EPH support without documenting this mandatory-per-packet restriction.

[Standards quote, production lines, and provenance](support/evidence.md#dev-709cd8127b)

</details>

</details>

<a id="reproduce-dev-ea51acda28"></a>
<details>
<summary>DEV-EA51ACDA28 — Inline EPH inspection rejects a valid header with terminal FF stuffing</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-EA51ACDA28 --output rfc-results/DEV-EA51ACDA28
```


**Standard:** t800-201906.txt § A.8.2

**Required behavior:** Apply the template coding-style EPH policy during inline packet inspection, propagate marker errors, and include a validated EPH in the successful consumed prefix.

**Observed deviation:** Inline EPH inspection rejects a valid header with terminal FF stuffing

A bounded safe-Rust inspection-unit fixture: one fresh packet state, one context and decoder, one valid 1x1 LL subband with fresh inclusion/zero-bitplane trees. The helper sets context maximum_zero_bitplanes=8 and constructs a matching eight-bitplane regular decoder using the real constructor. Header bits encode first inclusion, ZBP=6<8, one pass, Lblock=8, length=255. Mandatory terminal stuffing is 00, followed by enabled EPH FF92 and all 255 declared opaque body bytes. Layer zero is valid, component zero has a valid style, packed headers and SOP are absent, all default limits are respected. Inspection does not execute Tier-1, so entropy validity of the body is not a premise.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: inline inspection result/error/prefix
  left: Err(MalformedInput { offset: 0, kind: InvalidPacketHeader })
 right: Ok(261)
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/packet_jpip_contract_tests.rs:L61-L65`](support/tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L61-L65)

```text
let mut precinct = literal_packet_precinct();
    precinct.contexts[0].maximum_zero_bitplanes = 8;
    precinct.decoders[0] = Tier1CodeBlockDecoder::new(
        1, 1, 8, SubbandOrientation::Ll, Tier1CodingStyle::default(),
    ).unwrap();
```

[`src/packet_jpip_contract_tests.rs:L75-L80`](support/tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L75-L80)

```text
assert_eq!(bits.len(), 24);
    assert_eq!(pack_packet_bits(&bits), [0xc0, 0xbe, 0xff]);
    let header = [0xc0, 0xbe, 0xff, 0];
    assert_eq!(header[3] & 0x80, 0, "mandatory stuffed zero");
    let body = vec![0xff; 255];
    (precinct, jpip_framed(&header, &body, false, eph, 0))
```

[`src/packet_jpip_contract_tests.rs:L205-L208`](support/tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L205-L208)

```text
let (mut precinct, data) = jpip_terminal_ff_packet(true);
    assert_eq!(&data[4..6], &[0xff, 0x92], "EPH follows the complete stuffed header");
    jpip_assert_inspect(&jpip_template(false, true), &data, 0, 0, &mut precinct,
        &limits, Ok(4 + 2 + 255));
```

[`src/packet_jpip_contract_tests.rs:L50-L54`](support/tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L50-L54)

```text
assert_eq!(
        decode_jpip_precinct_packet_inner(template, bytes, sequence, layer, precinct, limits),
        expected,
        "inline inspection result/error/prefix"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/packet_jpip_contract_tests.rs`](support/tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs)
- [`jpip_contract_registration.patch`](support/tests/DEV-EA51ACDA28/files/jpip_contract_registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jpip_contract_registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/packet.rs'"'"').read_text(); t=Path('"'"'src/packet_jpip_contract_tests.rs'"'"').read_text(); assert s.count('"'"'include!("packet_jpip_contract_tests.rs");'"'"')==1; assert t.count('"'"'#[test]'"'"')==7; assert '"'"'let header = [0xc0, 0xbe, 0xff, 0];'"'"' in t; assert '"'"'Ok(4 + 255)'"'"' in t; assert '"'"'Ok(4 + 2 + 255)'"'"' in t; assert '"'"'fn jpip_assert_parser_accounting('"'"' in t; assert '"'"'assert_eq!(physical, logical,'"'"' in t; assert '"'"'actual_jpip_outer_restores_all_raw_fields_and_clone_preserves_unused_tree_words();'"'"' in t; assert '"'"'full_progression_uses_real_ppt_reassembly_and_plt_iteration_without_tile_body_bytes();'"'"' in t'
```

```sh
cargo test --locked --lib --no-run --target-dir target/deepspec_jpip_contract
```

```sh
python3 -c 'from pathlib import Path; import os,shutil,subprocess; d=Path('"'"'target/deepspec_jpip_contract'"'"'); a=[p for p in (d/'"'"'debug/deps'"'"').glob('"'"'jpeg2000_rs-*'"'"') if p.is_file() and os.access(p,os.X_OK)]; assert len(a)==1,a; r=d/'"'"'runner'"'"'; shutil.copy2(a[0],r); names=subprocess.check_output([str(r),'"'"'--list'"'"'],text=True); assert names.count('"'"'packet::tests::jpip_contract_'"'"')==7,names'
```

**Exact test invocation:**

```sh
target/deepspec_jpip_contract/runner --exact packet::tests::jpip_contract_eph_policy_errors_and_prefix --nocapture
```

[Machine-readable recipe](support/tests/DEV-EA51ACDA28/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is src/packet_jpip_contract_tests.rs:50, reached by the terminal-FF case at lines 205-208: the host observed Err(MalformedInput { offset: 0, kind: InvalidPacketHeader }) instead of Ok(261). This violates clause 62986c33c311dfc825987719's EPH validation/consumed-prefix guarantee on an admissible direct inspection-unit entry, not a build failure or Tier-1 decode failure. Capacity check: this case supplies 261 bytes (four header bytes, two EPH bytes, 255 body bytes), one code block, one subband, two one-node trees, one new pass, layer 0 of two, and eight maximum bitplanes. These are far below default limits (2,147,483,648 input bytes, 268,435,456 header bytes, 67,580 code-block bytes, 16,777,216 code blocks/tag-tree nodes, 874 passes, 256 layers, and 1,000,000,000 work units); there is no deep recursion or oversized fixture. The fresh state, single-leaf trees, matching vector lengths, coordinates and eight-bitplane decoder are consistent with the normal PacketPrecinctDecoder::new/new_subbands construction for one block. It is an inspection contract: opaque declared body bytes need not establish entropy-decoder success because the adapter explicitly passes decode_tier1=false. No owning-caller preparation is skipped that this direct unit contract requires.
Route: jpip_contract_eph_policy_errors_and_prefix -> jpip_terminal_ff_packet(true) -> jpip_assert_inspect -> decode_jpip_precinct_packet_inner. Its MIR bb1 installs layer zero; bb12 constructs inline data with absent packed headers and invokes decode_precinct_packet at offset zero, SOP disabled, EPH enabled, Tier-1 disabled. Contribution parsing consumes exactly 24 information bits C0 BE FF: presence and first inclusion, six zero bitplanes, one pass, Lblock incremented from 3 to 8, length 255. The fourth byte 00 is the mandatory final stuffed/padded byte. PacketBitReader only loads a following stuffed byte when another information bit is requested. align_to_byte merely clears bits_remaining, so after_headers obtains header_bytes=3, then validates packet_data[3..5]=00 FF as EPH instead of the actual FF92 at [4..6]. The invalid-marker branch returns InvalidPacketHeader, propagated unchanged by the inner adapter's MIR bb16. The exact failed check is marker validation at the incorrectly computed boundary, not a failure in fixture setup.
Authority: clause_origin is rfc_map, not infer. I read its parent finding and manifest, and independently read cited T.800 (06/2019) B.10.1 and A.8.2 through the document tools. B.10.1 expressly requires a stuffed zero even when FF would otherwise be the last header byte; A.8.2 places FF92 immediately after the complete packet header when COD signals EPH. The harness meets both. The parent finding explicitly excluded the final-FF boundary from its implemented assessment; that is a limitation of that assessment, not an input illegality or a caller requirement that the implementation already compute the boundary correctly. Accordingly this is not grounds to revise a sound standards rule to match the correlated native/model accounting. Edition reconciliation with selected T.800 (2024) remains outstanding as recorded in specs/README.md: this diagnosis is grounded in the cited 2019 edition and the stated inspection clause, not a certification of 2024 conformance or a fully decoded JPIP codestream.

[Standards quote, production lines, and provenance](support/evidence.md#dev-ea51acda28)

</details>

</details>

<a id="reproduce-dev-2f140d1710"></a>
<details>
<summary>DEV-2F140D1710 — Native selected-color context excludes the CRG-registered predecessor sample</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-2F140D1710 --output rfc-results/DEV-2F140D1710
```


**Standard:** t800-201906.txt § A.9.1

**Required behavior:** Selected-color context preparation shall honor component registration and subsampling when retaining the full-image sampling phase before cropping; registration-bearing metadata from a CRG marker must retain its defined physical offset meaning.

**Observed deviation:** Native selected-color context excludes the CRG-registered predecessor sample

Safe native-resolution direct helper invocation with parser-derived ImageSize, positive sampling, origin zero, a nonempty in-image ROI and a validator-produced tile list [0]. The 1024x1024 single-tile fixtures with at most three components are well within HARDENED geometry/resource limits. The CRG appears exactly once in the main header with one u16 pair per component and length 2+4*N, satisfying T.800 (06/2019) A.9.1 framing. No manual unit conversion or metadata mutation occurs. At separation 4 and registration 32768, the physical offset is 2 and the project's floor/predecessor policy requires sample 1 at coordinate 9; the context starting at 5 contains only coded sample indices 2..3.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: parse_layout(CRG)->validate_selection->color_selection_from_validated: components=[[4, 4, 32768, 32768]] ROI=[9, 9, 10, 10] context=ImageRegion { x: 5, y: 5, width: 5, height: 5 }; component=0 axis=0 coordinate=9: physical sample 1 missing from coded range 2..3
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode_crg_tests.rs:L125-L126`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L125-L126)

```text
        let layout = parse_layout(&bytes, &ResourceLimits::HARDENED)
            .expect("standards-framed main-header CRG fixture must parse");
```

[`src/decode_crg_tests.rs:L152-L158`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L152-L158)

```text
        let validated = validate_selection(size, &requested)
            .expect("nonempty in-image ROI on the sole tile is valid");
        assert_eq!(validated.region, region);
        assert_eq!(validated.tiles, vec![0]);
        assert_eq!(validated.discard_levels, 0);
        let (expanded, target) = color_selection_from_validated(size, validated)
            .expect("native-resolution context preparation must succeed");
```

[`src/decode_crg_tests.rs:L64-L66`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L64-L66)

```text
    let mut cases = (9..=14)
        .map(|x| (vec![[4, 4, 32_768, 32_768]], [x, x, x + 1, x + 1]))
        .collect::<Vec<_>>();
```

[`src/decode_crg_tests.rs:L51-L52`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L51-L52)

```text
    let offset = separation * u64::from(registration);
    let position = u64::from(coordinate) * 65_536;
```

[`src/decode_crg_tests.rs:L56-L56`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L56)

```text
    position.saturating_sub(offset) / (separation * 65_536)
```

[`src/decode_crg_tests.rs:L175-L176`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L175-L176)

```text
                let first_available = start.div_ceil(separation);
                let end_available = u64::from(region[axis + 2]).div_ceil(separation);
```

[`src/decode_crg_tests.rs:L181-L181`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L181)

```text
                    if required < first_available || required >= end_available {
```

[`src/decode_crg_tests.rs:L198-L198`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L198)

```text
    assert_eq!(failures, 0, "{}", examples.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode_crg_tests.rs`](support/tests/DEV-2F140D1710/files/src/decode_crg_tests.rs)
- [`validation/crg-context-tests.patch`](support/tests/DEV-2F140D1710/files/validation/crg-context-tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/crg-context-tests.patch
```

```sh
cargo test --locked --lib --no-run --target-dir target/deepspec-crg
```

**Exact test invocation:**

```sh
python3 -c 'import os, subprocess; from pathlib import Path; binaries = [p for p in Path('"'"'target/deepspec-crg/debug/deps'"'"').glob('"'"'jpeg2000_rs-*'"'"') if p.is_file() and os.access(p, os.X_OK)]; assert len(binaries) == 1, binaries; name = '"'"'decode::tests::selected_color_context_uses_crg_component_separation'"'"'; assert name + '"'"': test'"'"' in subprocess.check_output([str(binaries[0]), '"'"'--list'"'"'], text=True).splitlines(); os.execv(str(binaries[0]), [str(binaries[0]), '"'"'--exact'"'"', name, '"'"'--nocapture'"'"'])'
```

[Machine-readable recipe](support/tests/DEV-2F140D1710/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the final assert_eq!(failures, 0), with host-observed left=3874 and right=0; earlier parsing, metadata, ROI validation, target and returned-geometry checks succeeded. This is a valid direct unit-contract test, not a bypassed caller fixture: it obtains ImageSize from parse_layout(CRG), constructs an ordinary DecodeSelection, calls the actual validate_selection, then invokes color_selection_from_validated exactly as color_selection_for_size does. The test-only patch only includes the harness in decode::tests. Resource realism was checked first: each fixture is a 1024x1024 image (1,048,576 pixels), one tile, 1 or 3 eight-bit components, one resolution/layer, positive SIZ sampling <=255, discard=0, and a single main-header CRG of admissible length. HARDENED allows dimensions 1,000,000, 100,000,000 pixels, 64 components and 16,384 tiles. The 5437 cases are processed sequentially with tiny metadata/packet fixtures; even a decoded three-component image would be only a few MB, far below the 2 GiB decoded-byte limit. No oversized buffer, stack recursion or fabricated ValidatedSelection is involved. Empty packets are not used to claim decoded-pixel behavior: this helper consumes parsed geometry, not decoded sample data.

[Standards quote, production lines, and provenance](support/evidence.md#dev-2f140d1710)

</details>

</details>

<a id="reproduce-dev-355fe751f9"></a>
<details>
<summary>DEV-355FE751F9 — Reduced selected decode applies tile-local nonlinear transforms on unreduced tile coordinates</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-355FE751F9 --output rfc-results/DEV-355FE751F9
```


**Standard:** t800-201906.txt § B.1

**Required behavior:** Express selected component nonlinear-transform regions as nonempty half-open intersections of tile-component bounds and output extents, translated to output-local sample coordinates.

**Observed deviation:** Reduced selected decode applies tile-local nonlinear transforms on unreduced tile coordinates

Safe Rust slices and default options; actual encoded SIZ/COD/QCD/SOT/SOD/NLT bytes, not patched layout or reconstruction state. Image [1,32)x[1,16), eight 8x8 tiles, one unsigned 8-bit unit-sampled component, one decomposition level, empty packet payloads reconstructing zero coefficients. Tile-local gamma slopes [1,2,4,8,2,4,8,1] are positive supported parameters with equal source/target precision, exponent=1, threshold=1, a=1,b=0. All tile boundaries are multiples of eight, so discard=1 ownership is unambiguous. Region [3,29)x[3,15) lies inside the image. Tiny resource requirements are below default policy and ordinary caller scale. Successful unwrap and preceding controls establish successful public-entry decoding, while the independent slope oracle exposes wrong tile transform assignment.

**Recorded failure** (matched by the replay runner):

```text
native tile-specific nonlinear transform mismatch: ["d=1, region=None, native=(4,1), tile=1, expected=64, actual=128", "d=1, region=Some(ImageRegion { x: 3, y: 3, width: 26, height: 12 }), native=(4,2), tile=1, expected=64, actual=128"]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode_selected_contract_tests.rs:L4-L4`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L4)

```text
for value in [32_u32, 16, 1, 1, 8, 8, 0, 0] {
```

[`src/decode_selected_contract_tests.rs:L53-L59`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L53-L59)

```text
fn selected_contract_patterned() -> Vec<u8> {
    let mut bytes = selected_contract_grid(1, &[[7, 1, 1]]);
    for (tile, slope) in [1_u8, 2, 4, 8, 2, 4, 8, 1].iter().enumerate() {
        selected_contract_tile_header(&mut bytes, tile as u16, selected_contract_gamma(*slope));
    }
    bytes
}
```

[`src/decode_selected_contract_tests.rs:L204-L208`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L204-L208)

```text
for discard in 0..=1 {
        for region in [None, Some(crate::ImageRegion::new(3, 3, 26, 12))] {
            let selection =
                crate::options::selection_from_parts(TileSelection::All, region, discard);
            let image = decode_codestream_selected(&patterned, &options, &selection).unwrap();
```

[`src/decode_selected_contract_tests.rs:L232-L234`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L232-L234)

```text
let tile = (py * scale / 8) * 4 + px * scale / 8;
                    let expected = 128 / u64::from(slopes[tile as usize]);
                    let actual = samples[((py - y) * (ex - x) + px - x) as usize];
```

[`src/decode_selected_contract_tests.rs:L247-L250`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L247-L250)

```text
assert!(
        mismatches.is_empty(),
        "native tile-specific nonlinear transform mismatch: {mismatches:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode_selected_contract_tests.rs`](support/tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs)
- [`selected_contract_setup.py`](support/tests/DEV-355FE751F9/files/selected_contract_setup.py)

**Build commands** (in order):

```sh
python3 selected_contract_setup.py
```

```sh
cargo test --locked --lib --no-run
```

```sh
python3 -c 'import glob,os,shutil,subprocess; p=[p for p in glob.glob('"'"'target/debug/deps/jpeg2000_rs-*'"'"') if os.path.isfile(p) and os.access(p,os.X_OK)]; assert len(p)==1,p; names=subprocess.check_output([p[0],'"'"'--list'"'"'],text=True); assert names.count('"'"'decode::tests::selected_contract_'"'"')==5,names; shutil.copy2(p[0],'"'"'target/selected-contract-suite'"'"')'
```

**Exact test invocation:**

```sh
target/selected-contract-suite decode::tests::selected_contract_geometry_and_native_samples --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-355FE751F9/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final mismatches.is_empty() check in selected_contract_geometry_and_native_samples, not a setup error or generic exit status: host replay reports d=1 native=(4,1), tile=1, expected=64, actual=128, and the equivalent cropped failure at (4,2). The clause has infer origin, not rfc_map. Its promise that selected native-resolution samples receive their nonlinear processing is sound for this API: README explicitly promises native wavelet resolution, decode_typed_layout deliberately routes selected reconstruction through typed_layout, and typed_layout deliberately applies each tile's nonlinear transform. This is not a request for a new standards guarantee or a revision to match a shared model.

[Standards quote, production lines, and provenance](support/evidence.md#dev-355fe751f9)

</details>

</details>

<a id="reproduce-dev-404e919a62"></a>
<details>
<summary>DEV-404E919A62 — Arbitrary-filter lifting selects the wrong source neighbour for odd-origin axes</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-404E919A62 --output rfc-results/DEV-404E919A62
```


**Standard:** t800-201906.txt § F.3.2

**Required behavior:** Reconstruct one two-dimensional wavelet level with the supplied arbitrary kernel, using its reversibility-derived precision mode, the level's horizontal and vertical phases and the declared output geometry.

**Observed deviation:** Arbitrary-filter lifting selects the wrong source neighbour for odd-origin axes

Three ordinary borrowed samples, mathematically correct parity-partitioned dimensions and exact sample lengths, W=1,H=3, and decoded-byte limit 24 satisfy admission. The semantic REV/ARB/CON kernel has minit=0, signed 8-bit coefficients [-1] then [2], zero offsets/exponents/residues, and no scaling. It is built with existing explicit test-only constructors; unused descriptor metadata is not interpreted by this numerical unit API. Both singleton high-pass horizontal inputs are even, so halving is exact. All subsequent arithmetic is small exact reversible integer arithmetic. The independent standard oracle yields vertical [-90,128,-40] -> [-90,208,-40] -> [118,208,168].

**Recorded failure** (matched by the replay runner):

```text
1x3, phases=false/false, REV=true, minit=false: interleaved input=[-180, 256, -80]; expected [118, 208, 168]; got ReconstructedPlane { width: 1, height: 3, fraction_bits: 0, samples: [218, 308, 268] }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/transform/tests/arbitrary_level_contract.rs:L212-L212`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L212)

```text
                            let result = inverse_level_arbitrary(level, &kernel, &limits);
```

[`src/transform/tests/arbitrary_level_contract.rs:L208-L211`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L208-L211)

```text
                            let bytes = (width * height * 8) as u64;
                            let limits = ResourceLimits::default()
                                .with_maximum_decoded_bytes(bytes)
                                .unwrap();
```

[`src/transform/tests/arbitrary_level_contract.rs:L166-L169`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L166-L169)

```text
                                    let high_x = (x % 2 == 0) != first_x;
                                    let high_y = (y % 2 == 0) != first_y;
                                    bands[usize::from(high_x) + 2 * usize::from(high_y)]
                                        .push(data[y * width + x]);
```

[`src/transform/tests/arbitrary_level_contract.rs:L74-L75`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L74-L75)

```text
            let n = (coordinate - parity) / 2;
            let source = 2 * n + 1 - parity;
```

[`src/transform/tests/arbitrary_level_contract.rs:L309-L312`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L309-L312)

```text
    assert!(
        failures.is_empty(),
        "{} standard-equation mismatches; first examples:\n{}",
        failures.len(),
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/transform/tests/arbitrary_level_contract.rs`](support/tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs)
- [`arbitrary-level-registration.patch`](support/tests/DEV-404E919A62/files/arbitrary-level-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i arbitrary-level-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/transform.rs'"'"').read_text(); t=Path('"'"'src/transform/tests/arbitrary_level_contract.rs'"'"').read_text(); assert '"'"'    mod arbitrary_level_contract;'"'"' in s; assert '"'"'assert_eq!(contexts, 560);'"'"' in t; assert '"'"'failures.is_empty()'"'"' in t; assert '"'"'let result = inverse_level_arbitrary(level, &kernel, &limits);'"'"' in t; print('"'"'Replay source guards: additive registration, 560-case assertion, output oracle and subject call present'"'"')'
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib transform::tests::arbitrary_level_contract::arbitrary_level_matches_standard_equations_and_rejects_bad_geometry_or_budget -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-404E919A62/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

Case 8b69097aab2e6e4d6823ff4c faithfully exposes a deviation from clause d348494f1956deec3f860649: reconstruction with the supplied arbitrary kernel must respect the horizontal and vertical phases, including the source coordinates of its lifting filter. The first failed assertion is failures.is_empty() at harness line 309, after all 560 contexts and geometry/storage/budget/immutability checks completed. It aggregates 393 numerical mismatches, not a setup failure or an arbitrary nonzero exit. The first printed example is an irreversible one-unit discrepancy; I do not rely on exact irreversible precision or classify every reported mismatch. The same failed assertion includes the exact reversible 1x3 witness quoted in observation, which has no rounding-tolerance ambiguity.

[Standards quote, production lines, and provenance](support/evidence.md#dev-404e919a62)

</details>

</details>

<a id="reproduce-dev-37a5bf80e8"></a>
<details>
<summary>DEV-37A5BF80E8 — Part 1 vertical 9/7 synthesis omits odd-origin singleton halving</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-37A5BF80E8 --output rfc-results/DEV-37A5BF80E8
```


**Standard:** t800-201906.txt § F.3.5

**Required behavior:** The Part 1 vertical-column adapter extracts matching low/high columns at x and returns their phase-sensitive one-dimensional inverse synthesis for placement in that output column.

**Observed deviation:** Part 1 vertical 9/7 synthesis omits odd-origin singleton halving

For extent [1,2), the phase formula produces low_height=0 and high_height=1; both bands have width=1, x=0, exact row-major slice lengths zero and one, respectively. False low_pass_first correctly selects the high-band singleton. The one signed Q16 coefficient is even and modest, and all dimensions, capacities, byte limits and arithmetic are representable. Interleave's required low count is floor(1/2)=0 and high count is one. The unit test needs no fabricated private state, and the public caller accepts the same fixture. Test-only registration does not change production behavior.

**Recorded failure** (matched by the replay runner):

```text
inverse_column_pair width=1 height=1 origin=1 x=0 irreversible=true low=[] high=[-15859712]: expected=[-7929856], actual=Ok([-15859712])
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/transform_column_pair_contract_tests.rs:L15-L17`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L15-L17)

```text
let origin = u32::from(!low_first);
                    let low_height = (origin + height).div_ceil(2) - origin.div_ceil(2);
                    let high_height = height - low_height;
```

[`src/transform_column_pair_contract_tests.rs:L58-L59`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L58-L59)

```text
assert_eq!(low.samples.len(), (width * low_height) as usize);
                    assert_eq!(high.samples.len(), (width * high_height) as usize);
```

[`src/transform_column_pair_contract_tests.rs:L65-L74`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L65-L74)

```text
let expected = if height == 1 {
                            // F.3.6: X(i0)=Y(i0) for even i0, Y(i0)/2 for odd i0.
                            Ok(if low_first {
                                vec![l[0]]
                            } else {
                                vec![h[0] / 2]
                            })
                        } else {
                            reference_sequence(l, h, low_first, irreversible)
                        };
```

[`src/transform_column_pair_contract_tests.rs:L80-L82`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L80-L82)

```text
let actual =
                            inverse_column_pair(low, high, x, low_first, irreversible, None);
                        if actual.as_ref() != Ok(&expected) {
```

[`src/transform_column_pair_contract_tests.rs:L112-L118`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L112-L118)

```text
inverse_vertical_9_7(
                                low,
                                high,
                                height,
                                low_first,
                                &ResourceLimits::default(),
                            )
```

[`src/transform_column_pair_contract_tests.rs:L157-L157`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L157)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/transform_column_pair_contract_tests.rs`](support/tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs)
- [`validation/install-column-pair-contract-test.py`](support/tests/DEV-37A5BF80E8/files/validation/install-column-pair-contract-test.py)

**Build commands** (in order):

```sh
python3 validation/install-column-pair-contract-test.py
```

```sh
cargo test --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --lib transform::tests::part1_column_pair_contract -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-37A5BF80E8/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed numerical check accumulated by the replay is width=1,height=1,origin=1,x=0,irreversible=true: expected [-7929856], actual Ok([-15859712]); the final failures.is_empty() assertion reports it. This is a valid unit-contract entry, not malformed fixture setup. Realism was checked: generated dimensions are at most 7x65 (455 i64 samples, 3640 bytes per full plane); 1196 selected columns are processed sequentially with no recursion or large fixed stack buffers. All generated coefficient bands together contain 19656 samples across the sequential geometry/filter/phase fixtures, not a simultaneous allocation. Default HARDENED limits allow 1000000 per dimension, 100000000 pixels and 2147483648 decoded bytes. The failing entry needs only one i64 coefficient. Widths match, x=0 is in bounds, rectangular backing lengths are exact, and origin=1 with extent [1,2) gives low height zero and high height one. Empty low storage is therefore required, not an inadmissible missing band. Q16 high=-15859712 is even, so division by two is exact and does not depend on rounding or overflow conventions. No callee entry requirement is violated.
The clause originates in rfc_map 0c974871043c1354a71e5867. Independently re-read its pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e: F.3.5 requires per-column 1D_SR; F.3.6 states for length one X(i0)=Y(i0) for even i0 and Y(i0)/2 for odd i0. That singleton rule precedes and is not restricted by the subsequent length>=2 filter-specific procedures, so it applies to the selected Part 1 irreversible filter as well. None excludes Part 2 kernels. This establishes the oracle for this cited edition without claiming reconciliation with newer editions. The stored inverse_9_7 contracts and Q16 model explicitly describe unchanged singletons, but their stated authority is the project model, not independently established standard conformance; their agreement with implementation cannot override this standards-derived clause.
Compiler declaration/CFG evidence shows inverse_column_pair successfully collects [] and [-15859712], takes kernel=None then irreversible=true (bb8 -> bb14 -> bb15 -> bb16 -> bb17), and calls inverse_9_7. Its CFG takes interleave success then length<=1 (bb4 -> bb6 -> bb7) and returns the unchanged vector, without phase-sensitive singleton division. The harness directly invokes this valid unit entry and independently exercises the owning public route inverse_vertical_9_7 -> inverse_vertical -> inverse_columns -> inverse_column_pair -> inverse_9_7. Public geometry/byte checks accept the entry, and store_column writes the same incorrect value into the 1x1 output. The host output explicitly reports that public result too. Consequence: the odd-origin singleton column is twice its required reconstructed value. All 17 reported mismatches are this boundary (13 selected columns and four public planes); no claim of whole-decoder/codestream reachability or other lifting-rule correctness is needed. Production source is unchanged.

[Standards quote, production lines, and provenance](support/evidence.md#dev-37a5bf80e8)

</details>

</details>

<a id="reproduce-dev-2d75f3e8f6"></a>
<details>
<summary>DEV-2D75F3E8F6 — JP2 validation accepts additional top-level JPEG 2000 signature boxes</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-2D75F3E8F6 --output rfc-results/DEV-2D75F3E8F6
```


**Standard:** t800-201906.txt § I.5.1

**Required behavior:** JP2 container validation shall recognize the fixed JPEG 2000 signature as the first box and validate the file rule that exactly one such signature exists.

**Observed deviation:** JP2 validation accepts additional top-level JPEG 2000 signature boxes

A direct public parser unit test supplies owned, fully framed bytes and a standard immutable ResourceLimits::default(), with no forged internal state or bypassed owning caller. The complete original 167-byte fixture is verified/decoded successfully and its canonical 12-byte signature alone is duplicated, yielding 179 bytes, five top-level boxes and a 1x1 single-component image. Resource requirements are tiny relative to HARDENED limits. Signature uniqueness is the intended rejected property, not an entry requirement for this validation API; its assessed unit-contract requires list is empty.

**Recorded failure** (matched by the replay runner):

```text
second signature at box 2; malformed rejection, got Ok(Jp2File
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L5-L5`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L5)

```text
    let bytes = include_bytes!("../../validation/image-consumer-probe/tests/fixtures/gray.jp2");
```

[`src/jp2/spec_tests.rs:L68-L68`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L68)

```text
    assert!(parse(&parts.concat(), &ResourceLimits::default()).is_ok());
```

[`src/jp2/spec_tests.rs:L87-L95`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L87-L95)

```text
    for position in 2..=4 {
        let mut duplicate = parts.clone();
        duplicate.insert(position, parts[0].clone());
        spec_reject(
            &mut failures,
            &duplicate.concat(),
            format!("second signature at box {position}"),
        );
    }
```

[`src/jp2/spec_tests.rs:L47-L52`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L47-L52)

```text
    let result = parse(bytes, &ResourceLimits::default());
    spec_record(
        failures,
        matches!(result, Err(DecodeError::MalformedInput { .. })),
        format!("{}; malformed rejection, got {result:?}", context.into()),
    );
```

[`src/jp2/spec_tests.rs:L56-L60`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L56-L60)

```text
    assert!(
        failures.is_empty(),
        "clause violations:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2/spec_tests.rs`](support/tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs)
- [`jp2-spec-registration.patch`](support/tests/DEV-2D75F3E8F6/files/jp2-spec-registration.patch)
- [`validation/jp2_spec_runner.py`](support/tests/DEV-2D75F3E8F6/files/validation/jp2_spec_runner.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jp2-spec-registration.patch
```

```sh
python3 validation/jp2_spec_runner.py build
```

**Exact test invocation:**

```sh
python3 validation/jp2_spec_runner.py spec_signature_first_fixed_and_unique
```

[Machine-readable recipe](support/tests/DEV-2D75F3E8F6/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is spec_finish's failures.is_empty() at src/jp2/spec_tests.rs:56; its first recorded failed check is 'second signature at box 2; malformed rejection, got Ok(Jp2File ...'. This specifically refutes the clause's signature-uniqueness requirement, not merely an exit-code expectation. The later box-3 and box-4 checks reproduce the same omission. Entry realism: the repository's complete gray.jp2 fixture is 167 bytes; inserting its canonical 12-byte signature produces a 179-byte input with five top-level boxes, one 1x1 8-bit grayscale component, one codestream, two header children, and no association recursion. Top-level non-codestream framing totals 89 bytes. These are far below default HARDENED limits (2,147,483,648 input bytes, 16,777,216 metadata bytes, depth 32, 64 codestreams/components, 100,000,000 pixels, 1,000,000,000 work units). This is the same byte-slice/resource-limit entry shape used by real decode callers, using their normal tiny grayscale fixture, not an artificial private-state fixture. The unit-contract entry has no requires, and rejecting structurally invalid container bytes is the operation under test: a duplicate signature violates the target validation predicate, not a caller precondition. Harness setup also verifies canonical ftyp/header/framing, decodes the original fixture to its expected 1x1 sample, and successfully parses the original before adding the duplicate. Registration only includes tests within the existing cfg(test) module; production behavior is unchanged. Compiler describe_declaration confirms the &[u8], &ResourceLimits -> Result<Jp2File, DecodeError> signature. The control-flow graph and source show first-signature validation followed by a loop testing jp2h/jpch/jplh/comp/jp2c but not SIGNATURE; the duplicate falls into inspect_metadata_box, whose non-ASSOCIATION leaf path returns Ok((1, depth)). Parsing then reaches Ok(Jp2File), counting the duplicate as one metadata box. Standard authority was checked, not inferred from agreement with project tests/models: clause_origin is rfc_map, and read_rfc_section on its originating mapping confirms T.800 (2019-06) I.5.1 explicitly requires first position and 'one and only one' signature. This file-validity requirement applies to the advertised JP2 container-validation endpoint and the ordinary jp2-compatible fixture. It is not a claim that every permissive reader must reject every nonconforming file to meet reader conformance. The available standard edition is 2019, while the project lists a 2024 target; no edition reconciliation or whole-reader conformance claim is made. That traceability limitation is retained, but does not contradict the explicit cited rule or justify weakening this clause to match the implementation. The observed deviation is specifically acceptance contrary to that clause's validation guarantee. No resource/setup error or unsupported error category masks it: the host records successful parse results for the offending inputs.

[Standards quote, production lines, and provenance](support/evidence.md#dev-2d75f3e8f6)

</details>

</details>

<a id="reproduce-dev-056d9fbfb2"></a>
<details>
<summary>DEV-056D9FBFB2 — JP2 validation accepts duplicate File Type boxes as opaque metadata</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-056D9FBFB2 --output rfc-results/DEV-056D9FBFB2
```


**Standard:** t800-201906.txt § I.5.2

**Required behavior:** JP2 container validation shall require the File Type box immediately after the signature and validate that the file contains exactly one File Type box.

**Observed deviation:** JP2 validation accepts duplicate File Type boxes as opaque metadata

A safe direct call to the actual parse implementation with a fully owned 187-byte buffer and valid default ResourceLimits. The baseline 167-byte JP2 is independently exercised through decode_jp2, yielding one 1x1 component with sample 128, and through parse successfully. Only an intact 20-byte ftyp is duplicated; all lengths, the JP2 header, and the codestream remain unchanged. The constructed input is deliberately nonconforming in the exact cardinality being validated, but the parser API has no precondition that untrusted input already conform. No internal state is fabricated and no resource limit is approached.

**Recorded failure** (matched by the replay runner):

```text
second ftyp at box 2; malformed rejection, got Ok(Jp2File
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L17-L17`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L17)

```text
    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
```

[`src/jp2/spec_tests.rs:L27-L27`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L27)

```text
    let image = crate::decode::decode_jp2(bytes, &crate::DecodeOptions::default()).unwrap();
```

[`src/jp2/spec_tests.rs:L116-L124`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L116-L124)

```text
    for position in 2..=4 {
        let mut duplicate = parts.clone();
        duplicate.insert(position, parts[1].clone());
        spec_reject(
            &mut failures,
            &duplicate.concat(),
            format!("second ftyp at box {position}"),
        );
    }
```

[`src/jp2/spec_tests.rs:L46-L53`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L46-L53)

```text
fn spec_reject(failures: &mut Vec<String>, bytes: &[u8], context: impl Into<String>) {
    let result = parse(bytes, &ResourceLimits::default());
    spec_record(
        failures,
        matches!(result, Err(DecodeError::MalformedInput { .. })),
        format!("{}; malformed rejection, got {result:?}", context.into()),
    );
}
```

[`src/jp2/spec_tests.rs:L55-L61`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L55-L61)

```text
fn spec_finish(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "clause violations:\n{}",
        failures.join("\n")
    );
}
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2/spec_tests.rs`](support/tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs)
- [`jp2-spec-registration.patch`](support/tests/DEV-056D9FBFB2/files/jp2-spec-registration.patch)
- [`validation/jp2_spec_runner.py`](support/tests/DEV-056D9FBFB2/files/validation/jp2_spec_runner.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jp2-spec-registration.patch
```

```sh
python3 validation/jp2_spec_runner.py build
```

**Exact test invocation:**

```sh
python3 validation/jp2_spec_runner.py spec_file_type_immediate_and_unique
```

[Machine-readable recipe](support/tests/DEV-056D9FBFB2/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed check is the malformed-input rejection for 'second ftyp at box 2'; the final failures.is_empty() assertion aggregates that failure and equivalent duplicates at positions 3 and 4. Host replay reports Ok(Jp2File), not a setup failure or merely a nonzero exit. The entry is realistic: the 167-byte gray.jp2 fixture becomes 187 bytes after inserting one 20-byte ftyp, with five top-level boxes, two JP2-header children, one 1x1 8-bit component, one codestream, one compatible brand per ftyp, and metadata inspection depth 1. ResourceLimits::default() is HARDENED (2,147,483,648 input bytes, 16,777,216 metadata bytes, depth 32, 64 brands, 64 codestreams, and 1,000,000,000 work units); no large allocation, recursion, fixed pool or platform limit is approached. Real decoder interfaces also pass immutable input slices and ResourceLimits directly to jp2::parse. This is a valid direct unit-contract test: parse accepts untrusted byte slices for validation, not only already-conforming JP2 files. Deliberately duplicating a framed box is the validation target, not an inadmissible fabricated internal state. The unmodified fixture is decoded and its sample 128 verified before mutation, and the unmodified parse is checked for success. The test-only patch only includes the harness inside the existing test module. The compiler declaration confirms the slice/limits API; the MIR CFG confirms FILE_TYPE is checked for the second box only, later dispatch falls through to inspect_metadata_box (bb229-bb233), and the function ultimately returns Ok (bb282). Source inspection shows ftyp is not handled by any later dedicated branch and inspect_metadata_box immediately accepts non-association boxes. Thus the duplicate is counted as metadata, never checked for uniqueness, and a successful Jp2File is returned. The rfc_map origin was assessed rather than overridden by implementation intent: its parent finding cites T.800 (06/2019), I.5.2, and reading that section confirms both immediate placement and 'All files shall contain one and only one File Type box.' The fixture's jp2 brand and jp2 compatibility make this JP2 rule applicable. Together with the published JP2 'Container validation' scope and this clause's explicit validation requirement, the observed acceptance violates precisely the uniqueness guarantee. This does not claim the standard requires every reader to reject every malformed input, nor establish conformance to a later edition; the origin's unreconciled-edition qualification remains. There is no standards-specific contrary evidence supporting spec_error here. The malformed-error variant oracle does not affect this attribution because the actual result is Ok, which fails any rejection oracle.

[Standards quote, production lines, and provenance](support/evidence.md#dev-056d9fbfb2)

</details>

</details>

<a id="reproduce-dev-629a0a9c39"></a>
<details>
<summary>DEV-629A0A9C39 — JP2 header rejects reserved-method later colr boxes that the reader must ignore</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-629A0A9C39 --output rfc-results/DEV-629A0A9C39
```


**Standard:** t800-201906.txt § I.5.3.3

**Required behavior:** In JP2-reader mode, after selecting the first Colour Specification box, ignore later Colour Specification boxes rather than requiring their method-specific semantic parsing to succeed.

**Observed deviation:** JP2 header rejects reserved-method later colr boxes that the reader must ignore

The tested entry is the explicitly available direct unit contract, with initialized safe Rust byte vectors and ResourceLimits::default(), not fabricated parser internals. ihdr is 13x17, three components, unsigned depth8, compression7, known colourspace/no-IP flags. Its first colr is METH1, PREC0, APPROX0, EnumCS16 sRGB, matching three unsigned channels. All later colr boxes are contiguous and correctly framed; the reserved box has the three common fields and only permitted opaque trailing bytes. Four boxes consume at most 94 bytes and four reader work units with no recursive nesting, comfortably within all applicable default limits. The selected base=40 case is within the input limit and has no offset overflow. The pinned 2019 standard explicitly requires readers to ignore these reserved methods, so these bytes are admissible for the reader-ignore obligation even though a JP2 writer is instructed to emit METH1 or METH2. Earlier successful baseline parsing establishes the entry's unaffected metadata is accepted.

**Recorded failure** (matched by the replay runner):

```text
method=255 extra=0 base=40: expected ignored colr, got MalformedInput { offset: 77, kind: InvalidColorSpecification }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_header_spec_tests.rs:L112-L115`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L112-L115)

```text
        let mut baseline = spec_ihdr(13, 17, 3, 7);
        baseline.extend(spec_colr());
        let expected = parse_jp2_header(&baseline, &limits, base).unwrap();
        assert_eq!(expected.color, ColorSpecification::enumerated(16));
```

[`src/jp2_header_spec_tests.rs:L111-L111`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L111)

```text
    for base in [0, 40, 1 << 32] {
```

[`src/jp2_header_spec_tests.rs:L116-L118`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L116-L118)

```text
        for method in [0, 4, 5, 127, 254, 255] {
            for extra in [0, 1, 7, 31] {
                let mut ignored = vec![method, 0, 0];
```

[`src/jp2_header_spec_tests.rs:L127-L129`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L127-L129)

```text
                let mut bytes = baseline.clone();
                bytes.extend(encoded);
                bytes.extend(spec_colr());
```

[`src/jp2_header_spec_tests.rs:L133-L133`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L133)

```text
                match parse_jp2_header(&bytes, &limits, base) {
```

[`src/jp2_header_spec_tests.rs:L140-L142`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L140-L142)

```text
                    Err(error) => failures.push(format!(
                        "method={method} extra={extra} base={base}: expected ignored colr, got {error:?}"
                    )),
```

[`src/jp2_header_spec_tests.rs:L162-L166`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L162-L166)

```text
    assert!(
        failures.is_empty(),
        "JP2 reader ignore predicate violated:\n{}",
        failures.join("\n")
    );
```

[`src/jp2_header_spec_tests.rs:L10-L12`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L10-L12)

```text
fn spec_colr() -> Vec<u8> {
    box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16])
}
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_header_spec_tests.rs`](support/tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs)
- [`validation/build-jp2-header-spec-tests.py`](support/tests/DEV-629A0A9C39/files/validation/build-jp2-header-spec-tests.py)

**Build commands** (in order):

```sh
python3 validation/build-jp2-header-spec-tests.py
```

**Exact test invocation:**

```sh
target/jp2-header-spec-tests --exact jp2::tests::spec_header_ignores_later_colour_semantics --nocapture
```

[Machine-readable recipe](support/tests/DEV-629A0A9C39/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the final assert!(failures.is_empty()) in spec_header_ignores_later_colour_semantics, not a fixture panic, build failure, or unexplained nonzero exit. The host output identifies concrete parse_jp2_header calls returning InvalidColorSpecification instead of ignoring a later colr. In particular method=255, extra=0, base=40 returns offset=77, exactly base+37, the second colr's header offset after the 22-byte ihdr and 15-byte first colr. The baseline parse and selected-sRGB assertion precede the failing variants and succeed; the negative checks also complete before the aggregated assertion.

[Standards quote, production lines, and provenance](support/evidence.md#dev-629a0a9c39)

</details>

</details>

<a id="reproduce-dev-2032f100c3"></a>
<details>
<summary>DEV-2032F100C3 — JP2 palette validator accepts NE above the T.800 (06/2019) limit of 1024</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-2032F100C3 --output rfc-results/DEV-2032F100C3
```


**Standard:** t800-201906.txt § I.5.3.4

**Required behavior:** JP2 palette validation against the local T.800 (06/2019) contract should admit NE only in 1..1024, while preserving the validated entry count and complete table.

**Observed deviation:** JP2 palette validator accepts NE above the T.800 (06/2019) limit of 1024

The assessed parser unit entry has no requires. All inputs are initialized safe Rust values with valid shared borrows. Each negative fixture has NPC=1, a supported unsigned-eight-bit descriptor, exact normal 8-byte pclr framing and a complete NE-byte table, without padding or truncation. Header fields are produced by BoxReader rather than fabricated. At NE=1025 there are 1029 payload bytes and 1037 box bytes. The largest case is 65547 box bytes, under default input/metadata limits; a single box requires one framing work unit and no recursion. Only the tested NE validation predicate is false. No callee receives an invalid precision, width, offset, or header. The sole source patch registers test code under cfg(test), preserving production behavior.

**Recorded failure** (matched by the replay runner):

```text
T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted [1025, 1026, 2048, 32768, 65535]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_palette_contract_tests.rs:L64-L65`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L64-L65)

```text
let mut reader = BoxReader::new(bytes, &ResourceLimits::default()).unwrap();
    let box_ref = reader.next_box_ref().unwrap().unwrap();
```

[`src/jp2_palette_contract_tests.rs:L76-L77`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L76-L77)

```text
assert!(reader.next_box_ref().is_none());
    parse_palette(box_ref, 0)
```

[`src/jp2_palette_contract_tests.rs:L179-L179`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L179)

```text
for entries in [0, 1025, 1026, 2048, 32768, u16::MAX] {
```

[`src/jp2_palette_contract_tests.rs:L192-L196`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L192-L196)

```text
Ok(palette) => {
                contract_assert_retained_palette(&palette, entries, &columns, &expected);
                wrongly_admitted.push(entries);
                eprintln!("predicate violated: admitted forbidden NE={entries}");
            }
```

[`src/jp2_palette_contract_tests.rs:L199-L202`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L199-L202)

```text
assert!(
        wrongly_admitted.is_empty(),
        "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted {wrongly_admitted:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_palette_contract_tests.rs`](support/tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs)
- [`validation/palette-contract-tests.patch`](support/tests/DEV-2032F100C3/files/validation/palette-contract-tests.patch)

**Build commands** (in order):

```sh
patch --batch --fuzz=0 -p1 -i validation/palette-contract-tests.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jp2.rs'"'"').read_text(); t=Path('"'"'src/jp2_palette_contract_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jp2_palette_contract_tests.rs");'"'"') == 1; assert '"'"'fn validates_palette_mapping_relationships()'"'"' in s; assert '"'"'fn retains_signed_palette_and_channel_definition_records()'"'"' in s; assert '"'"'fn palette_contract_wire_order_precision_and_signedness()'"'"' in t; assert '"'"'fn palette_contract_t800_entry_count_domain_and_complete_table()'"'"' in t; print('"'"'Verified test-only registration and both clause assertions in replay source'"'"')'
```

```sh
cargo test --lib --locked --no-run
```

**Exact test invocation:**

```sh
cargo test --lib --locked jp2::tests::palette_contract_t800_entry_count_domain_and_complete_table -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-2032F100C3/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is wrongly_admitted.is_empty(), with host output: "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted [1025, 1026, 2048, 32768, 65535]". All preceding legal-count retention checks passed. This specifically refutes the NE admission part of clause e8e89fffa5faef739c2121f3, not table preservation. The host also explicitly logged acceptance of NE=1025, so attribution does not rest on cargo's nonzero exit alone.

[Standards quote, production lines, and provenance](support/evidence.md#dev-2032f100c3)

</details>

</details>

<a id="reproduce-dev-3db90259d8"></a>
<details>
<summary>DEV-3DB90259D8 — cdef parser rejects valid RGB colour associations exceeding physical channel count</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-3DB90259D8 --output rfc-results/DEV-3DB90259D8
```


**Standard:** t800-201906.txt § I.5.3.6

**Required behavior:** Preserve the distinction between a cdef channel index and its color association: validate the former against output channels and interpret the latter as a color-space index or special association value, without requiring the color index to fit the physical channel count.

**Observed deviation:** cdef parser rejects valid RGB colour associations exceeding physical channel count

Admissible direct unit input: genuine framed cdef BoxRef; N=3 and exact 2+6*N payload; one physical output channel with every Cni=0; supported Color type; RGB associations 1,2,3 each represented exactly once; unsigned 1x1 component, sRGB colr, no cmap/palette/MCT, one cdef after ihdr. The pinned T.800 section expressly permits multiple descriptions for one channel and separates colour indices from physical channel indices. Sizes are far below default resource limits. There is no callee precondition requiring the association predicate to return true, and no owning-caller-only state is needed by this pure parser boundary. This is not a claim of end-to-end file reachability or complete-file conformance.

**Recorded failure** (matched by the replay runner):

```text
channels=1: RGB colour indices 1,2,3 must be retained independently of channel count: Err(MalformedInput { offset: 110, kind: InvalidBoxContents })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`cdef-spec-tests.patch:L156-L162`](support/tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L156-L162)

```text
+        for channels in [1_u16, 2, 3, 4, 8, 64] {
+            // Sharing all RGB colours makes cdef necessary even with one channel.
+            let mut descriptions = vec![
+                (0, ChannelType::Color, ChannelAssociation::Color(1)),
+                (0, ChannelType::Color, ChannelAssociation::Color(2)),
+                (0, ChannelType::Color, ChannelAssociation::Color(3)),
+            ];
```

[`cdef-spec-tests.patch:L41-L47`](support/tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L41-L47)

```text
+        let mut header = box32(*b"ihdr", &ihdr);
+        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
+        header.extend(box32(*b"cdef", &payload));
+        let boxes = BoxReader::new(&header, &ResourceLimits::default())
+            .unwrap()
+            .collect::<Result<Vec<_>, _>>()
+            .unwrap();
```

[`cdef-spec-tests.patch:L60-L60`](support/tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L60)

```text
+        parse_channel_definition(boxes[2], channels, 73)
```

[`cdef-spec-tests.patch:L225-L225`](support/tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L225)

```text
+        assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`cdef-spec-tests.patch`](support/tests/DEV-3DB90259D8/files/cdef-spec-tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i cdef-spec-tests.patch
```

```sh
sh -eu -c 'grep -Fq '"'"'fn spec_cdef_retains_every_admitted_description()'"'"' src/jp2.rs; grep -Fq '"'"'fn spec_cdef_separates_channel_and_colour_domains()'"'"' src/jp2.rs; grep -Fq '"'"'failures.join("\n")'"'"' src/jp2.rs; cargo test --lib --locked --no-run --target-dir target/deepspec-cdef; set -- $(find target/deepspec-cdef/debug/deps -maxdepth 1 -type f -name '"'"'jpeg2000_rs-*'"'"' -executable); test "$#" -eq 1; cp "$1" target/deepspec-cdef/cdef-tests; target/deepspec-cdef/cdef-tests --list | grep -Fx '"'"'jp2::tests::spec_cdef_retains_every_admitted_description: test'"'"'; target/deepspec-cdef/cdef-tests --list | grep -Fx '"'"'jp2::tests::spec_cdef_separates_channel_and_colour_domains: test'"'"''
```

**Exact test invocation:**

```sh
target/deepspec-cdef/cdef-tests --exact jp2::tests::spec_cdef_separates_channel_and_colour_domains --nocapture
```

[Machine-readable recipe](support/tests/DEV-3DB90259D8/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the accumulated channels=1 exact-record expectation: the parser returned InvalidBoxContents at offset 110 instead of retaining channel 0's RGB associations 1, 2 and 3. This is a valid direct unit-contract entry, not an invalid fixture or a bypass of a required owning caller. Resource realism: the first witness has a 1x1 unsigned 8-bit image, one physical component, N=3, a 20-byte cdef payload and 65-byte header payload. The failing case's largest fixture has 64 components, N=67 and a 449-byte header payload, with only three flat boxes and heap-backed vectors. Default limits are HARDENED: 64 components, 16,777,216 metadata bytes, 2,147,483,648 input bytes and depth 32. No excessive allocation, stack nesting or platform limit explains this failure. The separate passing retention case's N=65535 is not needed for this diagnosis.

[Standards quote, production lines, and provenance](support/evidence.md#dev-3db90259d8)

</details>

</details>

<a id="reproduce-dev-457acab2c3"></a>
<details>
<summary>DEV-457ACAB2C3 — Ordinary JP2 rejects a resource-admitted second contiguous codestream instead of allowing first-codestream display</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-457ACAB2C3 --output rfc-results/DEV-457ACAB2C3
```


**Standard:** t800-201906.txt § I.5.4

**Required behavior:** In support of ordinary JP2 display, the container reader shall permit later well-framed Contiguous Codestream boxes after the JP2 Header and allow display of the first codestream while ignoring subsequent codestreams, subject to resource admission.

**Observed deviation:** Ordinary JP2 rejects a resource-admitted second contiguous codestream instead of allowing first-codestream display

Public parse accepts immutable bytes and a validated ResourceLimits without requiring an owning caller or private object invariants. The test uses ResourceLimits::default().with_maximum_codestreams(8).unwrap(), a 283-byte ordinary JP2 with two complete separately decoded repository codestreams, canonical signature/ftyp/header, valid lengths and legal post-header placement. Its first gray codestream matches the JP2 image header. The second complete RGB stream is intentionally distinct so first-stream display preservation is meaningful; T.800 I.5.4 says subsequent streams are ignored for display. Two streams are within eight, and all byte, depth, metadata, geometry, and work demands are small. No fixture setup assertion failed in host replay.

**Recorded failure** (matched by the replay runner):

```text
jp2::parse: ordinary JP2 accepts 2 complete well-framed codestreams within limit 8; Err(MalformedInput { offset: 176, kind: InvalidBoxContents }): false
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L308-L310`](support/tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L308-L310)

```text
    let limits = ResourceLimits::default()
        .with_maximum_codestreams(8)
        .unwrap();
```

[`src/jp2/spec_tests.rs:L312-L323`](support/tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L312-L323)

```text
    for count in [1, 2, 3, 7, 8] {
        let mut input = parts[..3].to_vec();
        for index in 0..count {
            input.push(if index == 0 {
                parts[3].clone()
            } else {
                rgb[77..].to_vec()
            });
            input.push(box32(*b"zzzz", &[0xff]));
        }
        let bytes = input.concat();
        let result = parse(&bytes, &limits);
```

[`src/jp2/spec_tests.rs:L326-L328`](support/tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L326-L328)

```text
            result.as_ref().is_ok_and(|file| {
                file.codestream_count() >= 1 && file.codestreams()[0].data() == &parts[3][8..]
            }),
```

[`src/jp2/spec_tests.rs:L16-L17`](support/tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L16-L17)

```text
    assert_eq!(parts[0], b"\0\0\0\x0cjP  \r\n\x87\n");
    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2/spec_tests.rs`](support/tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs)
- [`jp2-spec-registration.patch`](support/tests/DEV-457ACAB2C3/files/jp2-spec-registration.patch)
- [`validation/jp2_spec_runner.py`](support/tests/DEV-457ACAB2C3/files/validation/jp2_spec_runner.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jp2-spec-registration.patch
```

```sh
python3 validation/jp2_spec_runner.py build
```

**Exact test invocation:**

```sh
python3 validation/jp2_spec_runner.py spec_later_jp2_codestreams_preserve_first_display
```

[Machine-readable recipe](support/tests/DEV-457ACAB2C3/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed predicate is 'ordinary JP2 accepts 2 complete well-framed codestreams within limit 8', reporting InvalidBoxContents at offset 176. This is a valid direct unit-contract entry to public parse(&[u8], &ResourceLimits), not a fabricated internal state or a fixture bypassing an owning caller. Resource realism: the two-stream input is 283 bytes: 77 bytes of signature/ftyp/jp2h, a 90-byte gray jp2c, a 9-byte opaque box, a 98-byte RGB jp2c, and another 9-byte opaque box. The largest admitted sweep input is 925 bytes with eight streams and eight shallow opaque boxes; the nine-stream negative control is 887 bytes. These are far below the 2 GiB input, 16 MiB metadata, depth 32, and one-billion work-unit ceilings. The configured stream limit is explicitly eight. Each fixture is a complete repository 1x1 codestream; the replay first successfully decodes the gray and RGB originals and checks their samples. The first stream and JP2 header match; subsequent standalone complete RGB streams need not determine the first image's component count. The signature, ordinary jp2 brand/compatibility, header placement and box extents are asserted by the replayed harness. No large allocation, excessive nesting, private-state manipulation, or hidden pool/stack limit explains this small input. The real decode_jp2 caller passes the same input and limits through parse_and_inspect, so this also blocks its ordinary JP2 display route.

[Standards quote, production lines, and provenance](support/evidence.md#dev-457acab2c3)

</details>

</details>

<a id="reproduce-dev-8825e34c8b"></a>
<details>
<summary>DEV-8825E34C8B — Inherited main-header component NLT overrides first-tile-header default</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-8825E34C8B --output rfc-results/DEV-8825E34C8B
```


**Standard:** t801-202106.txt § A.3.10

**Required behavior:** Select a retained component-specific NLT before a retained default, as documented by the current local model; standards-compliant use must additionally preserve tile-header precedence over main-header selectors.

**Observed deviation:** Inherited main-header component NLT overrides first-tile-header default

Parser-created owned receiver; valid component 0 of three; first and only tile-part; unique per-header selectors; bounded native 64x32 fixture; Rsiz=0x8200; legal Tnlt=2 LUT payloads and marker lengths. No production modification, manual merged-registry construction, skipped owning parser, or violation of subject requires. This establishes admissible unit entry and selection failure, not full decoder execution.

**Recorded failure** (matched by the replay runner):

```text
parse_layout -> first tile extensions -> nonlinear_transform_for: Rsiz=0x8200, main=[(Some(0), 9), (Some(2), 9)], tile=[(Some(2), 11), (None, 10)], component=0, expected=Some(NonlinearPointTransform { component: None, precision: 10, signed: false, transform_type: 2, parameters: [0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255], d_min: Some(0), d_max: Some(4294967295), point_bits: Some(8), points: [0, 255], gamma_parameters: None }), actual=Some(NonlinearPointTransform { component: Some(0), precision: 9, signed: false, transform_type: 2, parameters: [0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255], d_min: Some(0), d_max: Some(4294967295), point_bits: Some(8), points: [0, 255], gamma_parameters: None })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`validation/nonlinear_lookup_contract.rs:L79-L79`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L79)

```text
            bytes[6..8].copy_from_slice(&0x8200_u16.to_be_bytes());
```

[`validation/nonlinear_lookup_contract.rs:L85-L85`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L85)

```text
            let layout = parse_layout(&bytes, &limits).expect("bounded, capability-signalled NLT header fixture must parse");
```

[`validation/nonlinear_lookup_contract.rs:L91-L91`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L91)

```text
            let owner = &layout.tile_parts[0].part2_extensions;
```

[`validation/nonlinear_lookup_contract.rs:L94-L97`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L94-L97)

```text
                let expected_entry = tile.iter().find(|(s, _)| *s == Some(component))
                    .or_else(|| tile.iter().find(|(s, _)| s.is_none()))
                    .or_else(|| main.iter().find(|(s, _)| *s == Some(component)))
                    .or_else(|| main.iter().find(|(s, _)| s.is_none()));
```

[`validation/nonlinear_lookup_contract.rs:L99-L99`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L99)

```text
                let actual = owner.nonlinear_transform_for(component);
```

[`validation/nonlinear_lookup_contract.rs:L109-L109`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L109)

```text
    assert!(failures.is_empty(), "NLT header precedence violations: {failures:#?}");
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`validation/nonlinear_lookup_contract.rs`](support/tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs)
- [`validation/build_nonlinear_lookup_contract.py`](support/tests/DEV-8825E34C8B/files/validation/build_nonlinear_lookup_contract.py)

**Build commands** (in order):

```sh
python3 validation/build_nonlinear_lookup_contract.py
```

**Exact test invocation:**

```sh
target/nonlinear-lookup-contract --exact codestream::tests::nonlinear_lookup_standard_header_precedence --nocapture
```

[Machine-readable recipe](support/tests/DEV-8825E34C8B/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the standards case's final assert!(failures.is_empty()), after parsing and owner-preservation checks and the 192-observation count. Its recorded mismatches show a main-header component-specific precision-9 descriptor returned instead of the tile-default precision-10 descriptor. This violates the standards-precedence half of clause 32aa5ed6bcd40eed56feecfe, not its accurately documented local specific-before-default rule. I read clause_origin (rfc_map) and re-read T.801 (06/2021) A.3.10 and A.2.1 using origin a8d4c31de8ce4de63071ab43: A.3.10 explicitly says 'Tile-part NLT > Tile-part NLT default > Main NLT > Main NLT default', and A.2.1 confirms Rsiz=0x8200 signals Part 2 and NLT without CAP. The fixture meets the applicable conditions: main and first tile-part headers only, unique selectors per header, component indices 0 and 2 within three components, optional 65535 default, legal unsigned depths 8–11, Tnlt=2 with two LUT values and Lnlt=19, Dmin=0/Dmax=4294967295 and PTval=8. Before tracing the implementation I checked entry scale: each of the 64 independently parsed inputs has one 64x32 image (2048 pixels, 6144 component samples), three components, one tile/first tile-part, one layer/resolution, three empty packet headers, at most six 21-byte NLT segments, and at most 216 input bytes. These are far below default limits (2 GiB input, 16 MiB metadata, 100 million pixels, 64 components, 16384 tiles/parts); no deep recursion or fabricated pool/stack state is involved. Real collect_nonlinear_regions also takes the tile's parser-produced extensions and queries the same component indices. This is a valid direct unit-contract call with no extra subject requires, not a claim that the harness executes collect_nonlinear_regions or reconstructs pixels. The owning parser is actually run: first tile extensions inherit main extensions; parse_nlt replaces only matching Option<u16> selectors, so a tile default leaves inherited main specifics intact. describe_declaration and the MIR CFG confirm the lookup's first scan returns an inherited specific through bb4 -> bb5 -> bb6 -> bb19, never reaching the default scan. Consequence: the selected descriptor/precision comes from the wrong header. The output tail truncates the panic heading and earliest mismatch, so I do not identify the first failing combination or claim a mismatch count; it nevertheless retains complete expected/actual route records and the failing native test result. Local-model agreement does not invalidate this standards-derived requirement. Edition scope remains the acquired June 2021 T.801 text, not uninspected newer editions; no numerical LUT or end-to-end image deviation is asserted.

[Standards quote, production lines, and provenance](support/evidence.md#dev-8825e34c8b)

</details>

</details>

<a id="reproduce-dev-49c7bafde3"></a>
<details>
<summary>DEV-49C7BAFDE3 — Reversible decorrelation resolver rejects normative N(N+1) coefficient arrays using an N² count</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-49C7BAFDE3 --output rfc-results/DEV-49C7BAFDE3
```


**Standard:** t801-202106.txt § J.3.1.1.3

**Required behavior:** Resolve array-backed collections according to the documented graph-collection model, rejecting coefficient vectors whose length differs from the computed expected element count and enforcing reversible precision checks.

**Observed deviation:** Reversible decorrelation resolver rejects normative N(N+1) coefficient arrays using an N² count

A valid direct unit entry, not a claimed complete codestream. Parser is initialized with empty input and HARDENED limits through new_parser; parse_mct accepts explicit complete unfragmented segments with Zmct=Ymct=0. For the quoted case there are six unique input components and six permuted unique output components, matching nonzero decorrelation index 7, no offsets selected, 42 Int16 integer coefficients, zero fractional precision, scale factors 2 and final scale 4. These satisfy the retrieved 2021 J.3.1.1.3 conditions and native caller/reference/lookup contracts. Six components and 42 elements are far below caps of 64 and 4096, respectively; all tested sizes remain within those caps. The resolver does not require the parser input to contain the separately supplied arrays, nor an N*N array as an entry premise.

**Recorded failure** (matched by the replay runner):

```text
direct resolve_mct_collection: J.3.1.1.3 N=6, integer coefficients=42, offsets=false: expected full collection, got Err(MalformedInput { offset: 0, kind: InvalidTransform })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/resolve_mct_contract.rs:L9-L10`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L9-L10)

```text
let limits = ResourceLimits::HARDENED;
    let parser = new_parser(&[], &limits).unwrap();
```

[`src/codestream/resolve_mct_contract.rs:L19-L20`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L19-L20)

```text
let count = (n + 1) * n;
        assert!(count <= usize::from(limits.maximum_components()).pow(2));
```

[`src/codestream/resolve_mct_contract.rs:L26-L27`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L26-L27)

```text
for row in 0..n { coefficients[row * n + n - 1 - row] = 2; }
        coefficients[count - 1] = 4;
```

[`src/codestream/resolve_mct_contract.rs:L39-L39`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L39)

```text
parser.parse_mct(Segment { marker: MCT, offset: 0, payload: &bytes[4..] }).unwrap()
```

[`src/codestream/resolve_mct_contract.rs:L65-L65`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L65)

```text
kind: MctArrayKind::Decorrelation, reversible: true, transform_index: 7, offset_index: 0,
```

[`src/codestream/resolve_mct_contract.rs:L76-L80`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L76-L80)

```text
let actual = parser.resolve_mct_collection(&reference, &arrays);
            checks += 1;
            if actual != expected {
                failures.push(format!("direct resolve_mct_collection: J.3.1.1.3 N={n}, integer coefficients={count}, offsets={with_offsets}: expected full collection, got {actual:?}"));
            }
```

[`src/codestream/resolve_mct_contract.rs:L113-L113`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L113)

```text
assert!(failures.is_empty(), "{} discriminating checks failed:\n{}", failures.len(), failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/resolve_mct_contract.rs`](support/tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs)

**Build commands** (in order):

```sh
bash -c 'printf '"'"'\ninclude!("resolve_mct_contract.rs");\n'"'"' >> src/codestream/tests.rs && cargo test --locked --lib --no-run --target-dir target/deepspec-mct'
```

**Exact test invocation:**

```sh
bash -c 'for bin in target/deepspec-mct/debug/deps/jpeg2000_rs-*; do if test -x "$bin" && ! test -d "$bin"; then exec "$bin" --exact codestream::tests::resolve_mct_collection_clause_7ca4daea --nocapture; fi; done; exit 2'
```

[Machine-readable recipe](support/tests/DEV-49C7BAFDE3/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

This is a reproduced unit-contract code deviation for the standards-derived meaning of clause 7ca4daea027df926ca127cca, not an instruction to rewrite its oracle to match the Verus model. I read clause_origin (rfc_map b091a9c63532ebcacba21070, finding a72486651c3b6f2a46bb7fdec2e050397216c308f97f94dbb365476614475380) and retrieved its original T.801 (06/2021) J.3.1.1.3 section using read_rfc_section with the origin item. Outside the informative factorization note, it explicitly requires (N+1)*N coefficients for a nonzero reversible decorrelation array index, equal nonempty input/output cardinalities, integer coefficients and power-of-two scale factors. The positive harness meets these conditions. Agreement of the native model test and implementation on N*N cannot override this normative count. Qualification: project REFERENCES.md selects the 2023 edition and explicitly leaves its reconciliation with the acquired 2021 edition open. This diagnosis establishes the admission mismatch against the cited 2021 authority; it does not assert verified 2023 conformance differences or complete numerical transform correctness.

[Standards quote, production lines, and provenance](support/evidence.md#dev-49c7bafde3)

</details>

</details>

<a id="reproduce-dev-61c38e5752"></a>
<details>
<summary>DEV-61C38E5752 — Inverse LUT NLT normalizes reconstructed samples by 2^precision instead of 2^precision−1</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-61C38E5752 --output rfc-results/DEV-61C38E5752
```


**Standard:** t801-202106.txt § K

**Required behavior:** Produce typed decoder output by applying admitted inverse nonlinear transformations after reconstructed samples and inverse component processing, preserving the selected reconstruction's geometry.

**Observed deviation:** Inverse LUT NLT normalizes reconstructed samples by 2^precision instead of 2^precision−1

A bounded valid direct unit entry, not an entropy-decoding claim: 3 equal four-bit source components, equal subsampling, one tile, matching selected reconstruction bounds and sample counts, ResourceLimits::HARDENED. Component 0's type-2 NLT has legal precision/sign, two points [255,0], PTval8, Dmin0 and Dmax=u32::MAX (normalized 0 and1); the NLT capability is signaled and main-header metadata is inherited into the only tile. The quoted route uses fraction_bits16 with exact integer-centered reconstruction and inverse_rct=false. Source rank0..15 corresponds to signed -8..7, entirely inside the source range. Existing assembly and typed conversion perform their native checks. Test access is a cfg(test) overlay that includes decode.rs unchanged.

**Recorded failure** (matched by the replay runner):

```text
typed_layout source_signed=true target_signed=false region=[3, 1, 35, 9] subsampling=1 fraction_bits=16 inverse_rct=false active=true component=0: actual=[255, 239, 223, 207, 191, 175, 159, 143, 128, 112, 96, 80, 64, 48, 32, 16] expected=[255, 238, 221, 204, 187, 170, 153, 136, 119, 102, 85, 68, 51, 34, 17, 0]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode_contract_overlay.rs:L2-L2`](support/tests/DEV-61C38E5752/files/src/decode_contract_overlay.rs#L2)

```text
include!("decode.rs");
```

[`src/typed_layout_contract_tests.rs:L153-L154`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L153-L154)

```text
    let mut decoded = assemble_selected_image(size, &tiles, region, 0, &ResourceLimits::HARDENED)
        .expect("valid reconstructed selected-image fixture");
```

[`src/typed_layout_contract_tests.rs:L296-L296`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L296)

```text
                for (fraction_bits, inverse_rct) in [(0, false), (16, false), (0, true)] {
```

[`src/typed_layout_contract_tests.rs:L324-L326`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L324-L326)

```text
                        let output = typed_layout(&layout, decoded, &limits);
                        let output = output.expect(&route);
                        assert_geometry(&output, &geometry, region);
```

[`src/typed_layout_contract_tests.rs:L310-L310`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L310)

```text
                                *value = 255 - 17 * rank - if target_signed { 128 } else { 0 };
```

[`src/typed_layout_contract_tests.rs:L342-L347`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L342-L347)

```text
    assert!(
        discrepancies.is_empty(),
        "standards-derived NLT mismatch ({} scenarios):\n{}",
        discrepancies.len(),
        discrepancies.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode_contract_overlay.rs`](support/tests/DEV-61C38E5752/files/src/decode_contract_overlay.rs)
- [`src/typed_layout_contract_tests.rs`](support/tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs)
- [`src/lib.rs`](support/tests/DEV-61C38E5752/files/src/lib.rs)

**Build commands** (in order):

```sh
python3 -c 'import json, shutil, subprocess, sys
p = subprocess.run(['"'"'cargo'"'"', '"'"'test'"'"', '"'"'--lib'"'"', '"'"'--no-run'"'"', '"'"'--message-format=json'"'"'], stdout=subprocess.PIPE, text=True)
if p.returncode:
    print(p.stdout)
    sys.exit(p.returncode)
artifacts = [json.loads(line) for line in p.stdout.splitlines() if line.startswith('"'"'{'"'"')]
[executable] = [a['"'"'executable'"'"'] for a in artifacts if a.get('"'"'reason'"'"') == '"'"'compiler-artifact'"'"' and a.get('"'"'executable'"'"') and a['"'"'target'"'"']['"'"'name'"'"'] == '"'"'jpeg2000_rs'"'"' and a['"'"'profile'"'"']['"'"'test'"'"']]
shutil.copy2(executable, '"'"'target/typed-layout-contract'"'"')'
```

**Exact test invocation:**

```sh
target/typed-layout-contract decode::typed_layout_contract_tests::typed_layout_inverse_nlt_after_reconstruction_preserves_geometry --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-61C38E5752/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the reconstruction test's final assert!(discrepancies.is_empty()), not fixture construction, resource exhaustion, or a geometry assertion. The retained host output contains concrete component-0 actual/expected sample mismatches. For example, signed four-bit reconstructed samples -8..7 converted to unsigned eight-bit output through the two-point descending LUT produce [255,239,...,16], instead of [255,238,...,0]. Geometry checks precede discrepancy collection and did not fail. Entry realism was checked first: the largest selection is 32x8, with three components and at most 768 total reconstructed i64 samples (6144 bytes of raw sample payload); a single tile, shallow fixed loops, and two LUT points are far below HARDENED limits (64 components, 100000000 pixels, 2147483648 decoded bytes). No implicit stack/pool stress or implausible count is involved. The normal caller decode_typed_layout also passes a full or selected reconstructed DecodedImage into typed_layout. This is a valid direct unit test of that stage, not a claim that arbitrary samples were decoded from the fixture's empty packets: parse_layout supplies the admitted layout, assemble_selected_image constructs matching selected component geometry, and the optional real inverse reversible component transform runs before typed_layout. The failing fraction_bits=16, inverse_rct=false route needs no component-transform assumptions: centered integer samples are multiplied by 2^16 and typed conversion restores valid signed four-bit values. The test-only overlay includes the unchanged decode.rs and merely registers a child module.

[Standards quote, production lines, and provenance](support/evidence.md#dev-61c38e5752)

</details>

</details>

<a id="reproduce-dev-afc4dede80"></a>
<details>
<summary>DEV-AFC4DEDE80 — JPX Association inspection accepts fewer than two immediate children</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-AFC4DEDE80 --output rfc-results/DEV-AFC4DEDE80
```


**Standard:** t801-202106.txt § M.11.11

**Required behavior:** When validating an Association superbox as JPX metadata, require at least two immediate child boxes before returning a successful structural inspection; recursion and aggregate node counts do not replace immediate-child cardinality.

**Observed deviation:** JPX Association inspection accepts fewer than two immediate children

A direct unit-contract entry has no requires predicates. The BoxRef is reader-produced, not forged; LBox is 8 with empty Association payload. Default limits admit eight bytes and depth 1; the recursive budget is u32::MAX-1. Empty child cardinality is intentionally invalid input for the structural validator, precisely the condition whose rejection the clause requires, not a violated separate caller/callee premise. Production callers pass the same depth/budget for metadata roots. Larger controls remain below all capacities and use unrestricted label/XML/Association children.

**Recorded failure** (matched by the replay runner):

```text
empty at depth 1: immediate=0, expected=None, observed=Ok((1, 1))
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_metadata_cardinality_test.rs:L17-L17`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L17)

```text
("empty".to_owned(), vec![], None),
```

[`src/jp2_metadata_cardinality_test.rs:L9-L9`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L9)

```text
let association = |children: &[Vec<u8>]| box32(*b"asoc", &children.concat());
```

[`src/jp2_metadata_cardinality_test.rs:L75-L79`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L75-L79)

```text
let mut reader = BoxReader::new(&bytes, &limits).unwrap();
            let root = reader.next_box_ref().unwrap().unwrap();
            assert_eq!(root.header.box_type(), BoxType::ASSOCIATION);
            assert_eq!(root.header.total_length(), bytes.len() as u64);
            assert!(reader.next_box_ref().is_none());
```

[`src/jp2_metadata_cardinality_test.rs:L88-L88`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L88)

```text
let observed = inspect_metadata_box(root, &limits, depth, u32::MAX - depth);
```

[`src/jp2_metadata_cardinality_test.rs:L98-L98`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L98)

```text
None => matches!(observed, Err(DecodeError::MalformedInput { .. })),
```

[`src/jp2_metadata_cardinality_test.rs:L110-L114`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L110-L114)

```text
assert!(
        failures.is_empty(),
        "M.11.11 immediate-child cardinality violations:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_metadata_cardinality_test.rs`](support/tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs)
- [`validation/metadata-cardinality-registration.patch`](support/tests/DEV-AFC4DEDE80/files/validation/metadata-cardinality-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/metadata-cardinality-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jp2.rs'"'"').read_text(); assert s.count('"'"'    include!("jp2_metadata_cardinality_test.rs");'"'"') == 1; assert '"'"'fn retains_general_layer_cdef_and_nested_metadata()'"'"' in s; t=Path('"'"'src/jp2_metadata_cardinality_test.rs'"'"').read_text(); assert '"'"'fn association_requires_two_immediate_children()'"'"' in t; assert '"'"'failures.is_empty()'"'"' in t'
```

```sh
cargo test --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --lib jp2::tests::association_requires_two_immediate_children -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-AFC4DEDE80/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing predicate is the final failures.is_empty() assertion, whose first recorded mismatch is the empty Association at depth 1: expected rejection, observed Ok((1, 1)). All earlier framing/admission assertions completed. This is a valid direct unit-contract negative-input test, not a bypass of a required owning caller: BoxReader constructs the BoxRef from an ordinary eight-byte LBox=8/TBox=asoc frame, and the inspector's purpose includes inspecting malformed metadata. Requiring the very cardinality under test as an entry premise would be circular. Real production calls in the top-level parser and parse_compositing_layer pass depth=1 and remaining_depth=u32::MAX-1, exactly the first failing case's arguments. Higher-depth cases represent recursive subtree entries and are not necessary for the attribution.

[Standards quote, production lines, and provenance](support/evidence.md#dev-afc4dede80)

</details>

</details>

<a id="reproduce-dev-aaca35161c"></a>
<details>
<summary>DEV-AACA35161C — JPX container validation accepts a nonempty codestream-header sequence shorter than the codestream sequence</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-AACA35161C --output rfc-results/DEV-AACA35161C
```


**Standard:** t801-202106.txt § M.11.6

**Required behavior:** For supported top-level JPX codestream representations, header i shall describe codestream i, and container validation shall check that a nonempty header sequence covers every codestream; if there are no headers, JP2 defaults apply to all streams.

**Observed deviation:** JPX container validation accepts a nonempty codestream-header sequence shorter than the codestream sequence

This is a contract-valid direct unit call with ordinary immutable borrowed bytes and a valid ResourceLimits value. The prefix uses the fixed signature, jpx brand with jp2/jpx compatibility, rreq, and the original matching 1x1 grayscale JP2 header. Each jpch has a valid matching ihdr plus label and inherits default colour; each jp2c is copied from the repository's complete, independently decoded gray.jp2 fixture. The observed input is 1148 bytes, H=7,C=8, with both individual counts <=9 and negligible metadata/depth/work. The deliberate header shortfall is precisely the downstream file rule being validated, not an inadmissible parser entry. No omitted owning caller, forged private state, oversized configuration, incomplete codestream or production modification is needed.

**Recorded failure** (matched by the replay runner):

```text
codestream_header: None }], compositing_layers: [], composition: None, metadata_boxes: 1, metadata_depth: 1, box_count: 19 })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L382-L384`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L382-L384)

```text
    let limits = ResourceLimits::default()
        .with_maximum_codestreams(9)
        .unwrap();
```

[`src/jp2/spec_tests.rs:L386-L386`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L386)

```text
    for count in [1_usize, 2, 3, 8] {
```

[`src/jp2/spec_tests.rs:L404-L404`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L404)

```text
        for header_count in [0, count, count + 1, count.saturating_sub(1)] {
```

[`src/jp2/spec_tests.rs:L411-L421`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L411-L421)

```text
            input.extend((0..count).map(|_| parts[3].clone()));
            let bytes = input.concat();
            let result = parse(&bytes, &limits);
            if header_count != 0 && header_count != count {
                spec_record(
                    &mut failures,
                    matches!(result, Err(DecodeError::MalformedInput { .. })),
                    format!(
                        "JPX streams={count}, headers={header_count}: reject unequal nonempty sequence; {result:?}"
                    ),
                );
```

[`src/jp2/spec_tests.rs:L55-L61`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L55-L61)

```text
fn spec_finish(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "clause violations:\n{}",
        failures.join("\n")
    );
}
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2/spec_tests.rs`](support/tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs)
- [`jp2-spec-registration.patch`](support/tests/DEV-AACA35161C/files/jp2-spec-registration.patch)
- [`validation/jp2_spec_runner.py`](support/tests/DEV-AACA35161C/files/validation/jp2_spec_runner.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jp2-spec-registration.patch
```

```sh
python3 validation/jp2_spec_runner.py build
```

**Exact test invocation:**

```sh
python3 validation/jp2_spec_runner.py spec_jpx_header_bijection_and_defaults
```

[Machine-readable recipe](support/tests/DEV-AACA35161C/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed assertion is spec_finish's failures.is_empty(), which aggregates the exact clause check matches!(result, Err(DecodeError::MalformedInput { .. })) whenever header_count != 0 && header_count != count. This is not a crash, setup failure, or incorrect expectation about ordinal association. The retained host output contains the successful Jp2File suffix with an unassociated last stream and box_count: 19, identifying the replayed C=8,H=7 shortfall fixture (4 prefix boxes + 7 jpch + 8 jp2c). Earlier in test order the same check is exercised at C=2,H=1; the output tail does not retain that earlier diagnostic, so the grounded observation here is the retained C=8,H=7 result, not a claim to have seen the earlier line.

[Standards quote, production lines, and provenance](support/evidence.md#dev-aaca35161c)

</details>

</details>

<a id="reproduce-dev-cbf63fcfa1"></a>
<details>
<summary>DEV-CBF63FCFA1 — JPX parser admits nonzero codestream-header deficits and exposes unassociated codestreams</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-CBF63FCFA1 --output rfc-results/DEV-CBF63FCFA1
```


**Standard:** t801-202106.txt § M.11.6

**Required behavior:** For admitted JPX files with explicit codestream headers, expose each codestream together with its ordinal header association; nonzero header/codestream cardinality mismatches should not be admitted as a valid association set.

**Observed deviation:** JPX parser admits nonzero codestream-header deficits and exposes unassociated codestreams

Public byte-parser entry accepts a borrowed byte slice and valid ResourceLimits; it need not assume cardinality-valid input because rejection of that malformed structure is precisely the tested obligation. Fixture uses the retained 167-byte grayscale JP2's actual image/color and codestream bytes, JPX signature/type and rreq framing, one explicit top-level header for two streams, and one default compositing layer. Payload lifetimes are valid and all getter indices are in range on the actual parse-produced receiver. Smallest failing input is 357 bytes; all cases stay below 10KB, at most 64 streams/headers, with one layer and shallow nesting. Positive zero-header and equal-header controls pass. The deliberate header deficit is not private-state fabrication or a caller-contract violation.

**Recorded failure** (matched by the replay runner):

```text
N=2, H=1: admitted 2 streams, missing headers at [1]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`deepspec_codestream.patch:L14-L14`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L14)

```text
let gray = include_bytes!("../validation/image-consumer-probe/tests/fixtures/gray.jp2");
```

[`deepspec_codestream.patch:L117-L117`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L117)

```text
(2, 1), (3, 2), (8, 7), (64, 63), (64, 1),
```

[`deepspec_codestream.patch:L121-L121`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L121)

```text
match parse(&bytes, &limits) {
```

[`deepspec_codestream.patch:L126-L126`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L126)

```text
.filter(|&index| file.codestream(index).unwrap().codestream_header().is_none())
```

[`deepspec_codestream.patch:L136-L136`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L136)

```text
admitted_mismatches.is_empty(),
```

[`deepspec_codestream.patch:L137-L137`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L137)

```text
"T.801 M.11.6 forbids nonzero cardinality mismatches:\n{}"
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`deepspec_codestream.patch`](support/tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i deepspec_codestream.patch
```

```sh
python3 -c 'import hashlib; from pathlib import Path; assert hashlib.sha256(Path('"'"'src/jp2.rs'"'"').read_bytes()).hexdigest() == '"'"'85817f58973870ac765c4ba110650fffcd4b43a5e20c9a5c04a92bb15dfb5750'"'"', '"'"'replay source must contain the entire additive assertion extension'"'"''
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jp2::tests::retains_and_inspects_explicit_jpx_codestreams_and_headers -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-CBF63FCFA1/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final admitted_mismatches.is_empty() check, not fixture setup, decoding, or a resource failure. Host output specifically reports 'N=2, H=1: admitted 2 streams, missing headers at [1]'; all H=N and H=0 controls and the four H>N rejection cases completed before that assertion. Entry realism: the smallest failing case is only 357 bytes with two 1x1, one-component codestreams and one explicit header. The largest constructed case is 9,811 bytes, with at most 64 streams, 64 headers, one layer and shallow box nesting, within configured limits of 64 codestreams, 256 layers, 32 box depth, 16,777,216 metadata bytes and 2,147,483,648 input bytes. There is no large allocation or stack-exhaustion explanation. Native decode_jpx_codestream also passes caller-supplied bytes to this same public parser with options.limits(); the test does not invent an internal receiver or bypass its owning producer.

[Standards quote, production lines, and provenance](support/evidence.md#dev-cbf63fcfa1)

</details>

</details>

<a id="reproduce-dev-7f4ae3ed02"></a>
<details>
<summary>DEV-7F4AE3ED02 — JPX layer-reference validation accepts mixed registration-box presence</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-7F4AE3ED02 --output rfc-results/DEV-7F4AE3ED02
```


**Standard:** t801-202106.txt § M.11.7.7

**Required behavior:** A complete JPX layer-registration structural validation must account for file-wide all-or-none registration-box presence as well as whether each referenced codestream exists.

**Observed deviation:** JPX layer-reference validation accepts mixed registration-box presence

A direct unit-contract test is available with no requires predicates. Both layers are produced by the production parser from scoped headers, have local grayscale colour groups, default/unit registration grids, and, where creg exists, one entry with XS=YS=XR=YR=1 and XO=YO=0. The supplied count is a valid unit input; both explicit and implicit references exist in that sequence. No actual codestream objects are required by this function's slice/count API. Mixed presence is intentionally invalid under the downstream structural predicate, but is not forbidden at this validator's entry. Two layers, two codestreams, 80 fixture bytes and depth 3 are comfortably within configured limits. No complete-file parsing or codestream-decoding reproduction is claimed.

**Recorded failure** (matched by the replay runner):

```text
M.11.7.7 all-or-none presence: accepted 494 mixed patterns; first=Some((2, 1))
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_layer_reference_tests.rs:L35-L35`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L35)

```text
let layer = parse_compositing_layer(framed, &limits).unwrap();
```

[`src/jp2_layer_reference_tests.rs:L141-L149`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L141-L149)

```text
let references: Vec<Vec<u16>> = (0..length)
                .map(|index| {
                    if mask & (1 << index) == 0 {
                        Vec::new()
                    } else {
                        vec![u16::try_from(length - 1 - index).unwrap()]
                    }
                })
                .collect();
```

[`src/jp2_layer_reference_tests.rs:L152-L152`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L152)

```text
let actual = validate_layer_references(&layers, length);
```

[`src/jp2_layer_reference_tests.rs:L155-L155`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L155)

```text
} else if actual.is_ok() {
```

[`src/jp2_layer_reference_tests.rs:L161-L161`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L161)

```text
accepted_mixed.push((length, mask));
```

[`src/jp2_layer_reference_tests.rs:L169-L174`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L169-L174)

```text
assert!(
        accepted_mixed.is_empty(),
        "M.11.7.7 all-or-none presence: accepted {} mixed patterns; first={:?}",
        accepted_mixed.len(),
        accepted_mixed.first()
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_layer_reference_tests.rs`](support/tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs)
- [`layer_reference_tests.patch`](support/tests/DEV-7F4AE3ED02/files/layer_reference_tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i layer_reference_tests.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jp2.rs'"'"').read_text(); t=Path('"'"'src/jp2_layer_reference_tests.rs'"'"').read_text(); assert '"'"'mod layer_reference_contract {\n        include!("jp2_layer_reference_tests.rs");\n    }'"'"' in s; assert '"'"'fn no_registration_uses_ordinal_bounds()'"'"' in t; assert '"'"'fn registration_presence_is_file_wide_and_references_exist()'"'"' in t; assert '"'"'accepted_mixed.is_empty()'"'"' in t'
```

```sh
cargo test --lib --locked --no-run
```

**Exact test invocation:**

```sh
cargo test --lib --locked jp2::tests::layer_reference_contract::registration_presence_is_file_wide_and_references_exist -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-7F4AE3ED02/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final accepted_mixed.is_empty() check, not a fixture/parser failure: host replay reports 494 accepted mixed patterns and first=Some((2, 1)). The earlier original regression, all-registered controls, out-of-range-reference checks, and coherent all/none controls completed before this assertion. The smallest failing unit entry contains two parser-produced layers and codestream_count=2: layer 0 has one explicit reference to codestream 1; layer 1 has no creg and uses implicit ordinal 1. This is an admissible input to the validator, whose assessed unit contract has no requires predicates; mixed presence is the property to validate, not an entry precondition. The harness does not fabricate private layer fields or bypass layer parsing: it builds framed jplh/cgrp/colr/creg bytes and invokes BoxReader and parse_compositing_layer before the target. Resource realism is not a concern: the failing family uses only 2–8 layers/codestreams, one registration per registered layer and nesting depth 3; the smallest case is 80 bytes. Even the preceding control families use at most 256 layers, 64 codestreams and two registrations per layer, within the configured 256-layer, 64-codestream, depth-32 and metadata/input budgets. Real parse callers pass the same parser-produced layer vector and codestream_boxes.len(). MIR confirms that each layer is considered independently: nonempty registrations only take the bounded explicit-index route; empty registrations only take the bounded ordinal route; both join the layer increment and ultimately return Ok. Thus the smallest mixed case reaches Ok despite violating the exact all-or-none structural condition. The origin is rfc_map, not infer. I read its parent finding and the original T.801 (06/2021), ISO/IEC 15444-2:2021 M.11.7.7 section through read_rfc_section using the parent mapping item: it explicitly requires every Compositing Layer Header box to have creg if any has one. This applies to these ordinary layer headers and does not depend on rendering or multiple registrations per layer. Empty vectors faithfully denote absence because the local creg parser requires at least one entry. No prior cross-layer check exists in the inspected parse path, which publishes Jp2File immediately after this validator. This diagnosis is limited to the standards-derived structural-validation clause and the reproduced valid unit entry; it does not assert that every reader must reject every nonconforming file, that this replay executed a complete mixed JPX file, or that the unexamined project-selected newer standard edition is established. The review-marked edition qualification remains: the cited and checked authority here is the 2021 edition, not a claim of full current-edition conformance.

[Standards quote, production lines, and provenance](support/evidence.md#dev-7f4ae3ed02)

</details>

</details>

<a id="reproduce-dev-c635ac0588"></a>
<details>
<summary>DEV-C635AC0588 — Indexed JPX composition paints a zero-LIFE nonpersistent instruction that should be skipped</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-C635AC0588 --output rfc-results/DEV-C635AC0588
```


**Standard:** t801-202106.txt § M.5.3.2.1

**Required behavior:** JPX frame inspection and indexed rendering preserve the frame-closing persistence and timing semantics for the collected composition instructions.

**Observed deviation:** Indexed JPX composition paints a zero-LIFE nonpersistent instruction that should be skipped

Valid direct normalized unit entry, not claimed public-byte reachability: four layers and four in-bounds disjoint 1x1 placements on a 4x1 canvas; sequential layer assignment, no reuse/crop, one unrepeated instruction set, tick=1, loop_count=1, two requested in-range frames. build_timeline owns creation of all inspected frame objects. Supplied layer keys cover every assignment, each grayscale layer has exactly one u16 sample and alpha=None denotes opacity. Fresh supplied banks are used per render. Default limits and the explicit two-frame builder bound suffice. LIFE=0/PERSIST=false is an expressly defined standard special case, not an invalid state. Neither setup nor output depends on altered production behavior.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: T.801 M.5.3.2.1: PERSIST=false/LIFE=0 should perform no action; column 1 must remain transparent, not display the skipped thumbnail
  left: ([(0, 17, false), (17, 5, true)], [1000, 2000, 3000, 0], [65535, 65535, 65535, 0], [0, 0, 0, 4000], [0, 0, 0, 65535])
 right: ([(0, 17, false), (17, 5, true)], [1000, 0, 3000, 0], [65535, 0, 65535, 0], [0, 0, 0, 4000], [0, 0, 0, 65535])
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`frame-semantics.patch:L16-L16`](support/tests/DEV-C635AC0588/files/frame-semantics.patch#L16)

```text
+        assert_frame_closing_semantics();
```

[`src/jpx_frame_semantics_tests.rs:L188-L194`](support/tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L188-L194)

```text
    let skipped = closing_semantics_composition(vec![
        closing_semantics_instruction(0, 0, true),
        closing_semantics_instruction(1, 0, false),
        closing_semantics_instruction(2, 17, false),
        closing_semantics_instruction(3, 5, true),
    ]);
    let frames = build_timeline(&skipped, 4, 2).unwrap();
```

[`src/jpx_frame_semantics_tests.rs:L205-L206`](support/tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L205-L206)

```text
    let first = closing_semantics_render(&frames, 4, 0).unwrap();
    let next = closing_semantics_render(&frames, 4, 1).unwrap();
```

[`src/jpx_frame_semantics_tests.rs:L45-L51`](support/tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L45-L51)

```text
                Some(Ok(OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Grayscale,
                    colors: vec![vec![u16::try_from((index + 1) * 1000).unwrap()]],
                    alpha: None,
                })),
```

[`src/jpx_frame_semantics_tests.rs:L55-L64`](support/tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L55-L64)

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

[`src/jpx_frame_semantics_tests.rs:L218-L220`](support/tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L218-L220)

```text
            vec![(0, 17, false), (17, 5, true)],
            vec![1000, 0, 3000, 0],
            vec![u16::MAX, 0, u16::MAX, 0],
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`frame-semantics.patch`](support/tests/DEV-C635AC0588/files/frame-semantics.patch)
- [`src/jpx_frame_semantics_tests.rs`](support/tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs)

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

**Exact test invocation:**

```sh
python3 -c 'import os, pathlib, subprocess; name='"'"'jpx::tests::timeline_expands_repetition_reuse_and_timing_deterministically'"'"'; bins=[p for p in pathlib.Path('"'"'target/debug/deps'"'"').glob('"'"'jpeg2000_rs-*'"'"') if p.is_file() and os.access(p,os.X_OK)]; assert len(bins)==1, bins; exe=str(bins[0]); listed=subprocess.check_output([exe,'"'"'--list'"'"','"'"'--exact'"'"',name],text=True); assert name+'"'"': test'"'"' in listed, listed; os.execv(exe,[exe,'"'"'--exact'"'"',name,'"'"'--nocapture'"'"'])'
```

[Machine-readable recipe](support/tests/DEV-C635AC0588/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is the final zero-LIFE nonpersistent pixel/alpha comparison at src/jpx_frame_semantics_tests.rs:215, not an earlier metadata assertion or a build failure. Frame 0 contains gray 2000 and alpha 65535 at column 1, where the independent oracle expects transparency. This refutes the standards-derived inspection/indexed-rendering clause e11d683717746f5872e7302c; it does not demonstrate an incorrect field load in persistent() or refute the narrower getter clause 2a438ae4e8c4d1f07241ce8b.

[Standards quote, production lines, and provenance](support/evidence.md#dev-c635ac0588)

</details>

</details>

<a id="reproduce-dev-9a1abe441f"></a>
<details>
<summary>DEV-9A1ABE441F — Bounded standard JPIP VBAS fields with redundant zero groups are rejected as malformed</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-9A1ABE441F --output rfc-results/DEV-9A1ABE441F
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Decode bounded standard binary JPIP header fields by preserving their payload value and field boundary, with checked rejection of unsupported numeric extents.

**Observed deviation:** Bounded standard JPIP VBAS fields with redundant zero groups are rejected as malformed

This is a real eight-byte slice with fresh JpipHeaderContext and default ResourceLimits, not fabricated parser internals. Independent form bb=11 explicitly supplies Class and CSn; Class payload [0x80,0x08] denotes metadata class 8, valid for both JPP/JPT; CSn=0, Bin-ID=0, offset=0, length=1, c=0, and even Class means no Aux. Body [0xab] exists and 0x5a is the following unconsumed sentinel. Both padded fields are two bytes, well within MAX_VBAS_BYTES=10, and all values and extents fit u64/usize and default input limit 2147483648. The direct-unit cases likewise use valid Rust references, actual small slices and the two caller-used masks; redundant-zero fields are at most ten bytes and preserve mathematical payload. There is no callee precondition requiring a nonzero first group: the canonical predicate is total over u8/bool, and its false return is the implementation decision being tested.

**Recorded failure** (matched by the replay runner):

```text
parse_jpip_message -> read_vbas, independent metadata header [60, 80, 08, 00, 00, 01, ab, 5a]: expected value-preserving Data, got Err(MalformedInput { offset: 1, kind: InvalidJpipVbas })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`validation/jpip_vbas_contract.rs:L130-L134`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L130-L134)

```text
    for (class, length) in [
        (&[8][..], &[1][..]),
        (&[0x80, 8][..], &[1][..]),
        (&[8][..], &[0x80, 1][..]),
    ] {
```

[`validation/jpip_vbas_contract.rs:L135-L140`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L135-L140)

```text
        let mut input = vec![0x60];
        input.extend_from_slice(class);
        input.extend_from_slice(&[0, 0]);
        input.extend_from_slice(length);
        let header_length = input.len();
        input.extend_from_slice(&[0xab, 0x5a]);
```

[`validation/jpip_vbas_contract.rs:L142-L143`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L142-L143)

```text
        let mut context = JpipHeaderContext::new();
        match parse_jpip_message(&input, &mut context, &ResourceLimits::default()) {
```

[`validation/jpip_vbas_contract.rs:L157-L160`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L157-L160)

```text
            other => failures.push(format!(
                "parse_jpip_message -> read_vbas, independent metadata header \
                 {input:02x?}: expected value-preserving Data, got {other:?}"
            )),
```

[`validation/jpip_vbas_contract.rs:L163-L168`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L163-L168)

```text
    assert!(
        failures.is_empty(),
        "{} standard VBAS checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`validation/jpip_vbas_contract.rs`](support/tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs)
- [`validation/install-jpip-vbas-contract.py`](support/tests/DEV-9A1ABE441F/files/validation/install-jpip-vbas-contract.py)

**Build commands** (in order):

```sh
python3 validation/install-jpip-vbas-contract.py
```

```sh
cargo test --lib --locked --no-run
```

**Exact test invocation:**

```sh
cargo test --lib --locked jpip::tests::bounded_standard_vbas_contract -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-9A1ABE441F/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

Attribute this exact standards-derived clause 6b2c861fc7792bec596e0b48 to code_deviation, not the separate canonical-only inferred clauses. Reality check first: the suite has 703 canonical values, each tested sequentially with caller-used masks; the longest successful-field fixture is 3 prefix + 10 field + 3 suffix = 16 bytes. The eleven-byte negative field plus prefix is 14 bytes and is expected to reject, not succeed. Public positive witnesses are seven/eight bytes with a one-byte body. No recursion, enormous object, stack/pool pressure, unsupported numeric extent or impossible cursor is involved. Counts do not become parser input size. Default input capacity is 2 GiB; the public witness has offset+length=1. Real parse_jpip_message callers use the same None mask for Class and Msg-Length, so these are not merely disconnected private-function states.

[Standards quote, production lines, and provenance](support/evidence.md#dev-9a1abe441f)

</details>

</details>

<a id="reproduce-dev-a47c95c101"></a>
<details>
<summary>DEV-A47C95C101 — F11 root overlap: ingestion rejects an initial dependent header instead of applying Annex A identifier defaults</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-A47C95C101 --output rfc-results/DEV-A47C95C101
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Consume standard binary JPIP message-header forms, including inherited identifiers, using response-local parsing context.

**Observed deviation:** F11 root overlap: ingestion rejects an initial dependent header instead of applying Annex A identifier defaults

Safe DatabinCache::new(ResourceLimits::default()) and ordinary Rust slices/borrows; no private state mutation. Prior accepted responses leave only distinct metadata bins (class 8/codestream 127/bin 0, three bytes; class 8/codestream 0/bin 0, one byte), two fragments and four stored bytes. The new response [0x30,0,1,0] is a complete Annex A form-1 final precinct-bin-0 message with initial class/codestream defaults 0, offset 0 and one-byte body. Its semantic bin is absent, so it cannot conflict with those metadata bins; all resource counts/extents are far below default limits. This API has no transport-packet precondition, and no H.2 unreliable transport is invoked. Response-local reset is respected, not bypassed.

**Recorded failure** (matched by the replay runner):

```text
input=[30, 00, 01, 00], result=Err(MalformedInput { offset: 0, kind: InvalidJpipHeader })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_ingest_contract_tests.rs:L61-L62`](support/tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L61-L62)

```text
let result = ingest_jpip_response(input, cache);
    assert!(result.is_ok(), "input={input:02x?}, result={result:?}");
```

[`src/jpip_ingest_contract_tests.rs:L97-L97`](support/tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L97)

```text
let mut cache = DatabinCache::new(ResourceLimits::default());
```

[`src/jpip_ingest_contract_tests.rs:L255-L261`](support/tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L255-L261)

```text
let previous = DatabinId::new(8, 127, 0).unwrap();
    contract_ingest(
        &mut cache,
        &contract_message(3, previous, 0, b"old", true),
        1,
    );
    contract_ingest(&mut cache, &[0x50, 8, 0, 1, b'n'], 1);
```

[`src/jpip_ingest_contract_tests.rs:L263-L265`](support/tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L263-L265)

```text
let initial_dependent = [0x30, 0, 1, 0]; // precinct 0, one empty packet, final
    eprintln!("A.2.1 initial defaults: wire={initial_dependent:02x?}, expected class=0 CSn=0");
    contract_ingest(&mut cache, &initial_dependent, 1);
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_ingest_contract_tests.rs`](support/tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs)
- [`validation/jpip-ingest-contract.patch`](support/tests/DEV-A47C95C101/files/validation/jpip-ingest-contract.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/jpip-ingest-contract.patch
```

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip_ingest_contract_tests.rs'"'"').read_text(); assert p.count('"'"'include!("jpip_ingest_contract_tests.rs");'"'"') == 1; assert '"'"'fn ingests_response_with_inherited_context()'"'"' in p; assert '"'"'fn contract_ingest_payload_reassembly()'"'"' in t; assert '"'"'fn contract_ingest_header_forms_response_local()'"'"' in t'
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::contract_ingest_header_forms_response_local -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-A47C95C101/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is contract_ingest's result.is_ok() at src/jpip_ingest_contract_tests.rs:62 for input [30, 00, 01, 00], with Err(MalformedInput { offset: 0, kind: InvalidJpipHeader }). It is not a build failure, fixture failure, failed later byte assertion, or inference from exit status. The additive patch only includes a test module inside the existing cfg(test) tests module; host compilation and registration guards succeeded and this named native test actually executed.

[Standards quote, production lines, and provenance](support/evidence.md#dev-a47c95c101)

</details>

</details>

<a id="reproduce-dev-34cc912f98"></a>
<details>
<summary>DEV-34CC912F98 — Fresh JPIP history rejects the standard initial inherited header instead of selecting class 0 and codestream 0</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-34CC912F98 --output rfc-results/DEV-34CC912F98
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Create a fresh, independent message-history context for the supported standard inherited and explicit JPIP message-header forms; absence of previous messages must be distinguishable from a previously selected identity.

**Observed deviation:** Fresh JPIP history rejects the standard initial inherited header instead of selecting class 0 and codestream 0

Public constructor only; no private-state writes, injected faults, or fabricated history. Default ResourceLimits; three input bytes and zero body/offset are within capacity. Form 01, bin 0, canonical single-byte zero Msg-Offset/Msg-Length, no final flag, and no Aux for initial even class 0 satisfy T.808 (12/2022) A.2.1/A.2.2 in a fresh ordered JPP sequence with codestream 0 available. A direct header-parser unit test need not run cache insertion or image decoding. Absence of previous class/codestream is the legitimate initial state for which the standard defines both defaults.

**Recorded failure** (matched by the replay runner):

```text
fresh inherited header must select standard initial identity (0,0): MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_header_context_contract_tests.rs:L86-L90`](support/tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L86-L90)

```text
    let mut first_inherited = JpipHeaderContext::new();
    // A.2.1 supplies class=0 and CSn=0 when there is no previous message.
    // No codestream body/marker validation is required for header parsing.
    check(&mut first_inherited, &limits, 1, 0, 0)
        .expect("fresh inherited header must select standard initial identity (0,0)");
```

[`src/jpip_header_context_contract_tests.rs:L9-L17`](support/tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L9-L17)

```text
        let mut bytes = vec![form << 5];
        if form >= 2 {
            bytes.push(class as u8);
        }
        if form == 3 {
            bytes.push(codestream as u8);
        }
        bytes.extend([0, 0]);
        bytes
```

[`src/jpip_header_context_contract_tests.rs:L33-L33`](support/tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L33)

```text
        let (message, consumed) = parse_jpip_message(&bytes, context, limits)?;
```

[`src/jpip_header_context_contract_tests.rs:L57-L58`](support/tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L57-L58)

```text
    let limits = ResourceLimits::default();
    assert_eq!(limits.maximum_codestreams(), 64);
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_header_context_contract_tests.rs`](support/tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs)
- [`tests/jpip-context-registration.patch`](support/tests/DEV-34CC912F98/files/tests/jpip-context-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i tests/jpip-context-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip_header_context_contract_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jpip_header_context_contract_tests.rs");'"'"') == 1; assert '"'"'fn fresh_header_context_preserves_absence_and_independent_standard_history()'"'"' in t; assert '"'"'fn parses_all_three_standard_header_forms_and_auxiliary()'"'"' in s; assert '"'"'fn rejects_malformed_prefixes_and_vbas()'"'"' in s'
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::fresh_header_context_preserves_absence_and_independent_standard_history -- --exact
```

[Machine-readable recipe](support/tests/DEV-34CC912F98/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the final expect("fresh inherited header must select standard initial identity (0,0)"): parsing [0x20,0x00,0x00] returns MalformedInput { offset: 0, kind: InvalidJpipHeader }. Earlier freshness, independence, explicit-form and established-history checks completed. Entry realism: the generated sweep makes 1,030 sequential parsing calls, each with only 3–5 header bytes, zero payload, zero offset, bin 0, no nesting and a constant number of small value contexts; it does not retain an aggregate buffer/cache or consume a fragment pool. Codestream indices are 0–63, within the default 64-codestream capacity, and inputs are far below the 2,147,483,648-byte default input limit. The failing call uses only codestream 0. Real callers JpipMessageParser::new and ingest_jpip_response likewise start with JpipHeaderContext::new and feed ordinary message bytes into this parser. This is a valid direct unit entry, not private-state fabrication or bypass of a required owning caller.

[Standards quote, production lines, and provenance](support/evidence.md#dev-34cc912f98)

</details>

</details>

<a id="reproduce-dev-30865b7d2d"></a>
<details>
<summary>DEV-30865B7D2D — Fresh JPIP form-01 messages reject standard default class and codestream identifiers</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-30865B7D2D --output rfc-results/DEV-30865B7D2D
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Accept the standard binary message-header forms with inherited, explicit-class, or explicit-class-and-codestream identifiers, subject to the bounded parser profile.

**Observed deviation:** Fresh JPIP form-01 messages reject standard default class and codestream identifiers

The context is constructed by JpipHeaderContext::new(), not fabricated or privately mutated; there is deliberately no previous message. ResourceLimits::default().with_maximum_input_bytes(65536).unwrap() is valid. The five-byte message has form 01, class/CSn omitted as A.2.1 permits with zero defaults, in-class identifier 0, offset 0, body length 2, complete body [0x12,0x34], last=false and no Aux for class 0. Its complete canonical one-byte fields and extent 2 satisfy the bounded parser and read_vbas contracts. All 20 observed variants remain 5-7 bytes, so no platform capacity issue or owning-caller requirement is skipped. This is direct header parsing, not an assertion that the body is a fully decodable JPEG2000 image.

**Recorded failure** (matched by the replay runner):

```text
A.2.1 first form-01 bytes=[20, 00, 02, 12, 34]: expected class=0, CSn=0; got Err(MalformedInput { offset: 0, kind: InvalidJpipHeader })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_contract_tests.rs:L184-L186`](support/tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L184-L186)

```text
    let limits = ResourceLimits::default()
        .with_maximum_input_bytes(65536)
        .unwrap();
```

[`src/jpip_contract_tests.rs:L309-L317`](support/tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L309-L317)

```text
    let data = ContractData {
        class: 0,
        stream: 0,
        bin: 0,
        offset: 0,
        last: false,
        auxiliary: None,
        body: Vec::new(),
    };
```

[`src/jpip_contract_tests.rs:L354-L361`](support/tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L354-L361)

```text
            let mut first = data.clone();
            first.bin = bin;
            first.last = last;
            first.body = vec![0x12, 0x34];
            let (bytes, header_length) = contract_data_bytes(&first, 1);
            assert!(bytes.len() as u64 <= limits.maximum_input_bytes());
            let mut fresh = JpipHeaderContext::new();
            match parse_jpip_message(&bytes, &mut fresh, &limits) {
```

[`src/jpip_contract_tests.rs:L383-L383`](support/tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L383)

```text
    assert!(rejected.is_empty(), "A.2.1 first-header defaults rejected: {rejected:?}");
```

[`validation/run_jpip_contract.py:L64-L66`](support/tests/DEV-30865B7D2D/files/validation/run_jpip_contract.py#L64-L66)

```text
    return subprocess.run(
        [binary, name, "--exact", "--nocapture"], cwd=ROOT
    ).returncode
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_contract_tests.rs`](support/tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs)
- [`validation/run_jpip_contract.py`](support/tests/DEV-30865B7D2D/files/validation/run_jpip_contract.py)

**Build commands** (in order):

```sh
python3 validation/run_jpip_contract.py build
```

**Exact test invocation:**

```sh
python3 validation/run_jpip_contract.py run forms
```

[Machine-readable recipe](support/tests/DEV-30865B7D2D/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is src/jpip_contract_tests.rs:383, assert!(rejected.is_empty(), ...), after all preceding header-form sweeps and negative checks succeeded. Host case 07be402c570ea0a3c7248a7d specifically reports the five-byte input [20, 00, 02, 12, 34] returning InvalidJpipHeader rather than a data message with class=0 and CSn=0. This violates clause a3f0825227645d309edff279's acceptance of standard binary header forms, not an assertion about image decoding or full-response conformance.

[Standards quote, production lines, and provenance](support/evidence.md#dev-30865b7d2d)

</details>

</details>

<a id="reproduce-dev-f1e3ae04a3"></a>
<details>
<summary>DEV-F1E3AE04A3 — Fresh JPIP parser rejects standard inherited headers instead of applying initial zero defaults</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-F1E3AE04A3 --output rfc-results/DEV-F1E3AE04A3
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Accept the documented standard inherited and explicit binary message-header forms within the bounded JPIP profile.

**Observed deviation:** Fresh JPIP parser rejects standard inherited headers instead of applying initial zero defaults

Public constructor establishes an ordinary valid fresh parser, without private-field mutation or skipped initialization. A.2.1 in the acquired T.808 (12/2022)/ISO 15444-9:2023 text defines absent initial Class and CSn as zero. Form 01 is permitted, in-class=0 is a supported precinct identifier, offset and body length are zero, no auxiliary is required for class zero, and all three wire bytes are present. Three bytes and zero extent are far below default InputBytes=2,147,483,648; no allocation, stack-depth or pool stress is involved. The module promises inherited standard headers and the real response consumer also begins with the same unprimed public constructor. Opaque body framing is the unit contract, not image decoding.

**Recorded failure** (matched by the replay runner):

```text
A.2.1 zero defaults violated:
first inherited [20, 00, 00]: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`deepspec_jpip_parse.patch:L117-L117`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L117)

```text
+        let limits = ResourceLimits::default();
```

[`deepspec_jpip_parse.patch:L175-L193`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L175-L193)

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

[`deepspec_jpip_parse.patch:L194-L198`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L194-L198)

```text
+        assert!(
+            failures.is_empty(),
+            "A.2.1 zero defaults violated:\n{}",
+            failures.join("\n")
+        );
```

[`deepspec_jpip_replay.py:L38-L38`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_replay.py#L38)

```text
["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"]
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`deepspec_jpip_parse.patch`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch)
- [`deepspec_jpip_replay.py`](support/tests/DEV-F1E3AE04A3/files/deepspec_jpip_replay.py)

**Build commands** (in order):

```sh
python3 deepspec_jpip_replay.py build
```

**Exact test invocation:**

```sh
python3 deepspec_jpip_replay.py run jpip::tests::deepspec_standard_header_forms
```

[Machine-readable recipe](support/tests/DEV-F1E3AE04A3/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() in deepspec_standard_header_forms, reporting 'A.2.1 zero defaults violated' and specifically rejecting [0x20,0x00,0x00] with InvalidJpipHeader. This violates clause 43f15ffa053ac75972b21f66, not an implementation-derived acceptance oracle. Its origin is rfc_map finding ffeed0a3342d0e813d9226d596e3080256b9aa9ddff9ae787c5ab31bc04a96f5. I read the actual cited A.2.1 passage through the parent mapping item: T.808 (12/2022), ISO/IEC 15444-9:2023, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc. Table A.1 permits bb=01, omitting Class and CSn; the text explicitly makes each identifier zero when absent and no previous message exists. Msg-Offset and Msg-Length are nonnegative, so offset=length=0 is admissible. This applies to receiving binary headers, which module documentation explicitly supports; no transport, server, complete-image or cache-reassembly precondition is needed for this direct parser unit contract. The edition uncertainty in the mapping remains: this finding establishes the cited 2022/2023 rule, not independent conformance to the catalog's newer 2026 edition.

[Standards quote, production lines, and provenance](support/evidence.md#dev-f1e3ae04a3)

</details>

</details>

<a id="reproduce-dev-772e004106"></a>
<details>
<summary>DEV-772E004106 — Initial standard dependent JPIP header rejected instead of using zero identifier defaults</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-772E004106 --output rfc-results/DEV-772E004106
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Consume standard binary message-header forms, including inherited identifiers, using a fresh response-local parsing context.

**Observed deviation:** Initial standard dependent JPIP header rejected instead of using zero identifier defaults

The caller uses DatabinCache::new(ResourceLimits::default()), DatabinId::new(0,0,0), and safe ingestion only. For the first failing instance, one preexisting metadata databin holds 'm'; the addressed precinct databin is absent. 0x30 encodes permitted form 01, final=true, bin=0; offset=0, length=1 and payload='p' are complete, followed by EOR [0,2,0]. Both missing identifiers default to zero under the cited T.808 A.2.1. Seven input bytes, two eventual stored bytes, and two eventual accepted fragments are far within configured capacities. No transport-packet constraint is asserted: this is a complete ordered de-framed response, and this safe public entry owns parser creation.

**Recorded failure** (matched by the replay runner):

```text
T.808 A.2.1 initial inherited-header defaults violated:
primed=false, bin=0: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_ingest_contract_tests.rs:L96-L96`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L96)

```text
let mut cache = DatabinCache::new(ResourceLimits::default());
```

[`src/jpip_ingest_contract_tests.rs:L129-L131`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L129-L131)

```text
let target = DatabinId::new(0, 0, bin).unwrap();
            let mut wire = contract_data(1, target, 0, b"p", true);
            wire.extend(contract_eor(2, b""));
```

[`src/jpip_ingest_contract_tests.rs:L133-L133`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L133)

```text
match ingest_jpip_response(&wire, &mut cache) {
```

[`src/jpip_ingest_contract_tests.rs:L141-L141`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L141)

```text
Err(error) => failures.push(format!("primed={primed}, bin={bin}: {error:?}")),
```

[`src/jpip_ingest_contract_tests.rs:L156-L160`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L156-L160)

```text
assert!(
        failures.is_empty(),
        "T.808 A.2.1 initial inherited-header defaults violated:\n{}",
        failures.join("\n")
    );
```

[`src/jpip_ingest_contract_tests.rs:L2-L2`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L2)

```text
// These are de-framed, ordered JPP message sequences, not HTTP-UDP packets.
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_ingest_contract_tests.rs`](support/tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs)
- [`validation/register-jpip-contract-tests.py`](support/tests/DEV-772E004106/files/validation/register-jpip-contract-tests.py)

**Build commands** (in order):

```sh
python3 validation/register-jpip-contract-tests.py
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::contract_ingest_standard_headers_and_response_local_defaults -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-772E004106/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() at src/jpip_ingest_contract_tests.rs:156, reporting specifically 'T.808 A.2.1 initial inherited-header defaults violated' and InvalidJpipHeader for the initial form-1 messages. This is not an arbitrary nonzero exit: the host successfully built the additively registered library test and observed all twelve target calls returning that error. Earlier explicit/in-response inherited-header controls succeeded.

[Standards quote, production lines, and provenance](support/evidence.md#dev-772e004106)

</details>

</details>

<a id="reproduce-dev-ca85c0d7b5"></a>
<details>
<summary>DEV-CA85C0D7B5 — Initial dependent JPIP header is rejected instead of applying class-zero default</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-CA85C0D7B5 --output rfc-results/DEV-CA85C0D7B5
```


**Standard:** t808-202212.txt § A.2.1

**Required behavior:** The JPIP header context should support the documented inherited-class binary message-header form.

**Observed deviation:** Initial dependent JPIP header is rejected instead of applying class-zero default

Fresh safe public new() context has no predecessor, exactly the initial-message condition specified by T.808 A.2.1. The input is a complete three-byte form-01 header: Bin-ID 1, omitted Class and CSn defaulting to zero, offset zero, length zero, no Aux and no body. It is an admissible direct parser/context unit-contract entry, with ordered operations and no private-state writes, skipped state-owning caller, size violation, or payload-decoding claim.

**Recorded failure** (matched by the replay runner):

```text
valid dependent/independent header must parse: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`inherited_class_registration.patch:L8-L8`](support/tests/DEV-CA85C0D7B5/files/inherited_class_registration.patch#L8)

```text
+        check_inherited_class_contract();
```

[`src/jpip_inherited_class_contract.rs:L119-L124`](support/tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L119-L124)

```text
    for mut context in [JpipHeaderContext::new(), JpipHeaderContext::default()] {
        assert_eq!(context.previous_class(), None);
        assert_eq!(context.previous_codestream(), None);
        let initial = message(1, 0, 0, 0, &[]);
        assert_eq!(initial, [0x21, 0, 0]);
        accept(&initial, &mut context, 0, 0, 0, &[]);
```

[`src/jpip_inherited_class_contract.rs:L46-L49`](support/tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L46-L49)

```text
        let result = parse_jpip_message(input, context, &ResourceLimits::default());
        let (parsed, consumed) = result.unwrap_or_else(|error| {
            panic!("valid dependent/independent header must parse: {error:?}")
        });
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`inherited_class_registration.patch`](support/tests/DEV-CA85C0D7B5/files/inherited_class_registration.patch)
- [`src/jpip_inherited_class_contract.rs`](support/tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs)

**Build commands** (in order):

```sh
patch -p1 --batch --forward -i inherited_class_registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); assert s.count('"'"'check_inherited_class_contract();'"'"') == 1; assert s.count('"'"'include!("jpip_inherited_class_contract.rs");'"'"') == 1; h=Path('"'"'src/jpip_inherited_class_contract.rs'"'"').read_text(); assert '"'"'assert_eq!(initial, [0x21, 0, 0]);'"'"' in h; assert '"'"'fn check_inherited_class_contract()'"'"' in h'
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-CA85C0D7B5/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing check is accept()'s requirement that parse_jpip_message succeed on [0x21, 0x00, 0x00] with a fresh JpipHeaderContext::new(); the host reports InvalidJpipHeader. This violates e747fe080bc71ebe1d1c602a's documented inherited-header support, interpreted using its rfc_map parent's explicitly applicable T.808 (12/2022), A.2.1, rather than the separate inferred observer-history clause. I read the parent provenance/document manifest and retrieved A.2.1 in full through the origin item. Table A.1 permits form 01 with neither Class nor CSn; the text explicitly defaults both identifiers to zero when there is no previous message. The test assumes one reliably ordered stream per context, which matches that section's applicability. No newer-edition conformance is claimed.

[Standards quote, production lines, and provenance](support/evidence.md#dev-ca85c0d7b5)

</details>

</details>

<a id="reproduce-dev-77c8a9e6c3"></a>
<details>
<summary>DEV-77C8A9E6C3 — JPIP classifier assigns standard data-bin semantics to reserved class identifiers 3 and 7</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-77C8A9E6C3 --output rfc-results/DEV-77C8A9E6C3
```


**Standard:** t808-202212.txt § A.2.2

**Required behavior:** The project's bounded JPIP profile classifies accepted message class identifiers into the semantic data-bin class used by parsed identifiers and cache lookup.

**Observed deviation:** JPIP classifier assigns standard data-bin semantics to reserved class identifiers 3 and 7

No owning-caller fixture is bypassed: this is a direct unit_contract test of a pure scalar function with an unrestricted u64 entry domain. Identifiers 3/7 satisfy that entry and the callee's precondition-free contract, despite being reserved by the pinned standard; rejection of unsupported identifiers is part of classification. No parsed headers, resource-limited buffers, cache objects, stream-type combinations, or negotiated extension states are manufactured. There are 2231 bounded sequential comparisons, constant call depth, and only two diagnostic strings on this replay. The build helper inserted only test registration and compiled the unchanged production implementation. Applicability assumes no separately negotiated reserved-class extension, as in the originating standards finding.

**Recorded failure** (matched by the replay runner):

```text
semantic classification mismatches:
DatabinClass::from_identifier(3): expected None, got Some(TileHeader)
DatabinClass::from_identifier(7): expected None, got Some(MainHeader)
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_class_contract_tests.rs:L16-L24`](support/tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L16-L24)

```text
    let mut check = |identifier, expected| {
        let actual = DatabinClass::from_identifier(identifier);
        checks += 1;
        if actual != expected {
            failures.push(format!(
                "DatabinClass::from_identifier({identifier}): expected {expected:?}, got {actual:?}"
            ));
        }
    };
```

[`src/jpip_class_contract_tests.rs:L31-L35`](support/tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L31-L35)

```text
    for identifier in 0..=1024 {
        if ![0, 1, 2, 4, 5, 6, 8].contains(&identifier) {
            check(identifier, None);
        }
    }
```

[`src/jpip_class_contract_tests.rs:L59-L63`](support/tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L59-L63)

```text
    assert!(
        failures.is_empty(),
        "semantic classification mismatches:\n{}",
        failures.join("\n")
    );
```

[`deepstar_jpip_contract.py:L9-L14`](support/tests/DEV-77C8A9E6C3/files/deepstar_jpip_contract.py#L9-L14)

```text
anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
registration = '\n    include!("jpip_class_contract_tests.rs");\n'
assert original.count(anchor) == 1, "existing JPIP unit-test module not found uniquely"
assert registration not in original, "contract test already registered"
updated = original.replace(anchor, anchor + registration, 1)
assert updated.replace(registration, "", 1) == original
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_class_contract_tests.rs`](support/tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs)
- [`deepstar_jpip_contract.py`](support/tests/DEV-77C8A9E6C3/files/deepstar_jpip_contract.py)

**Build commands** (in order):

```sh
python3 deepstar_jpip_contract.py
```

**Exact test invocation:**

```sh
target/deepstar-jpip-contract jpip::tests::class_identifiers_follow_t808_table_a2 --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-77C8A9E6C3/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() at src/jpip_class_contract_tests.rs:59, with the first mismatch 'DatabinClass::from_identifier(3): expected None, got Some(TileHeader)' and a second mismatch for 7/MainHeader. This violates the standards-derived semantic-classification clause 482717998449fb68d677ca32, interpreted with its rfc_map parent finding, not the separate implementation-inferred interval clause 23bb46fbb1035a00030a243f. I read clause_origin and independently retrieved the pinned T.808 (12/2022), A.2.2, version 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: Table A.2 defines only 0,1,2,4,5,6,8; all other values are reserved. Odd parity supplies auxiliary-field syntax for parsing even unrecognized messages; it does not define extended tile-header or extended main-header semantics. This is classification, not a claim that the standard mandates this particular Option API or unconditional rejection rather than skipping. Under the originating finding's no-separately-negotiated-extension assumption, None faithfully expresses the absence of a supported standard semantic class. The module advertises standard header forms and classes 0 through 8 but provides no negotiated-extension premise for inventing meanings for 3/7. Its range predicate, native oracle and correlated inferred contracts do not override the cited standard. This conclusion is limited to that edition and assumption, not every future extension.

[Standards quote, production lines, and provenance](support/evidence.md#dev-77c8a9e6c3)

</details>

</details>

<a id="reproduce-dev-e9ba9c12dc"></a>
<details>
<summary>DEV-E9BA9C12DC — JPIP parser rejects legal initial inherited-form header instead of defaulting Class and CSn to zero</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-E9BA9C12DC --output rfc-results/DEV-E9BA9C12DC
```


**Standard:** t808-202212.txt § A.2.2

**Required behavior:** The supported binary-header forms preserve inspectable message identifiers while deterministic reassembly recognizes extended and ordinary precinct messages as addressing the same logical precinct data-bin.

**Observed deviation:** JPIP parser rejects legal initial inherited-form header instead of defaulting Class and CSn to zero

Public JpipMessageParser::new(ResourceLimits::default()) provides a valid fresh context, with neither previous class nor previous codestream. No private mutation or unsafe fixture is used. T.808 (12/2022) A.2.1 expressly permits first-message bb=01 with both omitted fields defaulting to zero. The four-byte message carries ordinary precinct bin 0, byte offset 0, body length 1, no Aux or final flag, and one zero payload byte. It is far below configured capacities, requires no predecessor, and matches the fresh-parser pattern used by ingest_jpip_response. The broader supported-forms clause applies even though the failed parse prevents reaching the getter.

**Recorded failure** (matched by the replay runner):

```text
parse -> header.databin -> fragment: form=1 class=0 stream=0 bin=0 offset=0 last=false wire=[20, 00, 01, 00]: MalformedInput { offset: 0, kind: InvalidJpipHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`jpip_databin_contract.patch:L280-L281`](support/tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L280-L281)

```text
+        let mut initial = JpipMessageParser::new(limits);
+        contract_parse_precinct(&mut initial, 1, (0, 0, 0), (0, false));
```

[`jpip_databin_contract.patch:L75-L76`](support/tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L75-L76)

```text
+        let wire = contract_precinct_wire(form, identity, range);
+        let (message, consumed) = parser.parse(&wire).unwrap_or_else(|error| {
```

[`jpip_databin_contract.patch:L7-L7`](support/tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L7)

```text
+        databin_contract_sweep();
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`jpip_databin_contract.patch`](support/tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jpip_databin_contract.patch
```

```sh
python3 -c 'import hashlib; assert hashlib.sha256(open('"'"'src/jpip.rs'"'"','"'"'rb'"'"').read()).hexdigest() == '"'"'a997101e1b6d41f15ef6f9832cd4564f02a49fb6b42a380d01d9cab3d38c11f7'"'"', '"'"'replay source mismatch: test registration/assertions missing'"'"''
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-E9BA9C12DC/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the helper's expectation that parsing succeeds for form=1 class=0 stream=0 bin=0 offset=0 last=false, wire=[20, 00, 01, 00]. It is not a failed databin identity comparison or a sanitizer/setup failure. The preceding 25-identity sweep completes before this final fresh-parser case. Resource realism was checked first: the failing message is four bytes with a one-byte body and end offset 1; the sweep uses sequential, nonrecursive fixtures, at most eleven bytes per encoded message, and each cache holds three bins/five bytes/seven accepted fragments. Default limits allow 2,147,483,648 input bytes and 1,048,576 JPIP fragments. Large sampled identifiers are scalar identifiers, not requests to allocate that many codestreams or precincts. The actual failing identity is entirely zero. Production ingest_jpip_response likewise creates a fresh JpipMessageParser and immediately parses the first message; the test does not fabricate private state or bypass a required initialization step.

[Standards quote, production lines, and provenance](support/evidence.md#dev-e9ba9c12dc)

</details>

</details>

<a id="reproduce-dev-2d35e4323c"></a>
<details>
<summary>DEV-2D35E4323C — Reserved T.808 (12/2022) classes 3 and 7 receive standard TileHeader and MainHeader semantics</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-2D35E4323C --output rfc-results/DEV-2D35E4323C
```


**Standard:** t808-202212.txt § A.2.2

**Required behavior:** When constructing semantic data-bin identities for standard T.808 (12/2022) messages, the defined class values are 0,1,2,4,5,6,8. Reserved values 3 and 7 shall not be assigned ordinary TileHeader or MainHeader semantics as if defined by that edition, absent a documented separately supported extension. This does not prescribe whether a standalone raw-header utility must reject or preserve an unrecognized value.

**Observed deviation:** Reserved T.808 (12/2022) classes 3 and 7 receive standard TileHeader and MainHeader semantics

A direct public unit-contract entry accepting three by-value u64 arguments and returning Result, with no safety or caller preconditions. No fabricated private state, payload, wire frame, cache, negotiated extension, or decoder setup is involved. Reserved identifiers are admissible inputs to this fallible validation boundary. Only small finite vectors and constant stack depth are required; even the first stream=0, bin=0 cases produce the failure. Large numeric identifiers do not request corresponding resources. Scope is standard T.808 (12/2022) semantic assignment, not mandatory raw-header rejection or 2026 conformance.

**Recorded failure** (matched by the replay runner):

```text
264 reserved-class assignments received ordinary standard semantics; first examples: [(3, 0, 0, TileHeader), (7, 0, 0, MainHeader), (3, 0, 1, TileHeader), (7, 0, 1, MainHeader), (3, 0, 15, TileHeader), (7, 0, 15, MainHeader), (3, 0, 16, TileHeader), (7, 0, 16, MainHeader), (3, 0, 127, TileHeader), (7, 0, 127, MainHeader), (3, 0, 128, TileHeader), (7, 0, 128, MainHeader)]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_databin_contract_tests.rs:L1-L3`](support/tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L1-L3)

```text
// T.808 (12/2022), A.2.2/Table A.2 and A.3.6.1. These tests construct
// identities directly: there is no wire framing, AUX interpretation, or
// negotiated extension. Each cache represents exactly one logical target.
```

[`src/jpip_databin_contract_tests.rs:L61-L76`](support/tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L61-L76)

```text
        for raw in [3, 7] {
            // Rejection OR an explicit unknown representation is acceptable.
            // Do not impose a raw-header rejection/error-code requirement.
            if let Ok(identity) = DatabinId::new(raw, stream, bin) {
                if matches!(
                    identity.class(),
                    DatabinClass::Precinct
                        | DatabinClass::TileHeader
                        | DatabinClass::TileData
                        | DatabinClass::MainHeader
                        | DatabinClass::Metadata
                ) {
                    forbidden.push((raw, stream, bin, identity.class()));
                }
            }
        }
```

[`src/jpip_databin_contract_tests.rs:L78-L83`](support/tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L78-L83)

```text
    assert!(
        forbidden.is_empty(),
        "{} reserved-class assignments received ordinary standard semantics; first examples: {:?}",
        forbidden.len(),
        &forbidden[..forbidden.len().min(12)]
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_databin_contract_tests.rs`](support/tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs)
- [`validation/databin-contract-tests.patch`](support/tests/DEV-2D35E4323C/files/validation/databin-contract-tests.patch)
- [`validation/databin-contract-runner.py`](support/tests/DEV-2D35E4323C/files/validation/databin-contract-runner.py)

**Build commands** (in order):

```sh
bash -eu -c 'patch --batch --forward -p1 < validation/databin-contract-tests.patch; grep -Fq '"'"'include!("jpip_databin_contract_tests.rs");'"'"' src/jpip.rs; cargo test --lib --no-run --locked; python3 validation/databin-contract-runner.py'
```

**Exact test invocation:**

```sh
target/databin-contract-tests jpip::tests::standard_databin_class_assignment --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-2D35E4323C/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is forbidden.is_empty() in standard_databin_class_assignment, not a build, setup, resource, or unrelated cache failure. Host replay reports 264 reserved-class assignments, including (3,0,0,TileHeader) and (7,0,0,MainHeader). The test invokes the real public DatabinId::new and reads class(); it does not fabricate a DatabinId or bypass an owning caller. Its 132 coordinates produce 1,188 scalar constructor calls (seven defined and two reserved classes per coordinate), with 264 small failure records, constant call depth, and only small Vec allocations. u64::MAX coordinates are opaque identifiers, not requested buffer sizes or allocation counts. There is no constructor ResourceLimits argument, indexing by these identifiers, or payload allocation. The all-zero coordinate already exposes both deviations, so the finding does not depend on large values, nonzero main-header bin identifiers, or caller capacities.

[Standards quote, production lines, and provenance](support/evidence.md#dev-2d35e4323c)

</details>

</details>

<a id="reproduce-dev-7a30eec46f"></a>
<details>
<summary>DEV-7A30EEC46F — Reserved JPIP class identifiers 3 and 7 admitted as semantic header classes</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-7A30EEC46F --output rfc-results/DEV-7A30EEC46F
```


**Standard:** t808-202212.txt § A.2.2

**Required behavior:** For the supported standard binary-message profile, expose semantic header classes only for assigned message identifiers; reserved signaling is not to be admitted as a supported extension.

**Observed deviation:** Reserved JPIP class identifiers 3 and 7 admitted as semantic header classes

Public scalar-only checked-constructor inputs, initially codestream=0 and in_class=0, with no private-state edits, payloads, parser context or downstream plan invariants. Reserved codes are legitimate negative inputs to the checked constructor, whose job is admission. Only a bounded 54,340 sequential probes and small vectors are used; u64 identity extrema do not imply huge buffers. The runner's sole existing-source change is an asserted test-only include insertion and its host build succeeded.

**Recorded failure** (matched by the replay runner):

```text
DatabinId::new(3, 0, 0) -> Ok(DatabinId { class_identifier: 3, class: TileHeader, codestream: 0, in_class: 0 }); Table A.2 semantic class expected None
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_class_contract_cases.rs:L38-L39`](support/tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L38-L39)

```text
let result = DatabinId::new(code, codestream, in_class);
            match (standard_class_oracle(code), result) {
```

[`src/jpip_class_contract_cases.rs:L5-L9`](support/tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L5-L9)

```text
2 => Some(DatabinClass::TileHeader),
        4 | 5 => Some(DatabinClass::TileData),
        6 => Some(DatabinClass::MainHeader),
        8 => Some(DatabinClass::Metadata),
        _ => None,
```

[`src/jpip_class_contract_cases.rs:L58-L62`](support/tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L58-L62)

```text
assert!(
        violations.is_empty(),
        "reserved signaling exposed as supported, or assigned class rejected:\n{}",
        violations.join("\n")
    );
```

[`jpip_class_contract_suite.py:L24-L24`](support/tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py#L24)

```text
assert edited.replace(registration, "", 1) == original
```

[`jpip_class_contract_suite.py:L59-L59`](support/tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py#L59)

```text
os.execv(binary, [binary, "--exact", sys.argv[1], "--nocapture"])
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_class_contract_cases.rs`](support/tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs)
- [`jpip_class_contract_suite.py`](support/tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py)

**Build commands** (in order):

```sh
python3 jpip_class_contract_suite.py build
```

**Exact test invocation:**

```sh
python3 jpip_class_contract_suite.py jpip::tests::standard_assigned_classes_exclude_reserved_signaling
```

[Machine-readable recipe](support/tests/DEV-7A30EEC46F/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is violations.is_empty() at src/jpip_class_contract_cases.rs:58, not a setup failure or an arbitrary nonzero exit. The first recorded mismatch is DatabinId::new(3, 0, 0) returning Ok with class TileHeader when the independent Table A.2 oracle assigns no class; code 7 similarly returns MainHeader. This violates clause cca7e9b1da20c54a63cf8dfc's assigned-only supported semantic header guarantee.

[Standards quote, production lines, and provenance](support/evidence.md#dev-7a30eec46f)

</details>

</details>

<a id="reproduce-dev-76bf9f9717"></a>
<details>
<summary>DEV-76BF9F9717 — JPIP geometric mapping rejects admissible clipped precincts and invents empty-resolution precincts</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-76BF9F9717 --output rfc-results/DEV-76BF9F9717
```


**Standard:** t808-202212.txt § A.3.2.1

**Required behavior:** Derive precinct coordinates and JPIP in-class identifiers from SIZ/COD/COC geometry, with resolution-prefix numbering and checked tile/component interleaving.

**Observed deviation:** JPIP geometric mapping rejects admissible clipped precincts and invents empty-resolution precincts

This is a direct public unit-contract entry with an API-reachable cache, not an internal planner fixture. Fixture13's seeded geometry is 56x47, origin/tile-origin (3,2), tiles 24x24, six tiles and three legal sample separations, one layer and D<=2. Completed bins contain SOC/SIZ/main COD/QCD and properly scoped first-tile-part COD/COC/QCD/QCC overrides and SOD. Coding-style precedence and explicit precinct exponents satisfy T.800 A.6.1/B.6; headers and resource requirements are far below default limits. Missing precinct payload bins are intentional for geometric derivation and consistent with native usage and the target's synthetic header-only template. The zero-count execution violating the packet constructor's predicate is created by the owning production planner, not supplied by the test.

**Recorded failure** (matched by the replay runner):

```text
fixture=13 progression=0: expected 810 mappings, got MalformedInput { offset: 0, kind: InvalidPacketHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_precinct_map_contract.rs:L161-L165`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L161-L165)

```text
        let mut cache = DatabinCache::new(limits);
        for (id, bytes) in &bins {
            cache.insert(fragment(*id, 0, bytes, true)).unwrap();
            assert!(cache.is_complete(*id));
        }
```

[`src/jpip_precinct_map_contract.rs:L241-L241`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L241)

```text
    let mapping = cache.derive_precinct_map(stream)?;
```

[`src/jpip_precinct_map_contract.rs:L275-L280`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L275-L280)

```text
    let mut seed = 0x4a50_4950_u32;
    for index in 0..32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let width = 25 + seed % 42;
        let height = 25 + (seed >> 8) % 42;
        let origin = [seed % 4, (seed >> 4) % 4];
```

[`src/jpip_precinct_map_contract.rs:L330-L342`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L330-L342)

```text
            let (cache, bins) = geometry.fixture(stream, progression, ResourceLimits::default());
            let observed = match map_contract_observe(&cache, stream) {
                Ok(observed) => observed,
                Err(error) => {
                    let failure = format!(
                        "fixture={index} progression={progression}: expected {} mappings, got {error:?}",
                        expected.len()
                    );
                    eprintln!("{failure}");
                    failures.push(failure);
                    continue;
                }
            };
```

[`src/jpip_precinct_map_contract.rs:L207-L212`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L207-L212)

```text
                        // T.800 B-16 explicitly gives zero for empty resolution bounds.
                        spans[axis] = if hi == lo {
                            0
                        } else {
                            hi.div_ceil(width) - lo / width
                        };
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_precinct_map_contract.rs`](support/tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs)
- [`precinct-map-test-registration.patch`](support/tests/DEV-76BF9F9717/files/precinct-map-test-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i precinct-map-test-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip_precinct_map_contract.rs'"'"').read_text(); assert '"'"'include!("jpip_precinct_map_contract.rs");'"'"' in s; assert '"'"'fn derive_precinct_map_matches_standard_geometry_contract()'"'"' in t; assert '"'"'failures.is_empty()'"'"' in t; assert '"'"'requested'"'"' not in t'
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::derive_precinct_map_matches_standard_geometry_contract -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-76BF9F9717/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed check is the accumulated geometry-contract assertion at src/jpip_precinct_map_contract.rs:382, whose first recorded discrepancy is fixture=13 progression=0: expected 810 mappings, got MalformedInput { offset: 0, kind: InvalidPacketHeader }. This is a geometry failure of the actual public derive_precinct_map entry, not a build failure, sanitizer failure, or a fixture that directly injects invalid decoder state.

[Standards quote, production lines, and provenance](support/evidence.md#dev-76bf9f9717)

</details>

</details>

<a id="reproduce-dev-e2df8b9fac"></a>
<details>
<summary>DEV-E2DF8B9FAC — Automatic JPIP header normalization rejects legal SOD-omitted tile-header bins</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-E2DF8B9FAC --output rfc-results/DEV-E2DF8B9FAC
```


**Standard:** t808-202212.txt § A.3.3

**Required behavior:** Provide automatic precinct reconstruction with automatic normalization of the cached headers, as distinct from the explicitly caller-reframed plan path.

**Observed deviation:** Automatic JPIP header normalization rejects legal SOD-omitted tile-header bins

Safe-Rust unit test of the actual owning automatic_headers function, with test-only private access and no production behavior changes. Stream 37 has a legal SOC/SIZ/COD/QCD/COM main header, 1 tile, 1 component, and a completed class-2 in-class-0 empty tile bin. contract_insert uses normal fragment insertion with final_byte=true, then asserts is_complete and byte equality. Empty raw JPP tile-header bins and omitted SOD are explicitly permitted by pinned T.808 A.3.3. The sweep remains far below resource limits, uses one tile-part/tile and valid non-packed Part-1 fixtures, and requires no precinct payload to derive headers. Paired SOD-present entries pass the same exact canonical-header assertions.

**Recorded failure** (matched by the replay runner):

```text
A.3.3 class-2 raw JPP bins: stream=37, tiles=1, components=1, marker_bytes=0, SOD=false: rejected legal bin: MalformedInput { offset: 0, kind: MissingRequiredStructure }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_automatic_headers_contract_tests.rs:L31-L33`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L31-L33)

```text
    cache
        .insert(fragment(target, split as u64, &bytes[split..], true))
        .unwrap();
```

[`src/jpip_automatic_headers_contract_tests.rs:L39-L40`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L39-L40)

```text
    assert!(cache.is_complete(target));
    assert_eq!(cache.databin_view(target).unwrap().as_bytes(), bytes);
```

[`src/jpip_automatic_headers_contract_tests.rs:L203-L216`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L203-L216)

```text
                for with_sod in [true, false] {
                    let mut raw = markers.clone();
                    if with_sod {
                        raw.extend_from_slice(&[0xff, 0x93]);
                    }
                    let mut cache = DatabinCache::new(ResourceLimits::default());
                    contract_insert(&mut cache, DatabinId::new(6, stream, 0).unwrap(), &main);
                    for tile in 0..tiles {
                        contract_insert(
                            &mut cache,
                            DatabinId::new(2, stream, u64::from(tile)).unwrap(),
                            &raw,
                        );
                    }
```

[`src/jpip_automatic_headers_contract_tests.rs:L223-L223`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L223)

```text
                    match cache.automatic_headers(stream) {
```

[`src/jpip_automatic_headers_contract_tests.rs:L242-L244`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L242-L244)

```text
                        Err(error) => {
                            failures.push(format!("{context}: rejected legal bin: {error:?}"))
                        }
```

[`src/jpip_automatic_headers_contract_tests.rs:L260-L264`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L260-L264)

```text
    assert!(
        failures.is_empty(),
        "automatic header normalization failures:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_automatic_headers_contract_tests.rs`](support/tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs)
- [`validation/build_automatic_headers_contract.py`](support/tests/DEV-E2DF8B9FAC/files/validation/build_automatic_headers_contract.py)

**Build commands** (in order):

```sh
python3 validation/build_automatic_headers_contract.py
```

**Exact test invocation:**

```sh
target/automatic_headers_contract jpip::tests::automatic_headers_optional_sod_contract --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-E2DF8B9FAC/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the final failures.is_empty() assertion in automatic_headers_optional_sod_contract; its first collected failure is stream=37, tiles=1, components=1, marker_bytes=0, SOD=false returning MissingRequiredStructure at offset 0. This is a valid direct unit-contract entry to automatic_headers, not a fixture bypassing the owning normalization caller. Realism check: each case has 1/2/7 tiles, 1/3 components, image dimensions at most 14x3 (42 pixels), one tile-part/tile and no recursive nesting. The largest cache in this failing sweep contains 78 main-header bytes plus seven 9-byte tile bins, 141 bytes total, at most eight bins and 16 insertions. Default limits are HARDENED (16,384 tiles, 64 components, 100,000,000 pixels, 2,147,483,648 input bytes, 1,048,576 fragments), so these are ordinary small inputs, not a capacity/stack stress case. The real derive_precinct_map caller obtains its headers by this same automatic_headers call; its existing regression also builds a cache from completed main and tile bins before mapping. Precinct packet data is not an entry requirement for deriving planning headers. The replay builds the unchanged implementation with only an additive test include, constructs main/class-6 and tile/class-2 bins through insert and JpipFragment::new, explicitly completes even empty bins, and checks completion and exact retrieved bytes. The independent full-codestream fixture passes parse_layout before the target call. The applicable authority is the rfc_map parent's pinned T.808 (12/2022), ISO/IEC 15444-9:2023, Annex A.3.3, independently read through read_rfc_section using origin item 644e28e929d66e2e5c7a925f: tile-header bins exclude SOT, 'Inclusion of SOD markers is optional', and a server must send a tile-header bin even when empty. These fixtures instantiate exactly that raw JPP representation, not the explicitly caller-reframed plan interface. SUPPORTED.md says automatic reconstruction performs its own normalization. Thus the clause is sound; requiring an input SOD would contradict this normative representation allowance, not repair the test. Trace: automatic_headers retrieves completed main and all required tile bins, computes SIZ cardinality, normalizes main, and passes each raw tile vector unchanged to normalize_automatic_tile_headers. The non-SOT parser branch returns one transition containing the original bytes even for empty input. The normalizer sets found_sod=false, consumes zero or all valid marker segments, then rejects on !found_sod before its output SOD synthesis. The target MIR confirms the normalizer call at bb51 and unchanged error propagation through bb53/bb54/bb56. MissingRequiredStructure is therefore caused by an unnecessary framing requirement, not missing/incomplete bins, a fixture assertion, or a parser/build failure. All 18 omitted-SOD cases fail at offsets 0/7/6, while the paired SOD-present calls satisfy the exact output oracle. Retained implementation-descriptive error clauses documenting SOD rejection have no caller requires; they do not override the standards-derived consumer contract or make this entry inadmissible. The aggregate assertion reports the actual Result failures adequately; this diagnosis uses that successful host replay, not the separately noted failed standalone deviation replay. Consequence: legal completed JPP header data is rejected, preventing automatic planning/reconstruction instead of synthesizing canonical SOT/SOD framing.

[Standards quote, production lines, and provenance](support/evidence.md#dev-e2df8b9fac)

</details>

</details>

<a id="reproduce-dev-dfa2d383b6"></a>
<details>
<summary>DEV-DFA2D383B6 — Complete tile headers with undersized nonzero Psot are reported Available with unspecified data length</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-DFA2D383B6 --output rfc-results/DEV-DFA2D383B6
```


**Standard:** t808-202212.txt § A.3.3

**Required behavior:** Automatically normalize received JPIP header bins into reconstructed decoder inputs without imposing the explicit caller-planned header preparation contract.

**Observed deviation:** Complete tile headers with undersized nonzero Psot are reported Available with unspecified data length

The tested header is 22 actual bytes: a 12-byte SOT with Lsot=10, tile=7, Psot=12, TPsot=0 and TNsot=1; an 8-byte COM segment with valid length and registration; and 2-byte SOD. There is no earlier parse failure. The malformed Psot is intentionally the format predicate this public parser query must reject, not a cache-entry precondition. contract_cache uses DatabinCache::new(ResourceLimits::default()), inserts one final class-2 fragment at offset zero via JpipFragment::new and DatabinCache::insert, unwraps successful acceptance, and asserts is_complete for the matching ID. All storage/count/platform bounds are comfortably satisfied, and the shared Rust borrow is valid. The fixture never mutates private cache fields or fabricates a transition supplied to the production query.

**Recorded failure** (matched by the replay runner):

```text
bytes=[ff, 90, 00, 0a, 00, 07, 00, 00, 00, 0c, 00, 01, ff, 64, 00, 06, 00, 01, 6f, 6b, ff, 93]: expected Err(MalformedInput { offset: 0, kind: InvalidTilePart }); got Ok(JpipTilePartTransitionReport { status: Available, transitions: [JpipTilePartTransition { tile: 7, tile_part: 0, packet_count: None, data_length: None, functional: false, header: [255, 144, 0, 10, 0, 7, 0, 0, 0, 12, 0, 1, 255, 100, 0, 6, 0, 1, 111, 107, 255, 147] }], needs: [], missing: [] })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_transition_contract_tests.rs:L69-L74`](support/tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L69-L74)

```text
fn contract_cache(bytes: &[u8], stream: u64, tile: u16) -> DatabinCache {
    let mut cache = DatabinCache::new(ResourceLimits::default());
    contract_insert(&mut cache, 2, stream, tile, 0, bytes, true);
    assert!(cache.is_complete(DatabinId::new(2, stream, u64::from(tile)).unwrap()));
    cache
}
```

[`src/jpip_transition_contract_tests.rs:L257-L267`](support/tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L257-L267)

```text
    for declared in [1, 12, 13, valid.len() as u32 - 1] {
        for prefix in [Vec::new(), valid.clone()] {
            let (index, total) = if prefix.is_empty() { (0, 1) } else { (1, 2) };
            let mut undersized = contract_part(7, index, total, Some(0), false, false).header;
            undersized[6..10].copy_from_slice(&declared.to_be_bytes());
            let offset = prefix.len() as u64;
            let mut bytes = prefix;
            bytes.extend(undersized);
            failures.push((bytes, offset, MalformedKind::InvalidTilePart));
        }
    }
```

[`src/jpip_transition_contract_tests.rs:L269-L278`](support/tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L269-L278)

```text
    for (bytes, offset, kind) in failures {
        let cache = contract_cache(&bytes, 77, 7);
        eprintln!("complete malformed bytes={bytes:02x?}; expected {kind:?} at {offset}");
        let expected = Err(DecodeError::MalformedInput { offset, kind });
        let actual = cache.tile_part_transitions(77, 7);
        if actual != expected {
            errors.push(format!("bytes={bytes:02x?}: expected {expected:?}; got {actual:?}"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_transition_contract_tests.rs`](support/tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs)
- [`validation/build-jpip-transition-contract-tests.py`](support/tests/DEV-DFA2D383B6/files/validation/build-jpip-transition-contract-tests.py)

**Build commands** (in order):

```sh
python3 validation/build-jpip-transition-contract-tests.py
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::contract_exact_parse_and_errors -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-DFA2D383B6/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the final assert!(errors.is_empty(), "{}", errors.join("\n")) in contract_exact_parse_and_errors, which accumulates exact Result mismatches. The beginning of the host output tail is truncated, so I do not identify an unseen first mismatch; the first fully visible mismatch is a single 22-byte SOT/COM/SOD header for tile 7 with Psot=12: expected MalformedInput { offset: 0, kind: InvalidTilePart }, but the query returned Available with data_length=None. This directly contradicts clause ecde50e8b39ea19cc97ae9a9's explicit nonzero-Psot-underflow rejection and zero-Psot/None distinction. Entry realism: each failing fixture stores just 22 or 44 bytes in one complete bin using one public fragment insertion, with one or two headers, no recursion and no large allocation. Default HARDENED limits allow 2,147,483,648 input bytes, 1,048,576 fragments and 16,384 tile parts. Large Psot values elsewhere in this test are scalar declarations, not requests to allocate payloads; the tested operation reads header-only databins. Existing native callers/tests likewise construct complete class-2 header bins using fragment/insert and directly call this public query. No owning caller, main header, decoded image or payload must precede this independent unit-contract entry. Fragment construction checks ranges/resource limits, not codestream syntax; accepting malformed network bytes into the cache is admissible and their rejection belongs to parsing. Route confirmed by source, describe_declaration and Rust MIR CFG: contract_cache creates the default cache, inserts class 2 / stream 77 / tile 7 at offset zero with final=true and asserts completion; the direct query takes the complete branch, snapshots databin_view, and calls parse_tile_part_transitions. SOT length, tile identity and COM framing succeed; SOD establishes H=22. For nonzero L=12, declared.checked_sub(header_length) yields None, and map preserves None without producing DecodeError. The transition is pushed and returned; the query's index>0 condition is false for its singleton, so needs remains empty and status becomes Available. The 44-byte cases additionally demonstrate the same error after a valid first transition, at offset 22. This is not a harness setup failure or an oracle based on an unrelated exit code. clause_origin is infer, not rfc_map. The source documentation expressly says None denotes the legal final zero-Psot form, supporting the intended distinction independently of the failing implementation. The precise InvalidTilePart enum remains the review-marked inferred structural-error assumption (also used for wrong tile/SOT and nonfinal zero Psot); no independently quoted standard mandates that enum. Even under that uncertainty, returning successful None for a nonzero impossible length is the demonstrated deviation, not grounds to revise the clause merely to match the code. No production changes or new replay were needed: observation is the supplied host replay, not a local run.

[Standards quote, production lines, and provenance](support/evidence.md#dev-dfa2d383b6)

</details>

</details>

<a id="reproduce-dev-8115d45404"></a>
<details>
<summary>DEV-8115D45404 — Automatic JPIP reconstruction rejects complete legal tile-header bins without optional SOD</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-8115D45404 --output rfc-results/DEV-8115D45404
```


**Standard:** t808-202212.txt § A.3.3

**Required behavior:** Automatically normalize received JPIP header bins into reconstructed decoder inputs without imposing the explicit caller-planned header preparation contract.

**Observed deviation:** Automatic JPIP reconstruction rejects complete legal tile-header bins without optional SOD

All state is constructed with validated public constructors and cache insertion, without private-field mutation. Main header is SOC/SIZ/COD/QCD for one pixel, tile, component and resolution, Rsiz=0, supported progression and positive bounded layers. Received class-2 header is explicitly final and complete, excludes SOT and omits only SOD, which pinned T.808 A.3.3 makes optional; the empty-bin case is expressly permitted. One-byte zero packets are legal empty layer packets. Codestream id and tile/precinct ids are zero, selection is None/all-components, resource limits are default and comfortably exceed the entire setup. No explicit-plan framing contract applies to this automatic entry.

**Recorded failure** (matched by the replay runner):

```text
legal complete JPP header rejected: layers=1 progression=2 comment=false SOD=false: malformed input at byte 0: MissingRequiredStructure
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_snapshot_spec_tests.rs:L12-L13`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L12-L13)

```text
cache.insert(fragment(id(6, 0), 0, main, true)).unwrap();
    cache.insert(fragment(id(2, 0), 0, header, true)).unwrap();
```

[`src/jpip_snapshot_spec_tests.rs:L17-L18`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L17-L18)

```text
assert!(cache.is_complete(id(6, 0)));
    assert!(cache.is_complete(id(2, 0)));
```

[`src/jpip_snapshot_spec_tests.rs:L23-L25`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L23-L25)

```text
cache.build_automatic_snapshot(
        0, None, &JpipPacketSelection::all_components(), &ResourceLimits::default(),
    )
```

[`src/jpip_snapshot_spec_tests.rs:L143-L147`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L143-L147)

```text
for include_sod in [true, false] {
                    let mut header = markers.clone();
                    if include_sod {
                        header.extend_from_slice(&[0xff, 0x93]);
                    }
```

[`src/jpip_snapshot_spec_tests.rs:L149-L152`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L149-L152)

```text
let cache = snapshot_spec_cache(
                        &original[..sot], &header, &vec![0; usize::from(layers)],
                    );
                    match snapshot_spec_call(&cache) {
```

[`src/jpip_snapshot_spec_tests.rs:L169-L171`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L169-L171)

```text
Err(error) => failures.push(format!(
                            "legal complete JPP header rejected: layers={layers} progression={progression} comment={with_comment} SOD={include_sod}: {error}"
                        )),
```

[`src/jpip_snapshot_spec_tests.rs:L177-L177`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L177)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_snapshot_spec_tests.rs`](support/tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs)
- [`snapshot-tests.patch`](support/tests/DEV-8115D45404/files/snapshot-tests.patch)

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

**Exact test invocation:**

```sh
cargo test --lib jpip::tests::snapshot_spec_received_header_normalization -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-8115D45404/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the final assert!(failures.is_empty(), "{}", failures.join("\n")) in snapshot_spec_received_header_normalization, after snapshot_spec_call returned MissingRequiredStructure for SOD-less headers. This is a target-operation failure, not a setup failure or an inference from exit 101. The replay tail explicitly preserves multiple per-case errors, including the one-layer/progression-2/empty-header witness quoted below; its beginning is truncated, so I do not claim the chronologically first individual rejection is visible. The aggregate assertion and surviving individual messages identify the failed check sufficiently.

[Standards quote, production lines, and provenance](support/evidence.md#dev-8115d45404)

</details>

</details>

<a id="reproduce-dev-76187b67f4"></a>
<details>
<summary>DEV-76187B67F4 — Automatic tile-header normalization rejects complete JPP bins without optional SOD</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-76187B67F4 --output rfc-results/DEV-76187B67F4
```


**Standard:** t808-202212.txt § A.3.3

**Required behavior:** For the documented automatic reconstruction consumer, accept a complete valid JPP tile-header data-bin without requiring an optional on-wire SOD marker, and synthesize the SOT/SOD framing needed by the derived codestream.

**Observed deviation:** Automatic tile-header normalization rejects complete JPP bins without optional SOD

Admissible direct unit-contract input: tile 0, valid empty immutable Rust slice obtained from a complete, present class-2 bin, no SOT, omitted optional SOD, one transition, no malformed marker segments, and ordinary successful cache allocation/insertion. T.808 (12/2022) A.3.3 expressly permits optional SOD and requires empty tile-header bins to be sent. A valid main header can supply all coding defaults; no whole codestream is an input to this helper. The first failing entry requires no large tile count or resource stress. This assesses a valid unit entry, not full consumer reachability.

**Recorded failure** (matched by the replay runner):

```text
tile=0 retained=0 SOD=false: valid complete class-2 bin rejected: MalformedInput { offset: 0, kind: MissingRequiredStructure }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_normalization_contract_tests.rs:L27-L27`](support/tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L27)

```text
    let mut headers = vec![Vec::new()];
```

[`src/jpip_normalization_contract_tests.rs:L39-L51`](support/tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L39-L51)

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

[`src/jpip_normalization_contract_tests.rs:L62-L65`](support/tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L62-L65)

```text
                    Err(error) => failures.push(format!(
                        "tile={tile} retained={} SOD={sod}: valid complete class-2 bin rejected: {error:?}",
                        retained.len()
                    )),
```

[`src/jpip_normalization_contract_tests.rs:L74-L77`](support/tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L74-L77)

```text
    assert!(
        failures.is_empty(),
        "{} contract failures; first examples:\n{}",
        failures.len(),
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_normalization_contract_tests.rs`](support/tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs)
- [`register_normalization_contract_tests.py`](support/tests/DEV-76187B67F4/files/register_normalization_contract_tests.py)

**Build commands** (in order):

```sh
python3 register_normalization_contract_tests.py
```

```sh
cargo test --lib --no-run --quiet
```

**Exact test invocation:**

```sh
cargo test --lib --quiet -- jpip::tests::automatic_normalization_accepts_optional_wire_sod_contract --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-76187B67F4/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is failures.is_empty() at src/jpip_normalization_contract_tests.rs:74. Its first collected failure is tile=0, retained=0, SOD=false: a complete empty class-2 bin returns MalformedInput/MissingRequiredStructure at offset 0. This specifically refutes standards-derived clause 153c097eaf82d53210996129, not merely an exit-code expectation.

[Standards quote, production lines, and provenance](support/evidence.md#dev-76187b67f4)

</details>

</details>

<a id="reproduce-dev-efecf7d00d"></a>
<details>
<summary>DEV-EFECF7D00D — Metadata-bin availability incorrectly partitions cached bytes by codestream index</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-EFECF7D00D --output rfc-results/DEV-EFECF7D00D
```


**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Provide deterministic data-bin reassembly and availability/missing-range results for the supported JPIP data-bin classes, so incomplete cached data is not mistaken for successful reconstruction.

**Observed deviation:** Metadata-bin availability incorrectly partitions cached bytes by codestream index

An admissible safe-public-API unit entry: a single logical target's root metadata bin 0, two non-overlapping eight-byte fragments of one 16-byte free box, valid explicit class/CSn framing, offsets 0 and 8, final endpoint 16, default resource limits, one checked positive-length 16-byte request. All parsing, construction and insertion return Ok; no private cache mutation, invalid final-length conflict, excessive resource use, or missing owning-caller requirement. Under T.808 (2022) A.3.6.1 the metadata CSn is semantically ignored, so 0 and 1 do not create different metadata bins.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: metadata CSn must not partition availability
  left: JpipAvailabilityReport { status: Partial, required_bytes: 16, available_bytes: 8, missing: [JpipMissingRange { databin: DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 }, offset: 8, length: Some(8) }] }
 right: JpipAvailabilityReport { status: Available, required_bytes: 16, available_bytes: 16, missing: [] }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_availability_contract_tests.rs:L276-L280`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L276-L280)

```text
    for second_stream in [0_u8, 1] {
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        let mut context = JpipHeaderContext::new();
        for (offset, stream) in [(0_u8, 0), (8, second_stream)] {
```

[`src/jpip_availability_contract_tests.rs:L283-L285`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L283-L285)

```text
            let mut wire = vec![if final_byte { 0x70 } else { 0x60 }, 8, stream, offset, 8];
            wire.extend_from_slice(&metadata[offset as usize..offset as usize + 8]);
            let (message, used) = parse_jpip_message(&wire, &mut context, &limits).unwrap();
```

[`src/jpip_availability_contract_tests.rs:L296-L298`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L296-L298)

```text
            cache
                .insert(JpipFragment::from_message(header, body, &limits).unwrap())
                .unwrap();
```

[`src/jpip_availability_contract_tests.rs:L308-L310`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L308-L310)

```text
        let report = cache
            .availability(&[JpipDatabinRange::new(target, 0, 16).unwrap()])
            .unwrap();
```

[`src/jpip_availability_contract_tests.rs:L322-L326`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L322-L326)

```text
    assert_eq!(reports[0], expected, "same-CSn control");
    assert_eq!(
        reports[1], expected,
        "metadata CSn must not partition availability"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_availability_contract_tests.rs`](support/tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs)
- [`availability_contract.patch`](support/tests/DEV-EFECF7D00D/files/availability_contract.patch)

**Build commands** (in order):

```sh
patch --batch -p1 -i availability_contract.patch
```

```sh
grep -Fq 'include!("jpip_availability_contract_tests.rs");' src/jpip.rs
```

```sh
grep -Fq 'metadata CSn must not partition availability' src/jpip_availability_contract_tests.rs
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib availability_contract_ -- --nocapture --test-threads=1
```

[Machine-readable recipe](support/tests/DEV-EFECF7D00D/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is reports[1] == expected, labelled "metadata CSn must not partition availability", at replayed src/jpip_availability_contract_tests.rs:323. The same-CSn control passed; the changed-CSn case returned Ok(Partial), required_bytes=16, available_bytes=8, with a spurious missing interval [8,16), despite receiving both halves of the same metadata bin. This is a deviation from the targeted documented supported-class reassembly/availability clause (8f9b87d47d2934d844317263), interpreted using its rfc_map origin, not a reason to revise that clause to match the implementation. I read the parent finding and independently retrieved pinned T.808 (2022), A.3.6.1 (hash 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc): it explicitly says the codestream index shall be ignored in any metadata-bin message. It applies to both JPP and JPT streams and to this single logical target/root metadata bin 0. No image-bin cross-codestream merging is asserted. The separately inferred clause describing codestream-sensitive identity cannot override this metadata-specific normative exception.

[Standards quote, production lines, and provenance](support/evidence.md#dev-efecf7d00d)

</details>

</details>

<a id="reproduce-dev-f487b6a88c"></a>
<details>
<summary>DEV-F487B6A88C — Metadata cache identity incorrectly depends on message codestream index</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-F487B6A88C --output rfc-results/DEV-F487B6A88C
```


**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** For metadata messages belonging to one logical target, constructed class-8 identities used for semantic cache lookup shall identify the metadata bin independently of the message codestream index. Retaining a raw CSn for inspection or header inheritance must not prevent equal metadata in-class identifiers from contributing to the same metadata bin.

**Observed deviation:** Metadata cache identity incorrectly depends on message codestream index

A direct public unit-contract test with no constructor preconditions, supported metadata class 8 and ordinary CSn 0/1, metadata bin 0, one logical target per fresh cache, complete well-framed LBox=12 XML payload, offset 0, final length 12, no auxiliary field, and default resource limits. All state is established through DatabinId::new, JpipFragment::new and DatabinCache::insert, without private-field mutation. The nonzero split cases use consistent complementary fragments and admissible insertion orders. No count or buffer limit is approached.

**Recorded failure** (matched by the replay runner):

```text
DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 } / DatabinId { class_identifier: 8, class: Metadata, codestream: 1, in_class: 0 }, split=0, reverse=false: wrong final length/prefix/bytes/intervals for DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 }: None
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_databin_contract_tests.rs:L202-L203`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L202-L203)

```text
    let payload = box32(*b"xml ", b"<a/>");
    assert_eq!(payload.len(), 12);
```

[`src/jpip_databin_contract_tests.rs:L208-L209`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L208-L209)

```text
        let first = DatabinId::new(8, stream, bin).unwrap();
        let second = DatabinId::new(8, stream ^ 1, bin).unwrap();
```

[`src/jpip_databin_contract_tests.rs:L116-L127`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L116-L127)

```text
            let mut cache = DatabinCache::new(ResourceLimits::default());
            let mut fragments = Vec::new();
            if split != 0 {
                fragments.push(fragment(first, 0, &payload[..split], false));
            }
            fragments.push(fragment(second, split as u64, &payload[split..], true));
            if reverse {
                fragments.reverse();
            }
            for value in fragments {
                cache.insert(value).unwrap();
            }
```

[`src/jpip_databin_contract_tests.rs:L135-L136`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L135-L136)

```text
            for identity in [first, second] {
                let status = cache.status(identity);
```

[`src/jpip_databin_contract_tests.rs:L226-L231`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L226-L231)

```text
    assert!(
        failures.is_empty(),
        "{} metadata semantic identity failures; first examples: {:#?}",
        failures.len(),
        &failures[..failures.len().min(12)]
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_databin_contract_tests.rs`](support/tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs)
- [`validation/databin-contract-tests.patch`](support/tests/DEV-F487B6A88C/files/validation/databin-contract-tests.patch)
- [`validation/databin-contract-runner.py`](support/tests/DEV-F487B6A88C/files/validation/databin-contract-runner.py)

**Build commands** (in order):

```sh
bash -eu -c 'patch --batch --forward -p1 < validation/databin-contract-tests.patch; grep -Fq '"'"'include!("jpip_databin_contract_tests.rs");'"'"' src/jpip.rs; cargo test --lib --no-run --locked; python3 validation/databin-contract-runner.py'
```

**Exact test invocation:**

```sh
target/databin-contract-tests jpip::tests::metadata_databin_identity_ignores_codestream --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-F487B6A88C/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing test assertion is the final failures.is_empty() assertion in metadata_databin_identity_ignores_codestream; its first retained counterexample is the split=0, reverse=false case at class 8, bin 0, CSn 0 versus 1. Only the second identity's complete 12-byte XML box is inserted, yet status(first) returns None instead of the completed metadata bin. This violates this exact clause's semantic lookup guarantee, not a requirement to erase raw CSn or make derived Rust equality ignore it. Entry realism: the suite sweeps 132 scalar coordinate pairs, with fresh caches for each of five split points and two orders. Each primary cache accepts at most two payload fragments (12 total bytes) before a one-byte conflict probe; a reference cache contains one 12-byte fragment. Empty-bin checks use final empty fragments. There is no recursive metadata expansion, large bin-index-based allocation, or large payload: u64 boundary identifiers are labels, not counts. Defaults allow 2,147,483,648 input bytes and 1,048,576 fragments. The observed first witness uses only CSn 0/1 and bin 0, so no extreme index or implicit capacity limit is needed. The public direct-unit entry has no requires; class 8 satisfies the classifier contract, fragments use checked public construction, nonfinal empty fragments are skipped, offsets and final lengths are consistent, AUX is None, and each cache represents a single logical target. Production parsing also passes explicit/inherited CSn directly into this constructor, and from_message preserves the resulting identity; the test does not bypass an owning caller invariant. Compiler describe_declaration and CFG show class 8 taking the successful bb0-bb1-bb2-bb3-bb5-bb7 route and retaining the raw CSn. Cache insert/status then call find_databin_index -> same_as -> jpip_databin_identity, whose unconditional codestream equality rejects the metadata alias. At split=0 this makes the completed bin invisible through CSn 0; at split=1 it partitions complementary bytes into two bins, as also observed. The rfc_map parent is authoritative for the stated standards contract: read_rfc_section independently returned T.808 (12/2022), A.3.6.1, pinned SHA 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: 'The codestream index shall be ignored in any message\nwhich has the metadata-bin class identifier.' Its surrounding text explicitly applies to both JPP and JPT metadata from one logical target and allows arbitrary numeric metadata-bin IDs. Existing model agreement cannot override this exception. This assessment is bounded to that cited edition and one-target assumption; the parent notes that the project's newer selected 2026 edition has not been acquired/compared, so no whole-edition conformance conclusion is claimed. The output tail truncates the panic heading and total failure count, but retains the exact failing test, specific semantic-check diagnostics and FAILED verdict; the attribution rests on those concrete checks, not exit 101 alone.

[Standards quote, production lines, and provenance](support/evidence.md#dev-f487b6a88c)

</details>

</details>

<a id="reproduce-dev-51871f3eee"></a>
<details>
<summary>DEV-51871F3EEE — Metadata CSn incorrectly partitions logical bins and double-counts cached payload</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-51871F3EEE --output rfc-results/DEV-51871F3EEE
```


**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** JPIP cached-payload accounting supports deterministic sparse data-bin reassembly and duplicate/overlap consistency, including metadata used for placeholder reconstruction; payload identity must follow the supported bin semantics rather than treating an irrelevant metadata codestream field as a new logical bin.

**Observed deviation:** Metadata CSn incorrectly partitions logical bins and double-counts cached payload

One logical target per cache, metadata class 8 and root ID 0, valid 8/9/15/39-byte free boxes, bounded nonempty spans or legal empty final announcements, consistent overlap bytes and final lengths. Caches originate in DatabinCache::new and fragments in public JpipFragment::new via the existing helper. Default byte/fragment limits exceed the tiny sweep by orders of magnitude. CSn changes only an ignored metadata field under pinned T.808 A.3.6.1. The selected accounting failure predates intentionally conflicting probes, so no inadmissible negative-probe state is needed.

**Recorded failure** (matched by the replay runner):

```text
relabeled n=39 step=2 CSn=7: stored_bytes=41 expected union=31
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip/stored_bytes_tests.rs:L31-L32`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L31-L32)

```text
        let mut canonical = DatabinCache::new(ResourceLimits::default());
        let mut relabeled = DatabinCache::new(ResourceLimits::default());
```

[`src/jpip/stored_bytes_tests.rs:L51-L52`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L51-L52)

```text
            let csn = [0, 1, 7, 127, 128][step % 5];
            let alias = DatabinId::new(8, csn, 0).unwrap();
```

[`src/jpip/stored_bytes_tests.rs:L59-L61`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L59-L61)

```text
                let result = cache.insert(fragment(
                    target, start as u64, &bytes[start..end], end == n,
                ));
```

[`src/jpip/stored_bytes_tests.rs:L64-L65`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L64-L65)

```text
            seen[start..end].fill(true);
            let expected = seen.iter().filter(|&&present| present).count() as u64;
```

[`src/jpip/stored_bytes_tests.rs:L67-L69`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L67-L69)

```text
                check!(cache.stored_bytes() == expected,
                    "{name} n={n} step={step} CSn={csn}: stored_bytes={} expected union={expected}",
                    cache.stored_bytes());
```

[`src/jpip/stored_bytes_tests.rs:L139-L139`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L139)

```text
    assert!(failures.is_empty(), "stored_bytes contract violations:\n{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip/stored_bytes_tests.rs`](support/tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs)
- [`stored-bytes-registration.patch`](support/tests/DEV-51871F3EEE/files/stored-bytes-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i stored-bytes-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip/stored_bytes_tests.rs'"'"').read_text(); assert s.count('"'"'include!("jpip/stored_bytes_tests.rs");'"'"') == 1; assert t.count('"'"'fn stored_bytes_obeys_metadata_bin_identity_and_sparse_union()'"'"') == 1; assert '"'"'expected union={expected}'"'"' in t; print('"'"'verified registration and oracle in replay source'"'"')'
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
cargo test --locked --lib jpip::tests::stored_bytes_obeys_metadata_bin_identity_and_sparse_union -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-51871F3EEE/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The standards-derived clause is sound for same-target class-8 metadata. I read clause_origin (rfc_map), then independently retrieved its pinned T.808 (12/2022) A.3.6.1 through the originating map item 278a47b0ee3cd36f4c3d6f3c, anchor bb808185830df51cc635e32dd74dc317202aedeacc9e1a54cd6eacc7a0d5774c. It explicitly applies to both JPP and JPT and says: 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' Root bin 0 and box metadata match this fixture. This is not a request to normalize the raw header accessor, and agreement between implementation and refinement would not override this receiving-side rule. No claim about a newer standard edition is needed.

[Standards quote, production lines, and provenance](support/evidence.md#dev-51871f3eee)

</details>

</details>

<a id="reproduce-dev-aaf82b7313"></a>
<details>
<summary>DEV-AAF82B7313 — Metadata-bin lookup incorrectly partitions one logical target by message codestream index</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-AAF82B7313 --output rfc-results/DEV-AAF82B7313
```


**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Reassemble and resolve the metadata bins referenced by JPIP placeholders for semantic container reconstruction.

**Observed deviation:** Metadata-bin lookup incorrectly partitions one logical target by message codestream index

One logical target per freshly initialized default-limit cache; four metadata IDs 0..3; valid JP2 root signature/brand, original-content placeholders and independently validated reconstruction; contents-only non-root bins; complementary out-of-order fragments with consistent final lengths; at most 342 transported bytes, eight insertions and depth 2. No production behavior is modified and no cache internals are forged. Root entry and fresh vectors match assemble_semantic_container exactly. T.808 A.3.6.1 requires ignoring metadata CSn, so selecting CSn 7 and mixing CSn 0/7 are admissible, rather than a requirement to supply absent codestream data.

**Recorded failure** (matched by the replay runner):

```text
iteration=11 mode=0 selected-CSn=7: complete=false, output=0/282, missing=[JpipMissingRange { databin: DatabinId { class_identifier: 8, class: Metadata, codestream: 7, in_class: 0 }, offset: 0, length: None }], needs=[JpipSemanticNeed { kind: MetadataBin, identifier: 0 }]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_metadata_contract_tests.rs:L164-L169`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L164-L169)

```text
let (first, last) = match mode {
                    0 => (0, 0),
                    1 => (7, 7),
                    _ => (0, 7),
                };
                insert_split(&mut cache, *bin, bytes, first, last, split);
```

[`src/jpip_metadata_contract_tests.rs:L173-L175`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L173-L175)

```text
for selected in [0, 7, u64::MAX] {
                let (complete, output, missing, needs) = expand(&cache, selected);
                if !complete || output != f.expected || !missing.is_empty() || !needs.is_empty() {
```

[`src/jpip_metadata_contract_tests.rs:L100-L110`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L100-L110)

```text
.expand_metadata_bin(
            csn,
            0,
            true,
            0,
            u32::MAX,
            &mut active,
            &mut output,
            &mut missing,
            &mut needs,
        )
```

[`src/jpip_metadata_contract_tests.rs:L184-L188`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L184-L188)

```text
assert!(
        failures.is_empty(),
        "metadata identity/expansion violated:\n{}",
        failures.join("\n")
    );
```

[`src/jpip_metadata_contract_tests.rs:L84-L84`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L84)

```text
crate::jp2::parse_and_inspect(&expected, &ResourceLimits::default()).unwrap();
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_metadata_contract_tests.rs`](support/tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs)
- [`validation/run_metadata_contract_suite.py`](support/tests/DEV-AAF82B7313/files/validation/run_metadata_contract_suite.py)

**Build commands** (in order):

```sh
python3 validation/run_metadata_contract_suite.py build
```

**Exact test invocation:**

```sh
python3 validation/run_metadata_contract_suite.py metadata_identity_ignores_message_csn
```

[Machine-readable recipe](support/tests/DEV-AAF82B7313/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the case's final assert!(failures.is_empty(), "metadata identity/expansion violated:\n{}", ...), not a build, setup, sanitizer, or resource failure. The retained host output specifically reports iteration=11 mode=0 selected-CSn=7 returning complete=false, zero output instead of 282 bytes, and a spurious missing root-bin request. Mode 0 inserted all metadata under CSn 0; selecting 7 must not hide that metadata. The test also exercises fragmented data spanning CSn 0 and 7, but the same-CSn mode-0 witness already suffices.

[Standards quote, production lines, and provenance](support/evidence.md#dev-aaf82b7313)

</details>

</details>

<a id="reproduce-dev-9d6f9c61de"></a>
<details>
<summary>DEV-9D6F9C61DE — Class-8 metadata lookup incorrectly includes the codestream index in data-bin identity</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-9D6F9C61DE --output rfc-results/DEV-9D6F9C61DE
```


**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Provide complete metadata-bin views through the documented sparse JPIP cache; for standards-facing class-8 identity within one logical target, ignore the message codestream index.

**Observed deviation:** Class-8 metadata lookup incorrectly includes the codestream index in data-bin identity

One fresh public cache represents one logical target. Both IDs are constructed successfully by DatabinId::new, with Metadata class and root in-class ID 0. The payload is a length/type-checked JPEG 2000 family free box. The existing fragment helper calls JpipFragment::new with offset 0, final=true, no auxiliary field, and default limits. insert succeeds and the same-ID view equals the expected payload. Only CSn changes in the failing query, which T.808 12/2022 A.3.6.1 requires ignoring for metadata. No private fields, transport states, allocation-sized numeric labels, recursive nesting, or invalid downstream preconditions are introduced; the decisive cache has one fragment and 25 bytes.

**Recorded failure** (matched by the replay runner):

```text
complete root: stored CSn=0, queried CSn=1, expected=[0, 0, 0, 25, 102, 114, 101, 101, 109, 101, 116, 97, 100, 97, 116, 97, 32, 105, 100, 101, 110, 116, 105, 116, 121], actual=Err(MalformedInput { offset: 0, kind: MissingRequiredStructure })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`databin_view_contract_tests.patch:L137-L138`](support/tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L137-L138)

```text
+            let root = DatabinId::new(8, stored_stream, 0).unwrap();
+            let root_alias = DatabinId::new(8, other_stream, 0).unwrap();
```

[`databin_view_contract_tests.patch:L151-L154`](support/tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L151-L154)

```text
+            cache.insert(fragment(root, 0, &expected, true)).unwrap();
+            assert_eq!(cache.databin_view(root).unwrap().as_bytes(), expected);
+            let actual = cache.databin_view(root_alias).map(JpipByteView::into_bytes);
+            if actual != Ok(expected.clone()) {
```

[`databin_view_contract_tests.patch:L238-L240`](support/tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L238-L240)

```text
+        assert!(
+            failures.is_empty(),
+            "insert -> databin_view violates metadata CSn independence:\n{}"
```

[`run_databin_view_case.py:L33-L33`](support/tests/DEV-9D6F9C61DE/files/run_databin_view_case.py#L33)

```text
    result = subprocess.run([binary, name, "--exact", "--nocapture", "--test-threads=1"])
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`databin_view_contract_tests.patch`](support/tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch)
- [`run_databin_view_case.py`](support/tests/DEV-9D6F9C61DE/files/run_databin_view_case.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i databin_view_contract_tests.patch
```

```sh
python3 -c 'import hashlib; from pathlib import Path; assert hashlib.sha256(Path('"'"'src/jpip.rs'"'"').read_bytes()).hexdigest() == '"'"'3fad65d700b3556fd5ec8d9400494dedaabf9a43a381e19c8bab1afef3ff8ff5'"'"', '"'"'Source overlay missing or modified'"'"''
```

```sh
cargo test --locked --lib --no-run
```

**Exact test invocation:**

```sh
python3 run_databin_view_case.py metadata_identity
```

[Machine-readable recipe](support/tests/DEV-9D6F9C61DE/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final failures.is_empty() assertion in databin_view_metadata_identity_ignores_codestream; its first recorded mismatch is a complete root metadata bin stored with CSn=0 returning MissingRequiredStructure at offset 0 when queried with CSn=1, instead of its 25 original bytes. This is a valid direct cache-unit entry, not a fabricated private representation or a skipped caller prerequisite. The fixture uses DatabinId::new(8, ..., 0), DatabinCache::new, and checked JpipFragment::new/insert; successful same-ID retrieval establishes receipt of the complete payload before the alias query. The free box has a checked length and type. Each metadata cache holds at most 25 payload bytes, at most three accepted fragments and three implementation bins, with no recursive metadata expansion. Across all five CSn pairs and four non-metadata controls the identity test inserts 103 fragments into separate short-lived caches, not one accumulating cache. Default limits allow 2,147,483,648 input bytes, 16,777,216 metadata bytes and 1,048,576 fragments. The decisive CSn pair 0/1 is small even relative to maximum_codestreams=64. Later large numeric labels size no allocation and are not necessary to establish this deviation.

[Standards quote, production lines, and provenance](support/evidence.md#dev-9d6f9c61de)

</details>

</details>

<a id="reproduce-dev-a64a9efad1"></a>
<details>
<summary>DEV-A64A9EFAD1 — Metadata cache identity incorrectly includes CSn, producing spurious semantic dependencies</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-A64A9EFAD1 --output rfc-results/DEV-A64A9EFAD1
```


**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Expose outstanding dependencies of the documented metadata-placeholder assembly facility so incomplete data is distinguishable from successful reconstruction.

**Observed deviation:** Metadata cache identity incorrectly includes CSn, producing spurious semantic dependencies

One cache contains exactly one logical JP2 target; there is no cross-target mixing. Root has signature, JP2 ftyp, one ordinary 28-byte flags=1 phld replacing jp2h, and literal jp2c bytes. Child contains the original jp2h contents with ihdr and colr. Placeholder OrigID is the matching eight-byte child ID and original length/type are preserved. Both fragments use offset zero and final=true via JpipFragment::new and successful cache.insert. Root=150 bytes, child=37 bytes, output=167 bytes; depth one and two fragments satisfy default limits. CSn values 0 and 1 are valid constructor inputs and must be ignored for class-8 identity under the cited edition. u64::MAX is an ID, not an allocation request. The test obtains the assembly from its real public producer; no private state or production behavior is changed.

**Recorded failure** (matched by the replay runner):

```text
bin=18446744073709551615 root_csn=0 child_csn=1 query_csn=0 complete=true: actual=(Partial, [(MetadataBin, 18446744073709551615)], None) expected=(Available, [],
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`semantic-needs.patch:L189-L189`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L189)

```text
+                [(0, 0, 0), (7, 7, 7), (0, 1, 0), (0, 0, 1), (3, 5, 7)]
```

[`semantic-needs.patch:L200-L200`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L200)

```text
+                        DatabinId::new(8, root_csn, 0).unwrap(),
```

[`semantic-needs.patch:L210-L210`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L210)

```text
+                                DatabinId::new(8, child_csn, bin).unwrap(),
```

[`semantic-needs.patch:L222-L227`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L222-L227)

```text
+                    let report = cache.assemble_semantic_container(query_csn).unwrap();
+                    let needs: Vec<_> = report
+                        .semantic_needs()
+                        .iter()
+                        .map(|need| (need.kind(), need.identifier()))
+                        .collect();
```

[`semantic-needs.patch:L257-L261`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L257-L261)

```text
+        assert!(
+            violations.is_empty(),
+            "metadata CSn must not create spurious dependencies:\n{}",
+            violations.join("\n")
+        );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`semantic-needs.patch`](support/tests/DEV-A64A9EFAD1/files/semantic-needs.patch)

**Build commands** (in order):

```sh
bash -c 'patch --batch --forward -p1 < semantic-needs.patch && grep -Fq '"'"'fn semantic_metadata_needs_ignore_metadata_codestream_index()'"'"' src/jpip.rs && grep -Fq '"'"'exact outstanding dependencies'"'"' src/jpip.rs && grep -Fq '"'"'for split in splits'"'"' src/jpip.rs && cargo test --locked --lib --no-run && cp "$(find target/debug/deps -maxdepth 1 -type f -executable -name '"'"'jpeg2000_rs-*'"'"')" target/semantic-needs-tests'
```

**Exact test invocation:**

```sh
target/semantic-needs-tests --exact jpip::tests::semantic_metadata_needs_ignore_metadata_codestream_index --nocapture
```

[Machine-readable recipe](support/tests/DEV-A64A9EFAD1/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the test's final assert!(violations.is_empty()), which collects mismatches between reported dependencies/status/container and the standard-derived expectation; it is not a constructor failure, timeout, or arbitrary exit. The retained host output specifically shows complete root/child metadata yielding Partial with a MetadataBin need instead of Available with no needs. This violates clause 14ba92bb9ab4b9fc4b9ed06e in its rfc_map context: reported outstanding dependencies must describe genuinely incomplete target metadata, not an artificial CSn partition. I read the parent finding and the pinned T.808 (12/2022) A.3.6.1 through read_rfc_section using origin item 4bbbf5c259181f66f15ae648. It explicitly requires ignoring CSn on metadata messages, allows arbitrary numeric metadata-bin IDs, and assigns root ID 0. No claim about the unacquired 2026 edition is needed.

[Standards quote, production lines, and provenance](support/evidence.md#dev-a64a9efad1)

</details>

</details>

<a id="reproduce-dev-b80f74bb23"></a>
<details>
<summary>DEV-B80F74BB23 — JPIP single-codestream placeholders reject a permitted present-but-ignored NCS field</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-B80F74BB23 --output rfc-results/DEV-B80F74BB23
```


**Standard:** t808-202212.txt § A.3.6.3

**Required behavior:** Extract incremental codestream identifiers and stream counts from received metadata placeholders for the semantic assembler.

**Observed deviation:** JPIP single-codestream placeholders reject a permitted present-but-ignored NCS field

First rejected payload: flags=4, OrigID=7, ordinary OrigBH jp2c length 256, intermediate EquivID=9 and ordinary EquivBH jp2c length 256, CSID=0, present NCS=0. Total 48 payload bytes, outer ordinary phld length 56. Complete required fields, valid slices, supported low flags, no unknown extensions, no stream-ID overflow. T.808 (12/2022) Table A.3 requires effective NCS=1 for 01xx even with a present zero-valued NCS. Referenced lengths do not allocate content, and no stream-array expansion or cache-state fabrication occurs.

**Recorded failure** (matched by the replay runner):

```text
736 legal placeholders rejected; first failures:
flags=0x4, CSID=0x0, NCS=Some(0), wide=false: MalformedInput { offset: 0, kind: InvalidBoxContents }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_placeholder_contract_tests.rs:L115-L123`](support/tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L115-L123)

```text
                    let payload = contract_payload(
                        flags,
                        7,
                        &header,
                        Some((9, &header)),
                        Some(stream),
                        count,
                    );
                    match parse_jpip_placeholder(&payload) {
```

[`src/jpip_placeholder_contract_tests.rs:L50-L53`](support/tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L50-L53)

```text
    let mut reader = crate::jp2::BoxReader::new(&framed, &ResourceLimits::default()).unwrap();
    let received = reader.next_box_ref().unwrap().unwrap();
    assert_eq!(received.payload, payload);
    assert!(reader.next_box_ref().is_none());
```

[`src/jpip_placeholder_contract_tests.rs:L113-L114`](support/tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L113-L114)

```text
                    let expected = if flags & 8 == 0 { 1 } else { count.unwrap() };
                    assert!(stream.checked_add(u64::from(expected) - 1).is_some());
```

[`src/jpip_placeholder_contract_tests.rs:L173-L176`](support/tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L173-L176)

```text
    assert!(
        failures.is_empty(),
        "{} legal placeholders rejected; first failures:\n{}",
        failures.len(),
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_placeholder_contract_tests.rs`](support/tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs)
- [`validation/build_placeholder_contract_tests.py`](support/tests/DEV-B80F74BB23/files/validation/build_placeholder_contract_tests.py)

**Build commands** (in order):

```sh
python3 validation/build_placeholder_contract_tests.py
```

**Exact test invocation:**

```sh
target/placeholder-contract-tests jpip::tests::placeholder_contract_stream_identifiers_and_counts --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-B80F74BB23/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() at src/jpip_placeholder_contract_tests.rs:173, reporting 736 legal placeholders rejected. Its first concrete case is flags=4, CSID=0, NCS=Some(0), wide=false, returning InvalidBoxContents. This is a valid direct unit-contract entry, not an incomplete DatabinCache fixture. The input is a complete 48-byte payload (56 bytes including ordinary phld framing): Flags/OrigID/8-byte OrigBH/EquivID/8-byte EquivBH/CSID/4-byte NCS. Both referenced headers declare length 256 and type jp2c; their content is not required inside a placeholder. The test validates framing and payload extraction using BoxReader before calling the parser. Zero is permitted in this unused NCS field: effective count is one. The output also independently reports rejection with NCS=Some(1), so the deviation does not depend on zero or an extreme count.
Resource realism: the main matrix is 2 header widths x 23 identifiers x 8 flag values x 5 count representations = 1840 independent parser calls. Each payload is at most 64 bytes (72 framed), with no nesting, recursive reconstruction, large referenced-content allocation, or stream enumeration. NCS and CSID are scalar wire fields; the single-stream failures always request one effective stream, including when encoded NCS is UINT32_MAX. These are far below HARDENED input/metadata limits (2147483648/16777216 bytes), depth 32, and codestream capacity 64. Multi-stream scalar parse tests do not invoke downstream reconstruction or claim it must accept billions of streams. Real caller expand_metadata_bin likewise passes box_ref.payload directly after BoxReader identifies phld. The owning parser has no cache-state or resource-limit parameter; this test respects its slice and read-helper requirements.
Authority is the rfc_map parent, not correlated implementation models. I read its pinned T.808 (12/2022) A.3.6.3, SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc. Table A.3 explicitly requires 01xx to treat NCS as 1 regardless of its actual value when present; surrounding prose permits termination after CSID but does not require it, and requires intermediate fields even if unused. The harness includes these intermediate fields. Thus this is a concrete refutation of the mapped extraction clause under its originating standards interpretation, not a clause requiring revision to match the parser. REFERENCES.md explicitly leaves the newer 2026 edition unaudited; this finding is bounded to the cited 2022 text and does not establish a newer-edition conformance failure.
Source and MIR identify the exact route: after complete headers, flags & 4 reads CSID and advances position to 44. Since flags & 8 is zero, MIR bb106 -> bb109 sets count=1 without consuming optional NCS. bb124 observes 44 != 48, bb125/bb126/bb127 bypass the provisional-layout diagnostic (48 != original_end+24, which is 44), and bb130/bb131 return InvalidBoxContents. The required result was successful extraction of CSID=0 and stream_count=1. The 736 failures equal both widths x 23 IDs x four 01xx flags x four present NCS values. The compatibility diagnostic remains an error, not an accepting path. No production modification, arbitrary nonzero exit, or unrelated unused-high-bit failure is used as evidence; the recorded successful build and concrete parser error establish this specific deviation.

[Standards quote, production lines, and provenance](support/evidence.md#dev-b80f74bb23)

</details>

</details>

<a id="reproduce-dev-6744e122c7"></a>
<details>
<summary>DEV-6744E122C7 — Stream-equivalent metadata payload is reconstructed with the original box identity</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-6744E122C7 --output rfc-results/DEV-6744E122C7
```


**Standard:** t808-202212.txt § A.3.6.3

**Required behavior:** Reconstruct the supported metadata placeholder representation into correctly identified container boxes.

**Observed deviation:** Stream-equivalent metadata payload is reconstructed with the original box identity

A direct helper unit entry over public-API-established cache state, with a legal compact phld, complete bin 1, complete contents-only bin 2 on the failing phase, CSn 0 throughout, flags=2, ignored unavailable OrigID 99, EquivID 2, original xref header, equivalent colr header, consistent depth/budget and noncyclic ancestor identity [0]. Only 67 bytes maximum are cached and maximum reached depth is 2. Bin 1 represents superbox contents, not a complete file; the unit assumes the standard-permitted JPX xref-to-colr context, without claiming public assembler acceptance or full JPX conformance. The helper has no requirement to retrieve or validate the ancestor bin or unavailable original contents.

**Recorded failure** (matched by the replay runner):

```text
xref->colr original-XLBox=false, equivalent-XLBox=false: got [0, 0, 0, 15, 120, 114, 101, 102, 1, 0, 0, 0, 0, 0, 17], expected [0, 0, 0, 15, 99, 111, 108, 114, 1, 0, 0, 0, 0, 0, 17]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_metadata_contract_tests.rs:L267-L273`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L267-L273)

```text
let original = header(*b"xref", 32, original_extended);
            let equivalent = header(*b"colr", color.len(), equivalent_extended);
            let mut fields = 2_u32.to_be_bytes().to_vec();
            fields.extend_from_slice(&99_u64.to_be_bytes());
            fields.extend_from_slice(&original);
            fields.extend_from_slice(&2_u64.to_be_bytes());
            fields.extend_from_slice(&equivalent);
```

[`src/jpip_metadata_contract_tests.rs:L282-L285`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L282-L285)

```text
cache
                .insert(fragment(id(8, 1), 0, &placeholder, true))
                .unwrap();
            let mut active = vec![0];
```

[`src/jpip_metadata_contract_tests.rs:L309-L326`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L309-L326)

```text
cache.insert(fragment(id(8, 2), 0, &color, true)).unwrap();
            missing.clear();
            needs.clear();
            assert!(
                cache
                    .expand_metadata_bin(
                        0,
                        1,
                        true,
                        1,
                        u32::MAX - 1,
                        &mut active,
                        &mut output,
                        &mut missing,
                        &mut needs,
                    )
                    .unwrap()
            );
```

[`src/jpip_metadata_contract_tests.rs:L330-L335`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L330-L335)

```text
let expected = boxed(*b"colr", &color, equivalent_extended);
            if output != expected {
                failures.push(format!(
                    "xref->colr original-XLBox={original_extended}, equivalent-XLBox={equivalent_extended}: got {output:?}, expected {expected:?}"
                ));
            }
```

[`src/jpip_metadata_contract_tests.rs:L338-L342`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L338-L342)

```text
assert!(
        failures.is_empty(),
        "wrong stream-equivalent box header:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_metadata_contract_tests.rs`](support/tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs)
- [`validation/run_metadata_contract_suite.py`](support/tests/DEV-6744E122C7/files/validation/run_metadata_contract_suite.py)

**Build commands** (in order):

```sh
python3 validation/run_metadata_contract_suite.py build
```

**Exact test invocation:**

```sh
python3 validation/run_metadata_contract_suite.py stream_equivalent_uses_equivalent_box_identity
```

[Machine-readable recipe](support/tests/DEV-6744E122C7/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failed assertion is the final failures.is_empty() check, reported as 'wrong stream-equivalent box header'. All preceding admission, missing-payload, completion, active-stack and diagnostic assertions passed. The first compact/compact case emits TBox xref (120,114,101,102) instead of colr (99,111,108,114), violating the exact correctly-identified-box clause; the other three cases also expose use of the original rather than equivalent header width.

[Standards quote, production lines, and provenance](support/evidence.md#dev-6744e122c7)

</details>

</details>

<a id="reproduce-dev-174ec5fee4"></a>
<details>
<summary>DEV-174EC5FEE4 — JPH component decoding incorrectly requires colr when UnkC=1</summary>

**Run this finding:**

```sh
python doc_deviations/support/run_tests.py --repo . --finding DEV-174EC5FEE4 --output rfc-results/DEV-174EC5FEE4
```


**Standard:** t814-201906.txt § D.2

**Required behavior:** Decode supported JPH containers to typed component sample planes, preserving component properties independently of the separate explicit color-conversion APIs.

**Observed deviation:** JPH component decoding incorrectly requires colr when UnkC=1

Rust-valid byte slices and default options enter the public API, with no private-state corruption, bypassed owning caller or production modifications. Inputs are small, bounded, canonical JPH-branded containers with a single supported Part 15 payload, valid IHDR/BPCC and empty packets. Paired explicit-colour controls pass complete typed image checks. The sole metadata change is the T.814:2019 D.2-permitted combination UnkC=1, absent colr and absent cdef (hence no Typi=0). D.3 BR/MinV/CL are canonical. Max dimensions 16x14, one/three components and one tile are far below hardened limits. Unspecified colour does not require rendering because this API returns native component planes.

**Recorded failure** (matched by the replay runner):

```text
D.2 component-only JPH rejected: MalformedInput { offset: 40, kind: MissingRequiredStructure }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode/tests/jph_contract.rs:L63-L63`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L63)

```text
    ihdr.extend_from_slice(&[255, 7, u8::from(!colour), 0]);
```

[`src/decode/tests/jph_contract.rs:L70-L75`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L70-L75)

```text
    if colour {
        let space = if planes.len() == 1 { 17 } else { 16 };
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, space]));
    }
    // D.2: no colr implies UnkC=1 and no channel definition with Typ=0.
    // There is no cdef box here, hence no Typ fields at all.
```

[`src/decode/tests/jph_contract.rs:L163-L165`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L163-L165)

```text
        let known = container(&cs, size, planes, b"jph \0\0\0\0jph ", true);
        let decoded = decode_jph(&known, &options).expect("supported JPH with explicit colour");
        assert_zero_image(&decoded, size, planes);
```

[`src/decode/tests/jph_contract.rs:L169-L170`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L169-L170)

```text
        let unspecified = container(&cs, size, planes, b"jph \0\0\0\0jph ", false);
        match decode_jph(&unspecified, &options) {
```

[`src/decode/tests/jph_contract.rs:L181-L184`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L181-L184)

```text
    assert!(
        missing_colour_failures.is_empty(),
        "UnkC=1, no colr, no cdef must return the same typed planes: {missing_colour_failures:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode/tests/jph_contract.rs`](support/tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs)

**Build commands** (in order):

```sh
python3 -c 'from pathlib import Path; p=Path('"'"'src/decode.rs'"'"'); s=p.read_text(); anchor='"'"'#[cfg(test)]\nmod tests {\n'"'"'; assert s.count(anchor)==1; p.write_text(s.replace(anchor, anchor+'"'"'    mod jph_contract;\n'"'"', 1))'
```

```sh
cargo test --lib --locked --no-run
```

**Exact test invocation:**

```sh
cargo test --lib --locked decode::tests::jph_contract::typed_planes_independent_of_colour_conversion -- --exact --nocapture
```

[Machine-readable recipe](support/tests/DEV-174EC5FEE4/recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is missing_colour_failures.is_empty() at src/decode/tests/jph_contract.rs:181: every component-only counterpart returns MalformedInput { offset: 40, kind: MissingRequiredStructure }, instead of the same typed planes as its explicit-colour control. This is a reproduced admission deviation from clause 6a3078c49e79034ec262df2c, not a bad pixel oracle or an excessive fixture. Realism was checked first: 24 sequential cases, dimensions at most 16x14, origins at most 3, one or three components, one tile/layer/resolution, no decomposition, shallow box nesting, containers under 200 bytes, and at most 5,376 accounted sample bytes per image even using a deliberately loose three-full-size-plane bound. Default DecodeOptions uses ResourceLimits::HARDENED (1,000,000 per dimension, 100,000,000 pixels, 64 components, 16,384 tiles, 32 box depth, and 2,147,483,648 input/output bytes). Nothing approaches platform or configured capacity. The public validation_decoder likewise passes ordinary file bytes and options directly to decode_jph, and existing native JPH tests use the same public entry with 1x1 images and default options. No caller-owned state or internal fixture is fabricated.

[Standards quote, production lines, and provenance](support/evidence.md#dev-174ec5fee4)

</details>

</details>
