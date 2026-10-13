# Security policy

## Supported versions

There is no supported release. The project is approval-gated and codec
implementation has not started.

## Reporting a vulnerability

Do not publish suspected vulnerabilities or malicious test files in a public
issue. Report them through the repository owner's approved private security
channel. A public reporting address must be added before the first release.

Include:

- the affected immutable revision;
- the selected profile and resource-limit configuration;
- a minimal reproducer or its secure location;
- observed versus expected behavior;
- platform and toolchain versions;
- whether the issue causes a panic, excessive work or allocation, incorrect
  output, out-of-bounds behavior in a native oracle, or nondeterminism.

Restricted, purchased, or standards-derived vectors must remain in approved
storage and must not be attached to public reports.

## Security requirements

- Production Rust must forbid unsafe code unless an exception is individually
  reviewed, documented, and approved.
- Every input-derived length, offset, count, dimension, allocation, and work
  calculation must use checked arithmetic.
- Malformed input must not panic, loop without progress, allocate beyond the
  active limits, or access outside an owned buffer.
- Errors must distinguish malformed input, unsupported profile or feature,
  exceeded limits, and I/O failure.
- Parsers and decoders must be deterministic for identical input and options.
- Native differential oracles must run out of process with independent memory,
  output, and execution limits.

The detailed threat inventory and required controls are in
[`THREAT_MODEL.md`](THREAT_MODEL.md).
