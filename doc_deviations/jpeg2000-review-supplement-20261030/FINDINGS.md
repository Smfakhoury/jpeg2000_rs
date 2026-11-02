# JPEG 2000: reproduced findings for human review

Source: [`microsoft/jpeg2000_rs@69fe0bce9024`](https://github.com/microsoft/jpeg2000_rs/tree/69fe0bce9024ab5a4cb6556e3ebdf91134324148).

**3 review families**, grouped below. Every row links to native failing-test evidence. These are bounded reproductions, not Verus proofs or certification. Candidate diagnoses and unsupported inferred-intent-only families are excluded.

Use `python3 reproduce.py --repo /path/to/jpeg2000_rs --finding F28` for one row, or `--all` for the bundle. A successful reproduction means the original bug was observed; the underlying Rust regression test is expected to **fail**.

## Standards-grounded behavior

| ID | What goes wrong | Document citation | Deviating code | Backing test |
|---|---|---|---|---|
| F29 | **Allowing SOP makes every packet require an SOP marker**. With COD allowing SOP, a complete one-byte empty packet without its optional marker is rejected as truncated. | [T.800 (2019-06), A.8.1](https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.800-201906-S!!PDF-E&type=items) | [`src/packet.rs:3989-4009`](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/packet.rs#L3989-L4009) | [failure 1](cases/f71c07d698d8441f7f243d58/44c55ec8f76ec6b24e477888.log) / [recipe](cases/f71c07d698d8441f7f243d58/recipe.json) |
| F30 | **A terminal stuffed header byte is mistaken for the EPH marker**. The header C0 BE FF 00 is followed by the correct FF92 marker, but inspection skips the terminal stuffing byte in its boundary count and rejects the marker. | [T.800 (2019-06), B.10.1 and A.8.2](https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.800-201906-S!!PDF-E&type=items) | [`src/packet.rs:4173-4194`](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/packet.rs#L4173-L4194) | [failure 1](cases/f68e289bc92b358b513e6f20/23cf463d2324619732b2286f.log) / [recipe](cases/f68e289bc92b358b513e6f20/recipe.json) |

## Standards recommendations

These observations differ from a cited SHOULD recommendation, not an unconditional SHALL requirement.

| ID | What goes wrong | Document citation | Deviating code | Backing test |
|---|---|---|---|---|
| F28 | **A zero-life nonpersistent instruction is painted instead of skipped**. A normalized frame includes an opaque gray sample where the no-action recommendation calls for transparency. | [T.801 (2021-06), M.5.3.2.1](https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.801-202106-S!!PDF-E&type=items) | [`src/jpx.rs:818-875`](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/jpx.rs#L818-L875) | [failure 1](cases/d18e7664ef908ecf323e5b74/e4ed0a3af2c9d0dbd08711cc.log) / [recipe](cases/d18e7664ef908ecf323e5b74/recipe.json) |

## Scope notes

- **F28:** SHOULD recommendation, not an unconditional SHALL violation. Small normalized timeline/rendering fixture with supplied decoded layers; no public encoded-file route or animation-bit admission claim. The persistent() getter itself correctly returns its stored flag. This overlaps the earlier independent no-action probe and is not an additional root.
- **F29:** Parsed coding-style context and production progression preparation lead to a direct packet unit. The failing control has SOP allowed, no EPH, no packed headers, and one empty packet. No complete public image decode is established. Host reproduction is supported by the normative mapping parent; agreement with the narrower internal routing model is not a refutation of that model.
- **F30:** Direct inspection unit with one constructor-backed 1x1 decoder, 255 declared opaque body bytes and EPH enabled; Tier-1 body decoding is deliberately disabled. Root boundary operation is also src/packet.rs:164-166. Normative mapping-parent authority supports the observed rejection, not completed verification of the internal model. The separate no-EPH length assertion has narrower-clause test_error attribution and is not counted as another reproduced case.

## Editions and interpretation

The acquired editions are T.800 (2019-06), T.801 (2021-06), T.808 (2022-12), T.814 (2019-06), and T.803 (2024-02). The project selects newer T.800/T.801/T.808 editions; those differences remain unreconciled. T.803 was part of every original mapping batch but is not an independent authority for a row in this table.

Do not equate rows, tests, clauses, or diagnoses with a proven number of independent root causes.

Native/model agreement is not independent standards evidence. Recorded logs retain the original bounded output tails; `reproduce.py` saves complete output for new runs.
