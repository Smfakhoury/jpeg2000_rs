# Independent whole-package replay

The top-level `verify.py --all` entry point was executed on November 2, 2026,
using Rust 1.93.1 and the pinned upstream revision
`69fe0bce9024ab5a4cb6556e3ebdf91134324148`. `PYTHONPATH` was unset; no DeepSpec,
SOMA or specification database was used by the reproduction runner.

All five components report **reproduced** in [report.json](report.json).
Each component's detailed report and complete build/native logs are preserved
in this directory. The run lasted from 19:08:58 to 19:16:21 UTC.

| Component | Selected commands / observations | Detailed evidence |
|---|---|---|
| Original F01-F27 | 27 selected cases | [Report](jpeg2000-review-package-20261029/report.json) |
| Supplement F28-F30 | 3 selected cases | [Report](jpeg2000-review-supplement-20261030/report.json) |
| Final JPIP | 2 selected cases: F31 and an additional F11 wrapper | [Report](jpeg2000-final-jpip-witnesses-20261030/report.json) |
| Qualified U1 | 1 selected command, with three passing controls and one expected failing test | [Report](jpeg2000-u-extension-witness-20261030/report.json) |
| T1/T2 transforms | 1 command, with one passing control and two expected failing tests | [Complete native log](transforms.log) |

These are **33 curated review rows plus qualified U1**, not a one-to-one
correspondence between rows, commands, native Rust tests and root causes.
The aggregate runner checks exact selected-case coverage, expected failure
status and specific diagnostics. A missing test or unrelated failure does
not count as reproduction.

Verified entry-point SHA-256:
`3bf647381bbd4357e3b35f59690d34aed67d44a25e27b5c23bb0f16cf37dcae6`.

The eight `test_verify.py` checks cover inventory completeness, modified
archives/test-source rejection, recorded-report validation, missing/duplicate/
unexpected cases, unexpected success/timeouts/build failures, shared-case
counting and transform selection. Historical reports without the required
toolchain metadata are not accepted as fresh replay receipts.

Reproducing the assertions does not resolve newer-standard edition differences
or establish full encoded-image conformance. All scope qualifications in the
main review table remain applicable.
