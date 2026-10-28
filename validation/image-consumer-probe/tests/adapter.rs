use image::{
    error::LimitErrorKind, ColorType, DynamicImage, ExtendedColorType, ImageDecoder, ImageError,
    ImageReader, Limits,
};
use jpeg2000_image_consumer_probe::{register_jp2_hook, Jp2Decoder, ProbeError};
use jpeg2000_rs::{DecodeError, DecodeOptions, Resource, ResourceLimits};
use std::{
    error::Error,
    io::{self, Cursor, Read, Seek, SeekFrom},
    sync::OnceLock,
};

const GRAY: &[u8] = include_bytes!("fixtures/gray.jp2");
const RGB: &[u8] = include_bytes!("fixtures/rgb.jp2");

fn decoder(input: &[u8]) -> Jp2Decoder<'_> {
    Jp2Decoder::new(input, DecodeOptions::default(), 1024).unwrap()
}

fn register() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| register_jp2_hook(DecodeOptions::default(), 1024).unwrap());
}

fn precision_fixture(bits: u8) -> Vec<u8> {
    let mut bytes = GRAY.to_vec();
    let ihdr = bytes.windows(4).position(|part| part == b"ihdr").unwrap();
    let siz = bytes
        .windows(2)
        .position(|part| part == b"\xff\x51")
        .unwrap();
    let qcd = bytes
        .windows(2)
        .position(|part| part == b"\xff\x5c")
        .unwrap();
    bytes[ihdr + 14] = bits - 1;
    bytes[siz + 40] = bits - 1;
    bytes[qcd + 5] = bits << 3;
    bytes
}

#[test]
fn metadata_reports_output_and_original_color() {
    let gray = decoder(GRAY);
    assert_eq!(gray.dimensions(), (1, 1));
    assert_eq!(gray.color_type(), ColorType::L16);
    assert_eq!(gray.original_color_type(), ExtendedColorType::L8);
    assert_eq!(gray.total_bytes(), 2);
    let rgb = decoder(RGB);
    assert_eq!(rgb.color_type(), ColorType::Rgb16);
    assert_eq!(rgb.original_color_type(), ExtendedColorType::Rgb8);
    assert_eq!(rgb.total_bytes(), 6);
}

#[test]
fn trait_decodes_gray_and_rgb_through_dynamic_image() {
    let gray = DynamicImage::from_decoder(decoder(GRAY)).unwrap();
    assert_eq!(gray.as_luma16().unwrap().as_raw(), &[128 * 257]);
    let rgb = DynamicImage::from_decoder(decoder(RGB)).unwrap();
    assert_eq!(rgb.as_rgb16().unwrap().as_raw(), &[128 * 257; 3]);
}

#[test]
fn sixteen_bit_decode_writes_native_endian_bytes() {
    let fixture = precision_fixture(16);
    let decoder = decoder(&fixture);
    assert_eq!(decoder.original_color_type(), ExtendedColorType::L16);
    let mut bytes = [0; 2];
    decoder.read_image(&mut bytes).unwrap();
    assert_eq!(bytes, 32768_u16.to_ne_bytes());
}

#[test]
fn boxed_trait_object_decodes_without_alignment_requirements() {
    let decoder: Box<dyn ImageDecoder> = Box::new(decoder(GRAY));
    let mut bytes = [0; 3];
    decoder.read_image(&mut bytes[1..]).unwrap();
    assert_eq!(&bytes[1..], &(128_u16 * 257).to_ne_bytes());
}

#[test]
#[should_panic(expected = "output buffer length")]
fn wrong_buffer_length_obeys_trait_panic_contract() {
    decoder(GRAY).read_image(&mut [0]).unwrap();
}

#[test]
fn strict_dimensions_are_rejected_before_pixel_decode() {
    let limits = ResourceLimits::default()
        .with_maximum_decoded_bytes(0)
        .unwrap();
    let mut decoder = Jp2Decoder::new(GRAY, DecodeOptions::new(limits), 1024).unwrap();
    let mut image_limits = Limits::no_limits();
    image_limits.max_image_width = Some(0);
    assert!(
        matches!(decoder.set_limits(image_limits), Err(ImageError::Limits(error))
        if error.kind() == LimitErrorKind::DimensionError)
    );
}

#[test]
fn pixel_decode_is_deferred_and_codec_decoded_limit_is_preserved() {
    let limits = ResourceLimits::default()
        .with_maximum_decoded_bytes(0)
        .unwrap();
    let decoder = Jp2Decoder::new(GRAY, DecodeOptions::new(limits), 1024).unwrap();
    let error = decoder.read_image(&mut [0; 2]).unwrap_err();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<ProbeError>()
        .unwrap();
    assert!(matches!(
        source,
        ProbeError::Decoder(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            ..
        })
    ));
}

#[test]
fn metadata_work_limit_is_preserved_during_construction() {
    let limits = ResourceLimits::default()
        .with_maximum_work_units(0)
        .unwrap();
    let error = Jp2Decoder::new(GRAY, DecodeOptions::new(limits), 1024)
        .err()
        .unwrap();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<ProbeError>()
        .unwrap();
    assert!(matches!(
        source,
        ProbeError::Decoder(DecodeError::LimitExceeded {
            resource: Resource::WorkUnits,
            limit: 0,
            ..
        })
    ));
}

#[test]
fn insufficient_bridge_budget_fails_on_metadata() {
    assert!(matches!(
        Jp2Decoder::new(RGB, DecodeOptions::default(), 5),
        Err(ImageError::Limits(error)) if error.kind() == LimitErrorKind::InsufficientMemory
    ));
}

#[test]
fn image_allocation_budget_checks_the_internal_bridge_copy() {
    let mut decoder = decoder(RGB);
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(5);
    assert!(
        matches!(decoder.set_limits(limits), Err(ImageError::Limits(error))
        if error.kind() == LimitErrorKind::InsufficientMemory)
    );
}

#[test]
fn limits_can_be_reset_without_raising_original_codec_limits() {
    let options = DecodeOptions::new(
        ResourceLimits::default()
            .with_maximum_decoded_bytes(0)
            .unwrap(),
    );
    let mut decoder = Jp2Decoder::new(GRAY, options, 1024).unwrap();
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(64);
    decoder.set_limits(limits).unwrap();
    decoder.set_limits(Limits::no_limits()).unwrap();
    let error = decoder.read_image(&mut [0; 2]).unwrap_err();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<ProbeError>()
        .unwrap();
    assert!(matches!(
        source,
        ProbeError::Decoder(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: 0,
            ..
        })
    ));
}

#[test]
fn failed_limit_update_does_not_partially_change_decoder_state() {
    let mut decoder = decoder(GRAY);
    let mut limits = Limits::no_limits();
    limits.max_image_height = Some(0);
    assert!(decoder.set_limits(limits).is_err());
    decoder.set_limits(Limits::no_limits()).unwrap();
    decoder.read_image(&mut [0; 2]).unwrap();
}

#[test]
fn owned_input_is_included_in_the_later_image_budget() {
    let mut decoder =
        Jp2Decoder::from_reader(Cursor::new(GRAY), DecodeOptions::default(), 1024).unwrap();
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(GRAY.len() as u64 + 1);
    assert!(
        matches!(decoder.set_limits(limits), Err(ImageError::Limits(error))
        if error.kind() == LimitErrorKind::InsufficientMemory)
    );
}

#[test]
fn reader_honors_the_current_position() {
    let mut prefixed = b"prefix".to_vec();
    prefixed.extend_from_slice(GRAY);
    let mut reader = Cursor::new(prefixed);
    reader.set_position(6);
    let decoder = Jp2Decoder::from_reader(reader, DecodeOptions::default(), 1024).unwrap();
    let image = DynamicImage::from_decoder(decoder).unwrap();
    assert_eq!(image.as_luma16().unwrap().as_raw(), &[128 * 257]);
}

#[test]
fn oversized_input_is_rejected_before_reader_allocation() {
    let limits = ResourceLimits::default()
        .with_maximum_input_bytes(1)
        .unwrap();
    let error = Jp2Decoder::from_reader(Cursor::new(GRAY), DecodeOptions::new(limits), 1024)
        .err()
        .unwrap();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<ProbeError>()
        .unwrap();
    assert!(matches!(
        source,
        ProbeError::Decoder(DecodeError::LimitExceeded {
            resource: Resource::InputBytes,
            limit: 1,
            ..
        })
    ));
}

#[test]
fn malformed_input_preserves_the_typed_codec_error() {
    let error = Jp2Decoder::new(b"not JP2", DecodeOptions::default(), 1024)
        .err()
        .unwrap();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<ProbeError>()
        .unwrap();
    assert!(matches!(
        source,
        ProbeError::Decoder(DecodeError::MalformedInput { .. })
    ));
}

#[test]
fn unsupported_component_precision_is_not_truncated() {
    let fixture = precision_fixture(12);
    assert!(matches!(
        Jp2Decoder::new(&fixture, DecodeOptions::default(), 1024),
        Err(ImageError::Unsupported(_))
    ));
}

#[test]
fn unsupported_color_profile_is_explicit() {
    let mut bytes = RGB.to_vec();
    let color = bytes.windows(4).position(|part| part == b"colr").unwrap();
    bytes[color + 7..color + 11].copy_from_slice(&18_u32.to_be_bytes());
    assert!(matches!(
        Jp2Decoder::new(&bytes, DecodeOptions::default(), 1024),
        Err(ImageError::Unsupported(_))
    ));
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "test read failure",
        ))
    }
}

impl Seek for FailingReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        Ok(if matches!(position, SeekFrom::End(_)) {
            1
        } else {
            0
        })
    }
}

#[test]
fn reader_io_error_is_not_replaced_by_a_format_error() {
    assert!(matches!(
        Jp2Decoder::from_reader(FailingReader, DecodeOptions::default(), 1024),
        Err(ImageError::IoError(error)) if error.kind() == io::ErrorKind::PermissionDenied
    ));
}

#[test]
fn registered_detection_hook_drives_image_reader_for_gray_and_rgb() {
    register();
    for (input, color) in [(GRAY, ColorType::L16), (RGB, ColorType::Rgb16)] {
        let image = ImageReader::new(Cursor::new(input))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(image.color(), color);
        assert_eq!((image.width(), image.height()), (1, 1));
    }
}

#[test]
fn registered_extension_hook_drives_file_open_without_format_guessing() {
    register();
    let image = ImageReader::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/gray.jp2"
    ))
    .unwrap()
    .decode()
    .unwrap();
    assert_eq!(image.as_luma16().unwrap().as_raw(), &[128 * 257]);
}

#[test]
fn image_reader_into_decoder_preserves_the_public_trait_contract() {
    register();
    let decoder = ImageReader::new(Cursor::new(RGB))
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap();
    assert_eq!(decoder.dimensions(), (1, 1));
    assert_eq!(decoder.original_color_type(), ExtendedColorType::Rgb8);
    let image = DynamicImage::from_decoder(decoder).unwrap();
    assert_eq!(image.as_rgb16().unwrap().as_raw(), &[128 * 257; 3]);
}

#[test]
fn image_reader_propagates_strict_dimension_limits() {
    register();
    let mut reader = ImageReader::new(Cursor::new(GRAY))
        .with_guessed_format()
        .unwrap();
    let mut limits = Limits::no_limits();
    limits.max_image_height = Some(0);
    reader.limits(limits);
    assert!(matches!(reader.decode(), Err(ImageError::Limits(error))
        if error.kind() == LimitErrorKind::DimensionError));
}

#[test]
fn image_reader_reserves_its_output_before_the_decoder_budget() {
    register();
    let mut reader = ImageReader::new(Cursor::new(GRAY))
        .with_guessed_format()
        .unwrap();
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(1);
    reader.limits(limits);
    assert!(matches!(reader.decode(), Err(ImageError::Limits(error))
        if error.kind() == LimitErrorKind::InsufficientMemory));
}

#[test]
fn repeated_hook_registration_fails_without_replacing_the_first() {
    register();
    assert!(register_jp2_hook(DecodeOptions::default(), 0).is_err());
    ImageReader::new(Cursor::new(GRAY))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();
}
