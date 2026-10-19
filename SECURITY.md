<!-- BEGIN MICROSOFT SECURITY.MD V1.0.0 BLOCK -->

## Security

Microsoft takes the security of our software products and services seriously, which
includes all source code repositories in our GitHub organizations.

**Please do not report security vulnerabilities through public GitHub issues.**

For security reporting information, locations, contact information, and policies,
please review the latest guidance for Microsoft repositories at
[https://aka.ms/SECURITY.md](https://aka.ms/SECURITY.md).

<!-- END MICROSOFT SECURITY.MD BLOCK -->

## Supported versions

There is no stable supported release yet. The decoder is experimental.
Report suspected issues against the current source revision and include the
resource limits used.

## Reporting a vulnerability

Do not publish suspected vulnerabilities or malicious test files in a public
issue. Use the Microsoft reporting guidance linked above.

Include:

- the affected immutable revision;
- the selected profile and resource-limit configuration;
- a minimal reproducer or its secure location;
- observed versus expected behavior;
- platform and toolchain versions;
- whether the issue causes a panic, excessive work or allocation, incorrect
  output, out-of-bounds behavior in a native oracle, or nondeterminism.

Restricted or purchased vectors must remain in authorized storage and must not
be attached to public reports without redistribution permission.

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
