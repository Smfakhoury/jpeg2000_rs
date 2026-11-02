# Qualified U-extension witness

This preserves an independently standards-grounded **unit-level** observation,
separate from the published confirmed-finding tables. It is not a new Core
`reproduced` family and does not establish public image-decoding applicability.

| ID | Human-readable observation | Document citation | Deviating code | Backing test |
|---|---|---|---|---|
| U1 | With prefix5, suffix29 and extension10, the residual is74; adding the helper's predictor-one base should return75. Native decoding stops at prefix+suffix+1 and returns35, leaving the extension unread. | [T.814 (06/2019)](https://www.itu.int/rec/T-REC-T.814-201906-I/en), 7.3.4; 7.3.6, Table3 and equation(3), printed pp.16-18 | [`decode_non_initial_u`, lines2484-2501](https://github.com/microsoft/jpeg2000_rs/blob/69fe0bce9024ab5a4cb6556e3ebdf91134324148/src/tier1.rs#L2484-L2501), through `prefix_value` | [Original failure](cases/f4c99d87482bdb3bb429c765/recorded-test.log): input `00 00 00 00 4d ae 88 00`, mode1, expected `[75,1]`, observed `[35,1]`. |

The complete original7190-byte harness and assertion are unchanged. Its first
1089 extension-free pairs and budget/cursor checks pass, before the extension
witness fails. The selected command also retains three passing native controls.

**Attribution matters:** the inferred clause promised only prefix/suffix decoding,
not full U-extension decoding. Its `test_error` diagnosis concerns that narrower
clause; it does not retract the separate normative observation. A later passing,
narrower retest must not erase this original evidence.

**Applicability remains qualified:** the constructor accepts the small borrowed
input views, but this is an isolated U-field fixture, not a complete signalled
codestream. Prior CxtVLC decoding and public high-magnitude image reachability
are not established. T.814 7.3.4 guarantees extension-free coding when magnitudes
cannot exceed2^36; this witness must not be applied indiscriminately to those
profiles. Ghost/model agreement is not standards conformance or a Verus proof.

## Reproduce

Requirements: Linux, Python3.12+, Git, Rust/Cargo1.93.1 through rustup, and normal
Cargo dependencies. No DeepSpec, SOMA, model service, specification database or
standards documents are needed.

```sh
python3 reproduce.py --list
python3 reproduce.py --repo /path/to/jpeg2000_rs --all --output ./u-witness-results
python3 -m unittest test_reproduce.py
```

The repository must contain revision
`69fe0bce9024ab5a4cb6556e3ebdf91134324148`. The runner extracts that revision into
a disposable copy and leaves the supplied checkout unchanged. Review its recipe
before executing; it is not a sandbox. Native exit101 and **all** exact diagnostic
markers are required for `reproduced`; build errors, timeouts, unrelated panics
and unexpected passes do not count.

The manifest records source revision, standard hashes and original artifact
hash. Standards documents are not redistributed. The upstream MIT license is
included. This standalone package neither modifies nor renumbers the existing
27-case bundle, three-case supplement, or two-transform witness package.

## Verification

The packaged recipe freshly reproduced the exact `[35,1]` versus `[75,1]`
failure using Rust1.93.1 and the pinned source revision. The original command
returned101 with three controls passing and one test failing. See
[verification/report.json](verification/report.json) and its complete build/test
logs. Six portable-runner checks also passed.

This host replay is additional evidence, not another Core registered case.
Verify package integrity with `sha256sum -c SHA256SUMS`.
