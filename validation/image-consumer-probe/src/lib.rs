#![forbid(unsafe_code)]
//! Repository-only consumer exercise against the actual unpacked decoder crate,
//! using `image = 0.25.10` without its default codec features.
//!
//! Build the decoder archive first with `cargo package --locked` at the
//! repository root, then run `cargo test --locked --manifest-path
//! validation/image-consumer-probe/Cargo.toml`. This standalone workspace and
//! its dependencies are not included in the decoder's package or default graph.
//! See [`Jp2Decoder`] for the intentionally limited profile and memory scope.

mod decoder;

pub use decoder::{register_jp2_hook, HookRegistrationError, Jp2Decoder};

use image::DynamicImage;
use jpeg2000_rs::{
    decode_jp2_color, ConvertedChannelType, ConvertedColorSpace, DecodeError, DecodeOptions,
};
use std::{collections::TryReserveError, error::Error, fmt};

#[derive(Debug)]
pub enum ProbeError {
    Decoder(DecodeError),
    UnsupportedLayout(&'static str),
    InvalidLayout(&'static str),
    BridgeBufferLimit { required: u64, maximum: u64 },
    Allocation(TryReserveError),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decoder(error) => write!(formatter, "JPEG 2000 decoding failed: {error}"),
            Self::UnsupportedLayout(reason) => {
                write!(formatter, "unsupported probe layout: {reason}")
            }
            Self::InvalidLayout(reason) => write!(formatter, "invalid converted layout: {reason}"),
            Self::BridgeBufferLimit { required, maximum } => {
                write!(
                    formatter,
                    "bridge buffer needs {required} bytes, limit is {maximum}"
                )
            }
            Self::Allocation(error) => write!(formatter, "bridge allocation failed: {error}"),
        }
    }
}

impl Error for ProbeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decoder(error) => Some(error),
            Self::Allocation(error) => Some(error),
            _ => None,
        }
    }
}

/// Pixel bridge used by the local [`Jp2Decoder`] adapter.
///
/// Supports opaque normalized 16-bit grayscale/RGB output. Decoder options
/// bound the codec; `maximum_bridge_bytes` bounds only the additional copied
/// logical pixel-buffer bytes, not allocator overhead, the decoder's working
/// memory or total application memory.
pub fn decode_jp2_image(
    input: &[u8],
    options: &DecodeOptions,
    maximum_bridge_bytes: u64,
) -> Result<DynamicImage, ProbeError> {
    let converted = decode_jp2_color(input, options).map_err(ProbeError::Decoder)?;
    let channels = converted.channels();
    if !matches!(channels.len(), 1 | 3) {
        return Err(ProbeError::UnsupportedLayout("alpha or extra channels"));
    }
    let mut planes = Vec::new();
    planes
        .try_reserve_exact(channels.len())
        .map_err(ProbeError::Allocation)?;
    for channel in channels {
        planes.push((channel.channel_type(), channel.samples()));
    }
    pack_planes(
        converted.width(),
        converted.height(),
        converted.color_space(),
        converted.precision(),
        &planes,
        maximum_bridge_bytes,
    )
}

fn plane<'a>(
    channels: &[(ConvertedChannelType, &'a [u16])],
    kind: ConvertedChannelType,
    pixels: usize,
) -> Result<&'a [u16], ProbeError> {
    let mut found = None;
    for &(channel_type, samples) in channels {
        if channel_type == kind && found.replace(samples).is_some() {
            return Err(ProbeError::InvalidLayout("duplicate color channel"));
        }
    }
    let samples = found.ok_or(ProbeError::InvalidLayout("missing color channel"))?;
    if samples.len() != pixels {
        return Err(ProbeError::InvalidLayout(
            "channel length differs from dimensions",
        ));
    }
    Ok(samples)
}

fn pack_planes(
    width: u32,
    height: u32,
    color: ConvertedColorSpace,
    precision: u8,
    channels: &[(ConvertedChannelType, &[u16])],
    maximum_bridge_bytes: u64,
) -> Result<DynamicImage, ProbeError> {
    if width == 0 || height == 0 {
        return Err(ProbeError::InvalidLayout("zero dimensions"));
    }
    if precision != 16 {
        return Err(ProbeError::UnsupportedLayout(
            "expected normalized 16-bit samples",
        ));
    }
    let count = match color {
        ConvertedColorSpace::Grayscale => 1,
        ConvertedColorSpace::Srgb => 3,
    };
    if channels.len() != count {
        return Err(ProbeError::UnsupportedLayout("alpha or extra channels"));
    }
    let pixels = u64::from(width) * u64::from(height);
    let samples = pixels
        .checked_mul(count as u64)
        .ok_or(ProbeError::InvalidLayout("sample count overflow"))?;
    let required = samples
        .checked_mul(2)
        .ok_or(ProbeError::InvalidLayout("byte count overflow"))?;
    if required > maximum_bridge_bytes {
        return Err(ProbeError::BridgeBufferLimit {
            required,
            maximum: maximum_bridge_bytes,
        });
    }
    let pixels = usize::try_from(pixels)
        .map_err(|_| ProbeError::InvalidLayout("dimensions exceed addressable memory"))?;
    let samples = usize::try_from(samples)
        .map_err(|_| ProbeError::InvalidLayout("sample count exceeds addressable memory"))?;
    let mut output = Vec::new();
    match color {
        ConvertedColorSpace::Grayscale => {
            let gray = plane(channels, ConvertedChannelType::Gray, pixels)?;
            output
                .try_reserve_exact(samples)
                .map_err(ProbeError::Allocation)?;
            output.extend_from_slice(gray);
            image::ImageBuffer::from_raw(width, height, output)
                .map(DynamicImage::ImageLuma16)
                .ok_or(ProbeError::InvalidLayout(
                    "image buffer rejected grayscale layout",
                ))
        }
        ConvertedColorSpace::Srgb => {
            let red = plane(channels, ConvertedChannelType::Red, pixels)?;
            let green = plane(channels, ConvertedChannelType::Green, pixels)?;
            let blue = plane(channels, ConvertedChannelType::Blue, pixels)?;
            output
                .try_reserve_exact(samples)
                .map_err(ProbeError::Allocation)?;
            for index in 0..pixels {
                output.extend_from_slice(&[red[index], green[index], blue[index]]);
            }
            image::ImageBuffer::from_raw(width, height, output)
                .map(DynamicImage::ImageRgb16)
                .ok_or(ProbeError::InvalidLayout(
                    "image buffer rejected RGB layout",
                ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jpeg2000_rs::{ChannelAssociation, Resource, ResourceLimits};

    const GRAY: &[u8] = include_bytes!("../tests/fixtures/gray.jp2");
    const RGB: &[u8] = include_bytes!("../tests/fixtures/rgb.jp2");

    #[test]
    fn packaged_decoder_produces_an_image_gray_buffer() {
        let image = decode_jp2_image(GRAY, &DecodeOptions::default(), 2).unwrap();
        assert_eq!((image.width(), image.height()), (1, 1));
        assert_eq!(image.color(), image::ColorType::L16);
        assert_eq!(image.as_luma16().unwrap().as_raw(), &[128 * 257]);
    }

    #[test]
    fn packaged_decoder_produces_an_image_rgb_buffer() {
        let image = decode_jp2_image(RGB, &DecodeOptions::default(), 6).unwrap();
        assert_eq!(image.color(), image::ColorType::Rgb16);
        assert_eq!(image.as_rgb16().unwrap().as_raw(), &[128 * 257; 3]);
    }

    #[test]
    fn malformed_input_keeps_the_decoder_error() {
        assert!(matches!(
            decode_jp2_image(b"not JP2", &DecodeOptions::default(), 100),
            Err(ProbeError::Decoder(DecodeError::MalformedInput { .. }))
        ));
    }

    #[test]
    fn decoder_input_limit_is_not_hidden_by_the_bridge() {
        let limits = ResourceLimits::default()
            .with_maximum_input_bytes(1)
            .unwrap();
        assert!(matches!(
            decode_jp2_image(GRAY, &DecodeOptions::new(limits), 100),
            Err(ProbeError::Decoder(DecodeError::LimitExceeded {
                resource: Resource::InputBytes,
                ..
            }))
        ));
    }

    #[test]
    fn bridge_buffer_limit_is_explicit() {
        assert!(matches!(
            decode_jp2_image(GRAY, &DecodeOptions::default(), 1),
            Err(ProbeError::BridgeBufferLimit {
                required: 2,
                maximum: 1
            })
        ));
    }

    #[test]
    fn rgb_interleaving_uses_named_channels_and_keeps_all_bits() {
        let channels = [
            (ConvertedChannelType::Blue, &[0, 900][..]),
            (ConvertedChannelType::Red, &[512, 65535][..]),
            (ConvertedChannelType::Green, &[257, 1][..]),
        ];
        let image = pack_planes(2, 1, ConvertedColorSpace::Srgb, 16, &channels, 12).unwrap();
        assert_eq!(
            image.as_rgb16().unwrap().as_raw(),
            &[512, 257, 0, 65535, 1, 900]
        );
    }

    #[test]
    fn short_channels_are_rejected() {
        let channels = [(ConvertedChannelType::Gray, &[1][..])];
        assert!(matches!(
            pack_planes(2, 1, ConvertedColorSpace::Grayscale, 16, &channels, 4),
            Err(ProbeError::InvalidLayout(_))
        ));
    }

    #[test]
    fn duplicate_color_channels_are_rejected() {
        let channels = [
            (ConvertedChannelType::Red, &[1][..]),
            (ConvertedChannelType::Red, &[2][..]),
            (ConvertedChannelType::Blue, &[3][..]),
        ];
        assert!(matches!(
            pack_planes(1, 1, ConvertedColorSpace::Srgb, 16, &channels, 6),
            Err(ProbeError::InvalidLayout(_))
        ));
    }

    #[test]
    fn alpha_is_not_silently_discarded() {
        let channels = [
            (ConvertedChannelType::Gray, &[1][..]),
            (
                ConvertedChannelType::Alpha {
                    premultiplied: false,
                    association: ChannelAssociation::WholeImage,
                },
                &[2][..],
            ),
        ];
        assert!(matches!(
            pack_planes(1, 1, ConvertedColorSpace::Grayscale, 16, &channels, 4),
            Err(ProbeError::UnsupportedLayout(_))
        ));
    }

    #[test]
    fn zero_dimensions_are_rejected() {
        assert!(matches!(
            pack_planes(0, 1, ConvertedColorSpace::Grayscale, 16, &[], 10),
            Err(ProbeError::InvalidLayout(_))
        ));
    }

    #[test]
    fn byte_count_overflow_is_rejected_before_allocation() {
        let channels = [
            (ConvertedChannelType::Red, &[][..]),
            (ConvertedChannelType::Green, &[][..]),
            (ConvertedChannelType::Blue, &[][..]),
        ];
        assert!(matches!(
            pack_planes(
                u32::MAX,
                u32::MAX,
                ConvertedColorSpace::Srgb,
                16,
                &channels,
                u64::MAX
            ),
            Err(ProbeError::InvalidLayout(_))
        ));
    }

    #[test]
    fn unsupported_precision_is_not_truncated() {
        let channels = [(ConvertedChannelType::Gray, &[1][..])];
        assert!(matches!(
            pack_planes(1, 1, ConvertedColorSpace::Grayscale, 8, &channels, 2),
            Err(ProbeError::UnsupportedLayout(_))
        ));
    }
}
