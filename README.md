# jpeg2000_rs

A resource-bounded JPEG 2000 decoder written in Rust, with Verus specifications
and proofs attached to the production implementation.

The library decodes raw codestreams and JP2/JPH containers, provides explicit
JPX codestream and frame selection, and supports bounded JPIP databin
reconstruction. It returns typed sample planes without discarding component
precision, signedness, origins, or subsampling.

This is experimental software, not a certified JPEG 2000 implementation.
Official conformance testing and broader independent validation are required
before making a standards-conformance claim.

## Build

Install [rustup](https://rustup.rs/), then run:

```sh
cargo build --locked
cargo test --all-targets --locked
```

Ordinary decoder builds support Rust 1.88.0 and newer. CI checks this minimum
with `cargo +1.88.0`; the development toolchain remains pinned separately in
[rust-toolchain.toml](rust-toolchain.toml).
The default decoder build has no external Cargo dependencies and forbids unsafe
code. The optional `verus` feature supplies pinned verification tooling only;
it is outside the runtime minimum-version guarantee and uses its own toolchain.
See the [Cargo-Verus instructions](PROOFS.md#reproduce-verification).
Package publication is disabled.

## Decode a JP2 image

```no_run
use jpeg2000_rs::{DecodeOptions, TypedSamples, decode_jp2};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let input = std::fs::read("image.jp2")?;
let image = decode_jp2(&input, &DecodeOptions::default())?;

for component in image.components() {
    let info = component.info();
    match component.samples() {
        TypedSamples::Signed(samples) => {
            println!("{}-bit signed component: {} samples", info.precision, samples.len());
        }
        TypedSamples::Unsigned(samples) => {
            println!("{}-bit unsigned component: {} samples", info.precision, samples.len());
        }
    }
}
# Ok(())
# }
```

Use `decode_codestream` for a raw codestream, `decode_jph` for JPH, and
`decode_jpx_codestream` with an explicit index for JPX. Color-conversion and
JPX layer/frame APIs are separate from component sample decoding; see the
[public exports](src/lib.rs).

## Select a region or native resolution

```no_run
use jpeg2000_rs::{
    DecodeOptions, DecodeSelection, ImageRegion, decode_codestream_selected,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let input = std::fs::read("image.j2k")?;
let selection = DecodeSelection::full_image()
    .with_region(ImageRegion::new(0, 0, 256, 256))
    .with_discard_levels(1);
let image = decode_codestream_selected(&input, &DecodeOptions::default(), &selection)?;
# let _ = image;
# Ok(())
# }
```

Regions use absolute, half-open SIZ image-reference-grid coordinates. Discarding
resolution levels selects native wavelet resolution, not a resize of a fully
decoded image. A selection must fit the input's actual origin and geometry.

## Resource limits and errors

```no_run
use jpeg2000_rs::{DecodeOptions, ResourceLimits, decode_codestream};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let limits = ResourceLimits::default()
    .with_maximum_input_bytes(64 * 1024 * 1024)?
    .with_maximum_work_units(10_000_000)?;
let options = DecodeOptions::new(limits);
let input = std::fs::read("image.j2k")?;
let image = decode_codestream(&input, &options)?;
# let _ = image;
# Ok(())
# }
```

Limits cover input, output, dimensions, components, tiles, packets, metadata,
code-blocks, frames, and work. Callers can tighten the hardened defaults.
Inputs exceeding those bounds return typed `DecodeError` values; a valid
standard feature may still exceed the configured resource profile.

Errors distinguish malformed input, unsupported features or profiles, invalid
selections, resource exhaustion, and I/O. Applications should not treat a
successful metadata inspection as successful pixel decoding.

## Scope

- Decoder features from JPEG 2000 Parts 1, 2, 9, and 15.
- Legacy MQ/Tier-1 and high-throughput HTJ2K block decoding.
- Reversible integer and deterministic fixed-point irreversible transforms.
- JP2 palette and channel mapping; pinned sRGB, grayscale, and sYCC conversion.
- JPX explicit codestream/layer/frame selection and bounded composition.
- JPIP message parsing, transactional databin caching, and reconstruction;
  network transport and request scheduling are not provided.

Encoding, an ICC color-management engine, automatic handling of arbitrary
color-space registry additions, and internally parallel decoding are not
provided. See [supported profiles and limitations](SUPPORTED.md) for details.

## Proofs and testing

### Repository layout

- [src/](src/) is the single executable decoder used by Cargo and Verus.
- [specs/](specs/) and [proofs/](proofs/) contain maintained semantic models
  and proofs, not generated run results.
- [validation/](validation/) contains the reusable harness, regression tests,
  pinned oracle configuration, corpus manifest and JSON schemas.
  Its [technical reference inventory](validation/normative-coverage.json)
  links implementation, model, contract and test symbols; the accompanying
  [schema](validation/normative-coverage.schema.json) validates those links.
  These are required validation inputs, not a progress ledger or a
  standards-conformance certificate.
- Generated reports, native reference builds and fuzz artifacts belong under
  ignored `target/`, not in the repository root or versioned source.
  Personal execution plans, approval correspondence and session logs do not
  belong in the published tree.

- [Proof architecture and reproduction](PROOFS.md)
- [Decoder validation and external conformance inputs](validation/VALIDATION.md)
- [Contribution and local checks](CONTRIBUTING.md)
- [Standards and implementation references](REFERENCES.md)
- [Third-party notices](./THIRD_PARTY_NOTICES)

Rust builds and Verus verification share [src/](src/) as the executable source
of truth. [specs/](specs/) and [proofs/](proofs/) contain proof-only models and
lemmas. Verification is not a substitute for official test vectors, numerical
conformance, or independent review of the specifications.

The crate package deliberately contains the runtime source and its public
documentation, not the repository's verification and validation toolchain.
Reproducing proofs or reference-decoder comparisons requires a source checkout.

## Security

Follow [Microsoft's security reporting policy](SECURITY.md), not public issue
reports, for suspected vulnerabilities or sensitive reproductions.
The [threat model](THREAT_MODEL.md) describes the input and tooling boundaries.

## License

First-party source is licensed under the [MIT license](./LICENSE).
Derived HT VLC lookup tables retain the upstream BSD-2-Clause notices in
[THIRD_PARTY_NOTICES](./THIRD_PARTY_NOTICES).
