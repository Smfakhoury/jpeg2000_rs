# Publishing and releasing

Publishing source, claiming standards conformance, and publishing a versioned
crate are different decisions. `publish = false` remains in force until a
package release is explicitly authorized.

## Source publication

- Confirm the approved source license and preserve required third-party notices.
- Review the exact source candidate, reachable Git history, repository metadata,
  and CI artifacts for restricted or internal-only material.
- Keep specifications, proofs, tests, reusable validation tooling, and relevant
  documentation in the source tree.
- Keep internal approval correspondence and generated run reports outside it.
- Run the [contributor checks](CONTRIBUTING.md) from a clean clone.
- Describe the implemented profile and [proof boundaries](PROOFS.md) accurately.

Do not assume that removing a file from the current tree removes it from Git
history. An exported source snapshot and a visibility change on an existing
repository have different disclosure boundaries.

## Conformance evidence

- Reconcile the selected standards editions with the specification inventory.
- Execute all applicable authorized official vectors from external storage.
- Require bit-exact lossless output and measured, explicit lossy tolerances.
- Run independent decoder comparisons; preserve and adjudicate disagreements.
- Complete instrumented fuzz/resource campaigns for the claimed profile.
- Bind results to the source commit, input and oracle hashes, toolchain, solver,
  commands, and platform. Store results as release/CI artifacts.

Generated tests and successful Verus verification do not replace official
conformance testing. Missing inputs, skipped oracles, and untested platforms
must remain visible.

## Versioned package release

- Obtain owner authorization for a specific candidate and release scope.
- Confirm package contents, license metadata, examples, and API documentation.
- Verify that ghost code is erased in ordinary Rust builds.
- Review all required validation and proof results against the same candidate.
- Set a version and enable package publication only when authorized.
- Verify the published source/package and its reproducible build commands.
