# Final JPIP witnesses

These original native regressions add **one new finding, F31**, and another
reproduction of **existing F11**. They concern the locally acquired
[T.808 (12/2022)](https://www.itu.int/rec/T-REC-T.808-202212-I/en),
not an assertion that the project's newer selected edition is identical.

| ID | Human-readable discrepancy | Document citation | Deviating code | Expected versus observed |
|---|---|---|---|---|
| **F31: new** | The reader rejects an integer merely because it has a leading zero group, although the standard's continuation/payload encoding permits it. | A.2.1, Figures A.2-A.3, printed pp.12-13; A.2.2/Table A.2 for metadata Class8. | [`read_vbas`, jpip.rs:361-389](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L361-L389); [`jpip_vbas_first_is_canonical`, refinement.rs:2347-2349](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/refinement.rs#L2347-L2349). | Class bytes `80 08` still mean8; length bytes `80 01` still mean1. The equivalent canonical message passes, but the padded Class and length variants return `InvalidJpipVbas` at offsets1 and4. |
| **F11: additional evidence** | The first dependent message should start with default Class0/CSn0. Response ingestion rejects it instead. | A.2.1, Table A.1 and initial Class/CSn rules, printed pp.12-13. | [`parse_jpip_message`, jpip.rs:236-290](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L236-L290), reached through [`ingest_jpip_response`](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpip.rs#L3504-L3529). | `30 00 01 00` should retain one empty precinct packet in bin0. Native ingestion returns `InvalidJpipHeader` at offset0. Same root as published F11, not another defect. |

## Inspect the evidence

| ID | Original source | Registration | Original native result |
|---|---|---|---|
| F31 | [VBAS harness](cases/55730ec94ef7342e41f0ebc5/artifacts/validation/jpip_vbas_contract.rs) | [Installer](cases/55730ec94ef7342e41f0ebc5/artifacts/validation/install-jpip-vbas-contract.py) | [Recorded output](cases/55730ec94ef7342e41f0ebc5/recorded-test.log) |
| F11 | [Ingestion harness](cases/66aff522bc300198b9d75ec3/artifacts/src/jpip_ingest_contract_tests.rs) | [Patch](cases/66aff522bc300198b9d75ec3/artifacts/validation/jpip-ingest-contract.patch) | [Recorded output](cases/66aff522bc300198b9d75ec3/recorded-test.log) |

F31's public-parser inputs are `60 80 08 00 00 01 ab 5a` and
`60 08 00 00 80 01 ab 5a`. Each describes an independent metadata message:
Class8, CSn0, bin0, offset0, one-byte body`ab`. Byte`5a` is a trailing sentinel
that should remain unconsumed, not part of the message. Neither witness
depends on inherited-header state, excessive resources, large integers,
or an unsupported arbitrary-size VBAS.

The host separately checked the original PDF figures as well as the relevant
prose. The executable canonical helper and its correlated Verus model agree
with each other; that agreement does not make the extra rejection normative.
All the padded fields share one root.

F11 is bare ordered Annex A ingestion, not an unreliable transport packet
governed by H.2's explicit-first-header rule. Its original metadata-validation
warnings remain in the recorded result; the emitted native harness genuinely
exists and executed.

These are bounded parser/cache witnesses, not full encoded-image or transport
conformance proofs. No production fixes, official conformance signals, completed
Verus proofs, or newer-edition equivalence are claimed.
