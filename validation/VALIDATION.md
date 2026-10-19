# Decoder validation

This directory implements Phase H as **decoder** validation hardening.
Encoding is not part of the approved first-release profile. The framework does
not claim ISO/IEC 15444-4 conformance and contains no official, restricted, or
standards-derived vectors.

## Reproducible generated corpus

`framework.py` deterministically generates minimal raw codestream, JP2, JPH,
JPX, and JPIP cases plus truncation-prefix, negative, and metamorphic variants.
`corpus.json` records every byte length and SHA-256, its generation recipe,
redistribution status, expected result, and any metamorphic relationship.
`corpus.schema.json` rejects non-generated classifications. `verify` checks
hashes, duplicate identifiers, provenance policy, path containment,
metamorphic parents, and unmanifested corpus files.

```sh
python3 validation/run-validation.py generate --check
python3 validation/run-validation.py verify
```

Regenerate intentionally with `generate` without `--check`. Generation uses
only Python's standard library and is deterministic.

## Differential execution and normalization

The in-tree validation adapter is an example target, deliberately excluded
from the published package. It emits a stable image/error protocol. Successful
images are normalized by dimensions, component precision/sign, and integer
sample values before hashing. Error normalization retains only the typed error
category. Metamorphic cases require the same normalized output hash.

`oracles.json` defines optional command templates for OpenJPEG, Grok, and
JasPer. The framework neither downloads nor bundles those programs. It uses
`OPENJPEG_DECODER`, `GROK_DECODER`, or `JASPER_DECODER` when set, otherwise
searches `PATH`. Missing programs are recorded as `skipped` with a reason;
their absence does not fabricate a pass or a disagreement. External PGM/PPM
outputs are normalized to the same planar integer representation.

```sh
python3 validation/run-validation.py run
python3 validation/run-validation.py run --no-external
```

Per-case observations are written to
`validation/results/validation-results.json`; all acceptance or normalized
output differences are preserved in `validation/results/disagreements.json`
as unadjudicated records. Stable `oracle-disagreement-v1` identifiers are
derived only from the case, oracle, disagreement kind, and both normalized
hashes. Human decisions are stored separately in
`validation/results/oracle-adjudications.json` and survive reruns with the
same observation. An unresolved record blocks the oracle-adjudication gate;
the framework never resolves one automatically. These generated result files
are evidence, not official conformance results.

## Resource budgets and attestation

`budget` applies a five-second per-process timeout, a 30-second corpus budget,
one-MiB input/output caps, and a 512-MiB address-space limit where POSIX
`RLIMIT_AS` is available. The result explicitly reports whether the OS memory
limit was enforced; production decoder `ResourceLimits` remain active on every
platform.

```sh
python3 validation/run-validation.py budget
python3 validation/run-validation.py attest
python3 validation/run-validation.py all
```

`source-tree.json` is a non-VCS, deterministic `sha256-tree-v1` manifest. It
content-addresses production, proof, validation, schema, workflow, fuzz, and
release-documentation source while explicitly excluding generated evidence
and build output. A separate production-source revision plus the per-file
hashes in `production-proof-coverage.json` permits reuse of existing proof
evidence only when every proof input still matches.

The release attestation binds hashes for source, exact toolchain declarations,
proof attestation/coverage, corpus, differential validation, adjudications,
budgets, fuzz evidence, package inventory, architecture review, and approval
placeholders. Schema rules make a `passed` release impossible without
official vectors, zero unresolved oracle disagreements, current architecture
review, legal/publication approval, and release-owner approval.

`normative-coverage.json` records selected-profile functional refinement for
Parts 1, 2, 9, and 15: 54 requirement groups linked to 160 concrete contracted
functions with zero unlinked selected groups. Its validator resolves implementation,
normative spec, executable contract, proof/view, and test symbols, enforces the
complete selected Part 9 and Part 15 group inventories and source-accessibility
classifications, and rejects any closure claim with an unlinked group. A
`functionally_linked` claim is rejected when its contract does not directly
ensure the referenced spec or is not called by the claimed production
implementation.

```sh
python3 validation/run-validation.py source
python3 validation/run-validation.py source --check
python3 validation/run-validation.py architecture --check
python3 validation/run-validation.py schemas
```

The committed Phase I result remains blocked and does not claim official
conformance when any external approval or official-vector input is absent.

## External official vectors

Official Part 4 assets are never copied into or bundled with this repository.
The `official` command accepts an authorized, externally managed vector root
through `--vector-root` or `JPEG2000_OFFICIAL_VECTOR_ROOT`; an external
manifest may be selected with `--manifest` or
`JPEG2000_OFFICIAL_VECTOR_MANIFEST`. The default manifest name inside the root
is `official-vector-manifest.json`.

The manifest must validate against
`official-vector-manifest.schema.json`, bind the original archive and every
applicable input by SHA-256 and byte length, attest approved internal use with
redistribution disabled, and state that selected-profile decoder-vector
selection is complete. Accepted cases also require an independently established
normalized reference-output hash. Paths are confined to the external root.

```sh
JPEG2000_OFFICIAL_VECTOR_ROOT=/authorized/ephemeral/root \
  python3 validation/run-validation.py official
```

Execution applies per-case and total time, input, output, and address-space
limits. `validation/results/official-vector-results.json` contains only source
and acquisition metadata, archive/manifest/file hashes, public identifiers
when explicitly marked public, counts, pass/fail status, normalized output
hashes, image geometry/type metrics, and exact source/tool bindings. It never
records asset paths or decoded samples. The release attestation marks the
official-vector gate passed only when selection is complete and every
applicable vector is verified, executed, and passed.

Run script tests with:

```sh
python3 -m unittest discover -s validation/tests -v
```

Install the pinned schema validator first when it is not already available:

```sh
python3 -m pip install -r validation/requirements.txt
```
