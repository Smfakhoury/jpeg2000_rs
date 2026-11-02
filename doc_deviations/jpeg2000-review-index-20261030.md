# JPEG 2000 review tables and runnable evidence

Source revision:
[`microsoft/jpeg2000_rs@69fe0bce9024`](https://github.com/microsoft/jpeg2000_rs/tree/69fe0bce9024ab5a4cb6556e3ebdf91134324148).
Production source, models and Cargo files are unchanged.

## Review packages

| Package | Human-readable table | Runnable evidence |
|---|---|---|
| Original27 review rows, F01-F27 | [Findings, citations, code and scope](jpeg2000-review-package-20261029/FINDINGS.md) | [Instructions](jpeg2000-review-package-20261029/README.md) / [archive](jpeg2000-review-package-20261029.tar.gz) |
| Three additional rows, F28-F30 | [Findings, citations, code and scope](jpeg2000-review-supplement-20261030/FINDINGS.md) | [Instructions](jpeg2000-review-supplement-20261030/README.md) / [archive](jpeg2000-review-supplement-20261030.tar.gz) |
| Two transform discrepancies | Table below; `WITNESSES.md` inside the archive | [Pinned source, tests and native log](jpeg2000-transform-witnesses-20261030.tar.gz) |
| New F31, plus additional evidence for existing F11 | [Final JPIP findings table](jpeg2000-final-jpip-witnesses-20261030/FINDINGS.md) | [Instructions](jpeg2000-final-jpip-witnesses-20261030/README.md) / [archive](jpeg2000-final-jpip-witnesses-20261030.tar.gz) |
| **Separately qualified** U-extension observation | [U1 table and applicability limits](jpeg2000-u-extension-witness-20261030/README.md) | [Fresh replay report](jpeg2000-u-extension-witness-20261030/verification/report.json) / [archive](jpeg2000-u-extension-witness-20261030.tar.gz) |

The first four packages contain **33 curated review rows**:23 standards-behavior
observations, seven structural-validation gaps, two project-behavior mismatches
and one SHOULD recommendation. **These are not33 universal MUST violations.**
Rows, native commands, registered clauses and independent root causes are
different counts. The additional F11 wrapper does not add a row/root;
U1 is not promoted into the confirmed Core family count.

| Transform discrepancy | Document citation | Deviating code | Reproduction |
|---|---|---|---|
| Odd-origin arbitrary lifting selects the wrong neighbour. | T.801 (06/2021), H.2.5.1 equation H-3 and H.2.4.3 | [`lifting_source_sum`, transform.rs:846-905](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/transform.rs#L846-L905) | Production-decoded ATK kernel; expected `[118,208,168]`, observed `[218,308,268]`; even-origin control passes. |
| Odd-origin irreversible singleton is not halved. | T.800 (06/2019), F.3.6, printed p.120 | [`inverse_9_7`, transform.rs:461-469](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/transform.rs#L461-L469) | Q16 sample131072 should become65536; it remains131072. |

From the original, supplement or final-JPIP package directory:

```sh
python3 reproduce.py --repo /path/to/jpeg2000_rs --all --output ./reproduction-results
```

For the extracted transform archive:

```sh
cd jpeg2000-transform-witnesses-20261030
cargo +1.93.1 test --locked --offline --lib wave86_ -- --nocapture --test-threads=1
```

The transform command intentionally returns101: one control passes and two
standards assertions fail. Other package runners report success only when the
expected native failure and its specific diagnostic markers reproduce. See
each README for prerequisites and exact selection commands.

## Final execution results

**The supported-scope run and evidence packaging are complete, with the explicit
gaps below.** Initialization and bottom-up inference cover all 1,703
source-mapped default Cargo library subjects and 3,219 edges. This is not every
configuration or generated/unmapped body. Baseline execution passed 666
all-target tests and three doctests.

| Measure | Final result |
|---|---|
| Retained clauses | 4,947: 3,537 inferred and 1,410 added through standards mapping |
| Current completed generated suites | 2,782; 2,819 suite attempts |
| Registered native cases | **4,947: 4,855 pass / 92 fail** |
| Uniquely exercised retained clauses | **4,945**; two clauses have two cases each |
| Historical registered executions, including retests | 5,006; portable-package replays are separate |
| Pending/active pipeline items | Zero |

Two retained clauses explicitly lack observed verdicts:

- Provider-blocked `26a5fcebe11048293e786908`: exhausted after two attempts,
  without a result. No retry or substituted execution.
- Panic-unwind ownership clause `4fc54d6646487096c7134da6`: its accepted suite
  explicitly covers only the other two clauses. No ordinary valid-input unwind
  trigger was established; allocator abort is not unwind, and inserting a panic
  would test modified code. It is not counted as passed.

See [exact coverage](evaluation/clause-coverage.json)
and [final semantic/integrity audit](evaluation/final-evaluation-audit-20261030.json).
All original failure evidence remains retained, including standards witnesses
whose narrower inferred clauses later received test-error or partial-review
attributions.

**All mapping and final clause testing are reconciled.** One JPIP hypothesis was
corrected to require a successful first call; this is an inference-specification
error, not a newly discovered production defect. The other3536 original
inferred clause bodies and origins are unchanged;16 have updated review metadata.
All221 initial mapping results are preserved. The accepted corrected batch gives
222 historical batches and221 batches covering the current inferred snapshot,
each with all five full-text documents together, not just MUST/SHOULD extracts.
The current inferred snapshot matches the database exactly; no additional
mapping is needed. The final six clauses produced four passes and two failures.
Both failures received bounded source/standards review and fresh portable
reproductions: one new VBAS finding, F31, and additional evidence for F11.

The 104 historical accepted diagnoses comprise64 reproduced,25 candidate,
two specification errors,10 test errors and three partial reviews. Reproduced
diagnoses are grouped into38 Core review families, including five hypothesis-only
families; this is **not** the curated package-row count or38 certified defects.
The final audit found no change to the 4,947 projected specs, 2,342 model/Verus
mentions or270 heuristic flags after the final mapping snapshot. Review flags
remain triage, not independent verification.

## Interpretation limits

Passing native tests against a Verus-derived/project model do not establish
independent standards conformance or completed Verus proofs. In particular,
the passing unchanged-singleton Q16 model does not erase the second transform
witness. Some transitive test oracles remain manually unreviewed; the92 failing
cases are not92 confirmed defects.

The acquired T.800(2019), T.801(2021), T.808(2022), T.814(2019) and T.803(2024)
editions were mapped together. The project selects newer editions of the first
three; those differences remain unresolved. Official conformance test signals
were not acquired. Bounded unit/API witnesses are not necessarily complete
encoded-image reproductions. Packages contain citations and hashes, not the
copyrighted standards documents.

## Archive integrity

All five archives were rechecked; previous packages remain unchanged.
The U-extension and final-JPIP packages were freshly replayed and their15 and36
archived files verified byte-for-byte. The final-JPIP package preserves the
initial strict-marker mismatch and its correction; all exact error/input/exit
checks remain, and all seven runner checks pass.

| Archive | SHA-256 |
|---|---|
| Original27 | `82699bdcf73436ce711b61dc7d309c9e880d370a280c2bdf73a01824edc3f3ac` |
| Three-row supplement | `b6553db1e0262265c38659e1900a561ae5f33e7d9dc7bb442bdcb68d3cbd5294` |
| Two transforms | `564e795646daac9dda2a52fb914e1d3caff18b1e25a34ed8280b98cf18e942f0` |
| Qualified U extension | `47f75362318c29461d57ff039293211b06c8bf8b4487c853d279d2507ec67565` |
| Final JPIP | `5a3c1ad255a4ab9915ba6325b6cf7a95c8f56d82782e67100522ca44eb40a80a` |
