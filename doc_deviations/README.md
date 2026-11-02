# Standards-tested deviations

Source: `jpeg2000-doc-deviations-20261102@537185f8e355`.

Each **Reproduce** link opens one finding's test instructions: requirement, input, exact command, observed failure, and the cited test lines. Build helpers and supporting source quotes are kept on that page, not in this index.

53 reproduced clause-level findings joined to standards mappings (not necessarily distinct defects). A mapping is not by itself a conformance proof.

| ID | Observed deviation | Standard | Reproduce |
|---|---|---|---|
| DEV-0A9BF676EF | Explicit precinct transcode publishes non-LRCP effective tile coding styles | t800-201906.txt § A.4.2 | [Reproduce](tests/DEV-0A9BF676EF/README.md) |
| DEV-926EF1A301 | Nonzero Psot shorter than retained tile-part header is accepted as an unspecified data length | t800-201906.txt § A.4.2 | [Reproduce](tests/DEV-926EF1A301/README.md) |
| DEV-64FD2DDB2B | Ordinary COC accepts zero precinct exponents above resolution zero | t800-201906.txt § A.6.2 | [Reproduce](tests/DEV-64FD2DDB2B/README.md) |
| DEV-C56978D32B | PLM parser rejects standard one-octet Nplm framing as InvalidMarkerLength | t800-201906.txt § A.7.2 | [Reproduce](tests/DEV-C56978D32B/README.md) |
| DEV-4F2CFF1E64 | Metadata inspection rejects valid T.800 (06/2019) PLM framing by treating Nplmi as four bytes | t800-201906.txt § A.7.2 | [Reproduce](tests/DEV-4F2CFF1E64/README.md) |
| DEV-B6570A1D68 | Metadata inspection rejects standard 8-bit PLM length framing | t800-201906.txt § A.7.2 | [Reproduce](tests/DEV-B6570A1D68/README.md) |
| DEV-80E8727274 | Precinct validator accepts non-LRCP effective tile COD overrides | t800-201906.txt § A.7.3 | [Reproduce](tests/DEV-80E8727274/README.md) |
| DEV-C36C516466 | Tile-header parser rejects consistent combined PLM and PLT metadata permitted by T.800 A.7.3 | t800-201906.txt § A.7.3 | [Reproduce](tests/DEV-C36C516466/README.md) |
| DEV-709CD8127B | SOP-enabled progression decoding rejects standards-permitted omission of SOP | t800-201906.txt § A.8.1 | [Reproduce](tests/DEV-709CD8127B/README.md) |
| DEV-EA51ACDA28 | Inline EPH inspection rejects a valid header with terminal FF stuffing | t800-201906.txt § A.8.2 | [Reproduce](tests/DEV-EA51ACDA28/README.md) |
| DEV-2F140D1710 | Native selected-color context excludes the CRG-registered predecessor sample | t800-201906.txt § A.9.1 | [Reproduce](tests/DEV-2F140D1710/README.md) |
| DEV-355FE751F9 | Reduced selected decode applies tile-local nonlinear transforms on unreduced tile coordinates | t800-201906.txt § B.1 | [Reproduce](tests/DEV-355FE751F9/README.md) |
| DEV-404E919A62 | Arbitrary-filter lifting selects the wrong source neighbour for odd-origin axes | t800-201906.txt § F.3.2 | [Reproduce](tests/DEV-404E919A62/README.md) |
| DEV-37A5BF80E8 | Part 1 vertical 9/7 synthesis omits odd-origin singleton halving | t800-201906.txt § F.3.5 | [Reproduce](tests/DEV-37A5BF80E8/README.md) |
| DEV-2D75F3E8F6 | JP2 validation accepts additional top-level JPEG 2000 signature boxes | t800-201906.txt § I.5.1 | [Reproduce](tests/DEV-2D75F3E8F6/README.md) |
| DEV-056D9FBFB2 | JP2 validation accepts duplicate File Type boxes as opaque metadata | t800-201906.txt § I.5.2 | [Reproduce](tests/DEV-056D9FBFB2/README.md) |
| DEV-629A0A9C39 | JP2 header rejects reserved-method later colr boxes that the reader must ignore | t800-201906.txt § I.5.3.3 | [Reproduce](tests/DEV-629A0A9C39/README.md) |
| DEV-2032F100C3 | JP2 palette validator accepts NE above the T.800 (06/2019) limit of 1024 | t800-201906.txt § I.5.3.4 | [Reproduce](tests/DEV-2032F100C3/README.md) |
| DEV-3DB90259D8 | cdef parser rejects valid RGB colour associations exceeding physical channel count | t800-201906.txt § I.5.3.6 | [Reproduce](tests/DEV-3DB90259D8/README.md) |
| DEV-457ACAB2C3 | Ordinary JP2 rejects a resource-admitted second contiguous codestream instead of allowing first-codestream display | t800-201906.txt § I.5.4 | [Reproduce](tests/DEV-457ACAB2C3/README.md) |
| DEV-8825E34C8B | Inherited main-header component NLT overrides first-tile-header default | t801-202106.txt § A.3.10 | [Reproduce](tests/DEV-8825E34C8B/README.md) |
| DEV-49C7BAFDE3 | Reversible decorrelation resolver rejects normative N(N+1) coefficient arrays using an N² count | t801-202106.txt § J.3.1.1.3 | [Reproduce](tests/DEV-49C7BAFDE3/README.md) |
| DEV-61C38E5752 | Inverse LUT NLT normalizes reconstructed samples by 2^precision instead of 2^precision−1 | t801-202106.txt § K | [Reproduce](tests/DEV-61C38E5752/README.md) |
| DEV-AFC4DEDE80 | JPX Association inspection accepts fewer than two immediate children | t801-202106.txt § M.11.11 | [Reproduce](tests/DEV-AFC4DEDE80/README.md) |
| DEV-AACA35161C | JPX container validation accepts a nonempty codestream-header sequence shorter than the codestream sequence | t801-202106.txt § M.11.6 | [Reproduce](tests/DEV-AACA35161C/README.md) |
| DEV-CBF63FCFA1 | JPX parser admits nonzero codestream-header deficits and exposes unassociated codestreams | t801-202106.txt § M.11.6 | [Reproduce](tests/DEV-CBF63FCFA1/README.md) |
| DEV-7F4AE3ED02 | JPX layer-reference validation accepts mixed registration-box presence | t801-202106.txt § M.11.7.7 | [Reproduce](tests/DEV-7F4AE3ED02/README.md) |
| DEV-C635AC0588 | Indexed JPX composition paints a zero-LIFE nonpersistent instruction that should be skipped | t801-202106.txt § M.5.3.2.1 | [Reproduce](tests/DEV-C635AC0588/README.md) |
| DEV-9A1ABE441F | Bounded standard JPIP VBAS fields with redundant zero groups are rejected as malformed | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-9A1ABE441F/README.md) |
| DEV-A47C95C101 | F11 root overlap: ingestion rejects an initial dependent header instead of applying Annex A identifier defaults | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-A47C95C101/README.md) |
| DEV-34CC912F98 | Fresh JPIP history rejects the standard initial inherited header instead of selecting class 0 and codestream 0 | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-34CC912F98/README.md) |
| DEV-30865B7D2D | Fresh JPIP form-01 messages reject standard default class and codestream identifiers | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-30865B7D2D/README.md) |
| DEV-F1E3AE04A3 | Fresh JPIP parser rejects standard inherited headers instead of applying initial zero defaults | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-F1E3AE04A3/README.md) |
| DEV-772E004106 | Initial standard dependent JPIP header rejected instead of using zero identifier defaults | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-772E004106/README.md) |
| DEV-CA85C0D7B5 | Initial dependent JPIP header is rejected instead of applying class-zero default | t808-202212.txt § A.2.1 | [Reproduce](tests/DEV-CA85C0D7B5/README.md) |
| DEV-77C8A9E6C3 | JPIP classifier assigns standard data-bin semantics to reserved class identifiers 3 and 7 | t808-202212.txt § A.2.2 | [Reproduce](tests/DEV-77C8A9E6C3/README.md) |
| DEV-E9BA9C12DC | JPIP parser rejects legal initial inherited-form header instead of defaulting Class and CSn to zero | t808-202212.txt § A.2.2 | [Reproduce](tests/DEV-E9BA9C12DC/README.md) |
| DEV-2D35E4323C | Reserved T.808 (12/2022) classes 3 and 7 receive standard TileHeader and MainHeader semantics | t808-202212.txt § A.2.2 | [Reproduce](tests/DEV-2D35E4323C/README.md) |
| DEV-7A30EEC46F | Reserved JPIP class identifiers 3 and 7 admitted as semantic header classes | t808-202212.txt § A.2.2 | [Reproduce](tests/DEV-7A30EEC46F/README.md) |
| DEV-76BF9F9717 | JPIP geometric mapping rejects admissible clipped precincts and invents empty-resolution precincts | t808-202212.txt § A.3.2.1 | [Reproduce](tests/DEV-76BF9F9717/README.md) |
| DEV-E2DF8B9FAC | Automatic JPIP header normalization rejects legal SOD-omitted tile-header bins | t808-202212.txt § A.3.3 | [Reproduce](tests/DEV-E2DF8B9FAC/README.md) |
| DEV-DFA2D383B6 | Complete tile headers with undersized nonzero Psot are reported Available with unspecified data length | t808-202212.txt § A.3.3 | [Reproduce](tests/DEV-DFA2D383B6/README.md) |
| DEV-8115D45404 | Automatic JPIP reconstruction rejects complete legal tile-header bins without optional SOD | t808-202212.txt § A.3.3 | [Reproduce](tests/DEV-8115D45404/README.md) |
| DEV-76187B67F4 | Automatic tile-header normalization rejects complete JPP bins without optional SOD | t808-202212.txt § A.3.3 | [Reproduce](tests/DEV-76187B67F4/README.md) |
| DEV-EFECF7D00D | Metadata-bin availability incorrectly partitions cached bytes by codestream index | t808-202212.txt § A.3.6.1 | [Reproduce](tests/DEV-EFECF7D00D/README.md) |
| DEV-F487B6A88C | Metadata cache identity incorrectly depends on message codestream index | t808-202212.txt § A.3.6.1 | [Reproduce](tests/DEV-F487B6A88C/README.md) |
| DEV-51871F3EEE | Metadata CSn incorrectly partitions logical bins and double-counts cached payload | t808-202212.txt § A.3.6.1 | [Reproduce](tests/DEV-51871F3EEE/README.md) |
| DEV-AAF82B7313 | Metadata-bin lookup incorrectly partitions one logical target by message codestream index | t808-202212.txt § A.3.6.1 | [Reproduce](tests/DEV-AAF82B7313/README.md) |
| DEV-9D6F9C61DE | Class-8 metadata lookup incorrectly includes the codestream index in data-bin identity | t808-202212.txt § A.3.6.1 | [Reproduce](tests/DEV-9D6F9C61DE/README.md) |
| DEV-A64A9EFAD1 | Metadata cache identity incorrectly includes CSn, producing spurious semantic dependencies | t808-202212.txt § A.3.6.1 | [Reproduce](tests/DEV-A64A9EFAD1/README.md) |
| DEV-B80F74BB23 | JPIP single-codestream placeholders reject a permitted present-but-ignored NCS field | t808-202212.txt § A.3.6.3 | [Reproduce](tests/DEV-B80F74BB23/README.md) |
| DEV-6744E122C7 | Stream-equivalent metadata payload is reconstructed with the original box identity | t808-202212.txt § A.3.6.3 | [Reproduce](tests/DEV-6744E122C7/README.md) |
| DEV-174EC5FEE4 | JPH component decoding incorrectly requires colr when UnkC=1 | t814-201906.txt § D.2 | [Reproduce](tests/DEV-174EC5FEE4/README.md) |
