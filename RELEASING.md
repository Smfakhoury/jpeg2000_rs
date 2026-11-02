# Publishing and releasing

Publishing source, claiming standards conformance, and publishing a versioned
crate are different decisions. `publish = false` remains in force until a
package release is explicitly authorized.

The intended registry package is `jpeg2000-decoder`, imported in Rust as
`jpeg2000_decoder`. Repository URLs remain `microsoft/jpeg2000_rs`.
Check registry name availability again for the actual release; an earlier
not-found response does not reserve a name or authorize publication.

## 0.1 release branch and repository controls

Cut `release/0.1` from accepted `main`, not a local proof-development branch.
Keep the development toolchain pin and the experimental, non-certified claims
unchanged unless a separately reviewed change qualifies them.

Protect `main` with required PRs, an independent approval of the latest changes,
resolved review conversations, and the complete CI, Verus, CLA and model-review
checks. Do not add a bypass actor to make a pending change mergeable.
Keep a newly cut release branch frozen during bootstrap. Transition it only
through a reviewed exact-source operation after the release-branch CI, Verus
and trusted target-branch model hold have been integrated. Then require those
same checks and independent reviews for release PRs. Default-branch adoption
does not automatically update an existing release branch.

Protect version tags against updates and deletion. An authorized `v0.1.x` tag
must identify the reviewed release commit on `release/0.1`; a mutable branch
name is not the final release identity. Dependency-update PRs must pass the
same review controls and do not independently authorize verification-pin
changes.

### No-upload publication preflight

After an authorized release change enables `publish = ["crates-io"]`, dispatch
the [release preflight](.github/workflows/release.yml) from `release/0.1`.
It rejects other refs, checks the package identity and inventory, runs
`cargo publish --dry-run --locked`, and retains the actual archive, checksum,
source commit and command log. It has no upload step or registry credential.

This packaging preflight does not replace the candidate checks, separate
packaged-source Verus replay, numerical/resource gates or instrumented fuzz
campaigns below. Bind every required result to the same final candidate and
archive; an older successful dry-run is not a release attestation.

The [crates.io trusted-publishing prerequisites](https://crates.io/docs/trusted-publishing)
currently require an already published crate. The initial upload therefore
needs separately authorized API-token authentication. Configure trusted
publishing for subsequent releases using a pinned authentication action,
job-local OIDC permission and a protected environment. Never commit a token or
treat a dry-run, branch creation or successful workflow as upload authority.

## Source publication

- Confirm the approved source license and preserve required third-party notices.
- Review the exact source candidate, reachable Git history, repository metadata,
  and CI artifacts for restricted or internal-only material.
- Keep specifications, proofs, tests, reusable validation tooling, and relevant
  documentation in the source tree.
- Keep internal approval correspondence and generated run reports outside it.
- Keep maintained JSON manifests, schemas and the technical reference
  inventory under [validation/](validation/). They are test inputs, not
  disposable tracking files. Keep execution plans and task status in the
  issue/PR system rather than adding source-tree progress ledgers.
- Run the [contributor checks](CONTRIBUTING.md) from a clean clone.
- Describe the implemented profile and [proof boundaries](PROOFS.md) accurately.

Do not assume that removing a file from the current tree removes it from Git
history. An exported source snapshot and a visibility change on an existing
repository have different disclosure boundaries.

### Existing repository history and metadata

The owner must explicitly choose whether to retain reviewed history or approve
an exact cleanup plan before changing visibility. Closing pull requests,
deleting branches, or rewriting branch history does not by itself remove
GitHub-maintained PR refs, cached views, conversations, Actions logs, or artifacts.

GitHub marks PR refs as read-only. Its
[documented history-removal process](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository)
also states that Support does not remove non-sensitive data through that
process. Do not promise complete removal of ordinary development records on
that basis. Check the applicable platform process and remaining disclosure
boundary before proposing any destructive operation.

Source-publication approval does not by itself choose a history strategy or
authorize ref deletion, force-pushing, or repository replacement/renaming.
Obtain owner approval for the actual retained content and proposed operations
before any future visibility or history change.

### Unsafe-code review

Confirm the library crate-level prohibition and the main, standalone fuzz and
image consumer packages' `unsafe_code = "forbid"` lints remain enabled. Review all owned
Rust targets, not only the default library configuration, and run their
compiler/Clippy checks. The current owned sources have no unsafe code to
replace; do not describe dependency or standard-library internals as audited
by that fact. A future change in this boundary requires a new review.

### Candidate checks

Use the same committed candidate for the [CI workflow](.github/workflows/ci.yml),
the [proof workflow](.github/workflows/verus.yml), and retained validation
artifacts. Follow the [contributor setup](CONTRIBUTING.md) for tool prerequisites.

| Gate | Required check |
|---|---|
| Rust behavior | All-target tests, doctests and release checks on supported platforms |
| Rust diagnostics | Formatting, strict Clippy and rustdoc |
| Packaged decoder | Exact package-file inventory, `cargo package --locked` with its verification build, and pinned formatting of the actual unpacked archive |
| External consumer | Standalone `image` adapter tests/doctest and release check on Rust 1.88, pinned formatting, strict Clippy and rustdoc against the unpacked package |
| General decoder validation | Python regression suite and `python validation/run-validation.py all` |
| Independent HT validation | Separate `python validation/run-validation.py htj2k` invocation |
| Representative process bounds | Separate Linux `python validation/run-validation.py resources` invocation |
| Expressed proof obligations | Exact production-root command and strict coverage checker from [PROOFS.md](PROOFS.md) |

The `all` command does not implicitly run the HT or resource gates. Passing a
package-file comparison also does not show that the packaged source compiles;
keep the package verification build enabled. `cargo package` is a local build
operation, not package-publication authorization.

Perform these checks in a clean checkout, regenerate the corpus without
changing tracked source, and retain source/tool/input/output hash bindings.
Keep skipped comparisons and unavailable external conformance inputs visible
rather than presenting them as passed checks.

### External `image` consumer exercise

The repository-only [consumer probe](validation/image-consumer-probe/src/lib.rs)
uses published `image` 0.25.10 with its built-in codecs disabled. Its dependency
points to the actual `cargo package` output, not the repository decoder.
The [CI quality job](.github/workflows/ci.yml) builds that package before running
the probe. Neither the probe nor `image` is added to the decoder's shipped
package or default dependency graph.

The probe implements `ImageDecoder` plus opt-in `ImageReader` hooks for a bounded
opaque grayscale/sRGB JP2 profile. Pixel decoding is deferred until the trait's
read method; strict `image` dimensions are checked first, and the original codec
resource limits remain in force. It is not streaming, comprehensive metadata or
profile support, a global memory bound, an upstream `image` integration, or part
of the decoder's Verus proof claim. Synthetic fixtures exercise API behavior,
not official conformance.

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
- Verify Cargo metadata reports package `jpeg2000-decoder` and library target
  `jpeg2000_decoder`; exercise the actual `jpeg2000-decoder-<version>.crate`
  archive and its unpacked directory, not an old-name package left in `target`.
- Run the pinned formatter on the unpacked package as well as the repository,
  so conditional module paths do not point to omitted source files. Build and
  run the unpacked unit tests and doctests at the runtime minimum.
- Require the archive's separate whole-crate Verus replay and fail-closed
  collection, with unchanged input bindings; do not infer it from repository
  verification or source-file inclusion.
- Verify that ghost code is erased in ordinary Rust builds.
- Review all required validation and proof results against the same candidate.
- Set a version and enable package publication only when authorized.
- Verify the published source/package and its reproducible build commands.
