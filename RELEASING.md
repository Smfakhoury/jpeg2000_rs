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
Keep the repository private until the actual retained content and any proposed
operations have the required owner approval.

### Unsafe-code review

Confirm the library crate-level prohibition and both the main and standalone
fuzz package's `unsafe_code = "forbid"` lints remain enabled. Review all owned
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
| Packaged decoder | Exact package-file inventory and `cargo package --locked`, including its default verification build |
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
