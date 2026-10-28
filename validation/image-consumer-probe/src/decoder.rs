use crate::{decode_jp2_image, ProbeError};
use image::{
    error::{
        DecodingError, ImageFormatHint, LimitError, LimitErrorKind, UnsupportedError,
        UnsupportedErrorKind,
    },
    ColorType, ExtendedColorType, ImageDecoder, ImageError, ImageResult, LimitSupport, Limits,
};
use jpeg2000_rs::{
    jp2::{
        parse_and_inspect, BoxType, ColorMethod, ComponentBitDepth, ENUMERATED_GRAYSCALE,
        ENUMERATED_SRGB,
    },
    DecodeError, DecodeOptions, Resource,
};
use std::{
    borrow::Cow,
    error::Error,
    fmt,
    io::{self, Read, Seek, SeekFrom},
};

const JP2_SIGNATURE: &[u8] = b"\0\0\0\x0cjP  \r\n\x87\n";

fn format() -> ImageFormatHint {
    ImageFormatHint::Name("JPEG 2000 (JP2)".into())
}

fn unsupported(reason: &str) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        format(),
        UnsupportedErrorKind::GenericFeature(reason.into()),
    ))
}

fn memory_limit() -> ImageError {
    ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
}

fn probe_error(error: ProbeError) -> ImageError {
    match error {
        ProbeError::UnsupportedLayout(reason) => unsupported(reason),
        ProbeError::BridgeBufferLimit { .. } | ProbeError::Allocation(_) => memory_limit(),
        error => ImageError::Decoding(DecodingError::new(format(), error)),
    }
}

fn decoder_error(error: DecodeError) -> ImageError {
    probe_error(ProbeError::Decoder(error))
}

/// Prototype `image` 0.25.10 decoder for single-codestream JP2.
///
/// Supports opaque enumerated grayscale/sRGB with uniform unsigned 8/16-bit
/// components, without palettes, component mappings or channel definitions.
/// Output is normalized `L16`/`Rgb16` in native byte order. Other layouts are
/// explicitly unsupported, not silently stripped or truncated.
///
/// Construction inspects metadata; pixel decoding waits until `read_image`.
/// Codec limits are always retained. `set_limits` checks strict dimensions
/// before pixel decoding and uses `max_alloc` as a best-effort budget for owned
/// compressed input, the copied bridge buffer and codec-accounted decoded bytes.
/// This is not a bound on RSS, allocator overhead or all native temporaries.
/// Caller-owned output is budgeted separately by `ImageReader::decode`.
///
/// ICC/Exif/XMP/IPTC metadata, animation, rectangle decoding and streaming are
/// not implemented by this bounded consumer prototype.
pub struct Jp2Decoder<'a> {
    input: Cow<'a, [u8]>,
    dimensions: (u32, u32),
    color: ColorType,
    original_color: ExtendedColorType,
    options: DecodeOptions,
    decoding_options: DecodeOptions,
    maximum_bridge_bytes: u64,
}

impl<'a> Jp2Decoder<'a> {
    pub fn new(
        input: &'a [u8],
        options: DecodeOptions,
        maximum_bridge_bytes: u64,
    ) -> ImageResult<Self> {
        Self::from_input(Cow::Borrowed(input), options, maximum_bridge_bytes)
    }

    fn from_input(
        input: Cow<'a, [u8]>,
        options: DecodeOptions,
        maximum_bridge_bytes: u64,
    ) -> ImageResult<Self> {
        let inspected = parse_and_inspect(&input, options.limits()).map_err(decoder_error)?;
        let file = inspected.file();
        if !file.file_type().is_compatible_with(BoxType::JP2_BRAND)
            || file.codestream_count() != 1
            || file.compositing_layer_count() != 0
        {
            return Err(unsupported("expected a single-codestream JP2 image"));
        }
        let header = file.header();
        if header.image.unknown_color_space
            || header.palette.is_some()
            || !header.component_mappings.is_empty()
            || !header.channel_definition_records.is_empty()
        {
            return Err(unsupported(
                "palette, mapping, channel definitions or unknown color space",
            ));
        }
        let (color, original8, original16) = match (header.color.method(), header.image.components)
        {
            (
                ColorMethod::Enumerated {
                    color_space: ENUMERATED_GRAYSCALE,
                },
                1,
            ) => (
                ColorType::L16,
                ExtendedColorType::L8,
                ExtendedColorType::L16,
            ),
            (
                ColorMethod::Enumerated {
                    color_space: ENUMERATED_SRGB,
                },
                3,
            ) => (
                ColorType::Rgb16,
                ExtendedColorType::Rgb8,
                ExtendedColorType::Rgb16,
            ),
            _ => return Err(unsupported("expected opaque enumerated grayscale or sRGB")),
        };
        let original_color = match header.image.bit_depth {
            ComponentBitDepth::Uniform {
                bits: 8,
                signed: false,
            } => original8,
            ComponentBitDepth::Uniform {
                bits: 16,
                signed: false,
            } => original16,
            _ => {
                return Err(unsupported(
                    "expected uniform unsigned 8-bit or 16-bit components",
                ))
            }
        };
        let dimensions = (header.image.width, header.image.height);
        drop(inspected);
        let decoder = Self {
            input,
            dimensions,
            color,
            original_color,
            options,
            decoding_options: options,
            maximum_bridge_bytes,
        };
        if decoder.total_bytes() > maximum_bridge_bytes {
            return Err(memory_limit());
        }
        Ok(decoder)
    }
}

impl Jp2Decoder<'static> {
    /// Buffers the remaining seekable input under the codec's input-byte limit.
    ///
    /// `ImageReader` supplies its limits after hook construction, so compressed
    /// buffering and metadata inspection use the caller's `DecodeOptions`,
    /// not later `image::Limits`. No pixel decoding happens here.
    pub fn from_reader<R: Read + Seek>(
        mut reader: R,
        options: DecodeOptions,
        maximum_bridge_bytes: u64,
    ) -> ImageResult<Self> {
        let start = reader.stream_position()?;
        let end = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(start))?;
        let length = end.checked_sub(start).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "input position exceeds its length",
            )
        })?;
        let maximum = options.limits().maximum_input_bytes();
        if length > maximum {
            return Err(decoder_error(DecodeError::LimitExceeded {
                resource: Resource::InputBytes,
                limit: maximum,
                actual: length,
            }));
        }
        let length = usize::try_from(length).map_err(|_| memory_limit())?;
        let mut input = Vec::new();
        input
            .try_reserve_exact(length)
            .map_err(|_| memory_limit())?;
        input.resize(length, 0);
        reader.read_exact(&mut input)?;
        let mut extra = [0];
        let extra_length = loop {
            match reader.read(&mut extra) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        if extra_length != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input size changed while buffering",
            )
            .into());
        }
        Self::from_input(Cow::Owned(input), options, maximum_bridge_bytes)
    }
}

impl ImageDecoder for Jp2Decoder<'_> {
    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    fn color_type(&self) -> ColorType {
        self.color
    }

    fn original_color_type(&self) -> ExtendedColorType {
        self.original_color
    }

    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&LimitSupport::default())?;
        limits.check_dimensions(self.dimensions.0, self.dimensions.1)?;
        let mut remaining = limits;
        if matches!(self.input, Cow::Owned(_)) {
            remaining.reserve_usize(self.input.len())?;
        }
        remaining.reserve(self.total_bytes())?;
        let original = self.options.limits();
        let maximum = remaining
            .max_alloc
            .map_or(original.maximum_decoded_bytes(), |budget| {
                budget.min(original.maximum_decoded_bytes())
            });
        let tightened = original
            .with_maximum_decoded_bytes(maximum)
            .map_err(decoder_error)?;
        self.decoding_options = DecodeOptions::new(tightened);
        Ok(())
    }

    fn read_image(self, buffer: &mut [u8]) -> ImageResult<()> {
        assert_eq!(
            u64::try_from(buffer.len()).ok(),
            Some(self.total_bytes()),
            "output buffer length differs from ImageDecoder::total_bytes",
        );
        let image = decode_jp2_image(
            &self.input,
            &self.decoding_options,
            self.maximum_bridge_bytes,
        )
        .map_err(probe_error)?;
        if (image.width(), image.height()) != self.dimensions || image.color() != self.color {
            return Err(probe_error(ProbeError::InvalidLayout(
                "decoded layout differs from inspected metadata",
            )));
        }
        buffer.copy_from_slice(image.as_bytes());
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buffer: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buffer)
    }
}

#[derive(Debug)]
pub struct HookRegistrationError;

impl fmt::Display for HookRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an image decoding hook for JP2 is already registered")
    }
}

impl Error for HookRegistrationError {}

/// Opt in to process-wide JP2 detection/decoding hooks without replacing an
/// existing decoder. Registration is not automatic and cannot be undone through
/// `image` 0.25.10's public API.
///
/// ```no_run
/// use jpeg2000_image_consumer_probe::register_jp2_hook;
/// use jpeg2000_rs::DecodeOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// register_jp2_hook(DecodeOptions::default(), 64 * 1024 * 1024)?;
/// let pixels = image::ImageReader::open("example.jp2")?
///     .with_guessed_format()?
///     .decode()?;
/// # Ok(())
/// # }
/// ```
pub fn register_jp2_hook(
    options: DecodeOptions,
    maximum_bridge_bytes: u64,
) -> Result<(), HookRegistrationError> {
    let installed = image::hooks::register_decoding_hook(
        "jp2".into(),
        Box::new(move |reader| {
            Ok(Box::new(Jp2Decoder::from_reader(
                reader,
                options,
                maximum_bridge_bytes,
            )?))
        }),
    );
    if !installed {
        return Err(HookRegistrationError);
    }
    image::hooks::register_format_detection_hook("jp2".into(), JP2_SIGNATURE, None);
    Ok(())
}
