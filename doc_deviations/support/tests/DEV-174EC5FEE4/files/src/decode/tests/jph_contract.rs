use super::*;
use crate::codestream::ComponentInfo;
use crate::image::TypedSamples;

#[derive(Clone, Copy, Debug)]
struct Plane {
    precision: u8,
    signed: bool,
    dx: u8,
    dy: u8,
}

fn zero_codestream(size: [u32; 4], planes: &[Plane], style: u8, capability: u16) -> Vec<u8> {
    let [width, height, x, y] = size;
    assert!(width > 0 && height > 0 && !planes.is_empty());
    let mut siz = 0x4000_u16.to_be_bytes().to_vec();
    for value in [x + width, y + height, x, y, x + width, y + height, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&u16::try_from(planes.len()).unwrap().to_be_bytes());
    for plane in planes {
        assert!((1..=38).contains(&plane.precision));
        assert!(plane.dx > 0 && plane.dy > 0);
        siz.extend_from_slice(&[
            (plane.precision - 1) | if plane.signed { 0x80 } else { 0 },
            plane.dx,
            plane.dy,
        ]);
    }
    let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
    bytes.extend(segment(0xff51, &siz));
    let mut cap = vec![0, 2, 0, 0];
    cap.extend_from_slice(&capability.to_be_bytes());
    bytes.extend(segment(0xff50, &cap));
    // One layer, no MCT, no decomposition, reversible 5/3, one precinct.
    bytes.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 2, 2, style, 1]));
    let exponent = planes.iter().map(|p| p.precision.min(31)).max().unwrap();
    bytes.extend(segment(0xff5c, &[0x40, exponent << 3]));
    let tile_length = 14 + u32::try_from(planes.len()).unwrap();
    let mut sot = vec![0, 0];
    sot.extend_from_slice(&tile_length.to_be_bytes());
    sot.extend_from_slice(&[0, 1]);
    bytes.extend(segment(0xff90, &sot));
    bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
    // A zero packet-present bit per component means every coefficient is zero.
    bytes.extend(vec![0; planes.len()]);
    bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
    bytes
}

fn container(
    codestream: &[u8],
    size: [u32; 4],
    planes: &[Plane],
    file_type: &[u8],
    colour: bool,
) -> Vec<u8> {
    let mut bytes = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
    bytes.extend(box32(*b"ftyp", file_type));
    let mut ihdr = size[1].to_be_bytes().to_vec();
    ihdr.extend_from_slice(&size[0].to_be_bytes());
    ihdr.extend_from_slice(&u16::try_from(planes.len()).unwrap().to_be_bytes());
    ihdr.extend_from_slice(&[255, 7, u8::from(!colour), 0]);
    let mut header = box32(*b"ihdr", &ihdr);
    let depths: Vec<_> = planes
        .iter()
        .map(|p| (p.precision - 1) | if p.signed { 0x80 } else { 0 })
        .collect();
    header.extend(box32(*b"bpcc", &depths));
    if colour {
        let space = if planes.len() == 1 { 17 } else { 16 };
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, space]));
    }
    // D.2: no colr implies UnkC=1 and no channel definition with Typ=0.
    // There is no cdef box here, hence no Typ fields at all.
    assert_eq!(ihdr[12], u8::from(!colour));
    bytes.extend(box32(*b"jp2h", &header));
    bytes.extend(box32(*b"jp2c", codestream));
    bytes
}

fn assert_zero_image(image: &TypedImage, size: [u32; 4], planes: &[Plane]) {
    let [width, height, x, y] = size;
    assert_eq!(
        (
            image.width(),
            image.height(),
            image.x_origin(),
            image.y_origin()
        ),
        (width, height, x, y)
    );
    assert_eq!(image.components().len(), planes.len());
    for (component, plane) in image.components().iter().zip(planes) {
        let cw = (x + width).div_ceil(u32::from(plane.dx)) - x.div_ceil(u32::from(plane.dx));
        let ch = (y + height).div_ceil(u32::from(plane.dy)) - y.div_ceil(u32::from(plane.dy));
        assert_eq!((component.width(), component.height()), (cw, ch));
        assert_eq!(
            component.info(),
            ComponentInfo {
                precision: plane.precision,
                signed: plane.signed,
                x_subsampling: plane.dx,
                y_subsampling: plane.dy,
                x_registration: 0,
                y_registration: 0,
            }
        );
        let count = usize::try_from(cw * ch).unwrap();
        let expected = if plane.signed {
            TypedSamples::Signed(vec![0; count])
        } else {
            TypedSamples::Unsigned(vec![1_u64 << (plane.precision - 1); count])
        };
        assert_eq!(component.samples(), &expected);
    }
}

#[test]
fn typed_planes_independent_of_colour_conversion() {
    // Reuse the repository's nonzero cleanup and refinement oracles, too.
    top_level_jph_decodes_cleanup_only_ht_coefficients();
    top_level_raw_and_jph_decode_ht_refinement();
    let options = DecodeOptions::default();
    let mut seed = 0x5eeda11_u32;
    let mut missing_colour_failures = Vec::new();
    for index in 0..24 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let precision = [1, 2, 7, 8, 12, 16, 24, 31, 32, 38][index % 10];
        let planes = [
            Plane {
                precision,
                signed: (index / 10) % 2 == 0,
                dx: 1,
                dy: 1,
            },
            Plane {
                precision: 8,
                signed: false,
                dx: 2,
                dy: 3,
            },
            Plane {
                precision: 12,
                signed: true,
                dx: 3,
                dy: 2,
            },
        ];
        let planes = &planes[..if index % 3 == 0 { 1 } else { 3 }];
        let size = if index == 0 {
            [1, 1, 0, 0]
        } else {
            [
                4 + seed % 13,
                4 + (seed >> 8) % 11,
                (seed >> 16) % 4,
                (seed >> 24) % 4,
            ]
        };
        let cs = zero_codestream(size, planes, 0x40, 0x001f);
        eprintln!("decode_jph typed planes: index={index}, size={size:?}, planes={planes:?}");
        let known = container(&cs, size, planes, b"jph \0\0\0\0jph ", true);
        let decoded = decode_jph(&known, &options).expect("supported JPH with explicit colour");
        assert_zero_image(&decoded, size, planes);

        // D.2 permits this change. Only colour metadata differs; the component
        // samples and properties must not depend on implicit colour conversion.
        let unspecified = container(&cs, size, planes, b"jph \0\0\0\0jph ", false);
        match decode_jph(&unspecified, &options) {
            Ok(image) => {
                assert_zero_image(&image, size, planes);
                assert_eq!(image, decoded);
            }
            Err(error) => {
                eprintln!("D.2 component-only JPH rejected: {error:?}");
                missing_colour_failures.push((index, error));
            }
        }
    }
    assert!(
        missing_colour_failures.is_empty(),
        "UnkC=1, no colr, no cdef must return the same typed planes: {missing_colour_failures:?}"
    );
}

#[test]
fn branding_and_capability_gate_decoding() {
    let size = [7, 5, 0, 0];
    let planes = [Plane {
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    let options = DecodeOptions::default();
    for (style, capability) in [
        (0x40, 0x001f),
        (0x00, 0x801f),
        (0x40, 0x801f),
        (0xc0, 0xc01f),
    ] {
        eprintln!("decode_jph valid gate: style={style:#x}, Ccap15={capability:#x}");
        let cs = zero_codestream(size, &planes, style, capability);
        let bytes = container(&cs, size, &planes, b"jph \0\0\0\0jph ", true);
        assert_zero_image(&decode_jph(&bytes, &options).unwrap(), size, &planes);
    }

    let cs = zero_codestream(size, &planes, 0x40, 0x001f);
    for (label, ftyp) in [
        ("wrong primary brand", &b"jp2 \0\0\0\0jph "[..]),
        ("missing JPH compatibility", &b"jph \0\0\0\0jp2 "[..]),
        ("nonzero minor version", &b"jph \0\0\0\x01jph "[..]),
    ] {
        let bytes = container(&cs, size, &planes, ftyp, true);
        eprintln!("decode_jph rejection: {label}");
        assert!(
            matches!(
                decode_jph(&bytes, &options),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidProfile,
                    ..
                })
            ),
            "{label}"
        );
    }

    for (label, style, capability) in [
        ("legacy blocks in HT-only declaration", 0x00, 0x001f),
        ("mixed blocks in HT-declared declaration", 0xc0, 0x801f),
        ("reserved Ccap15 mode", 0x40, 0x401f),
        ("reserved Ccap15 bit", 0x40, 0x005f),
    ] {
        let cs = zero_codestream(size, &planes, style, capability);
        let bytes = container(&cs, size, &planes, b"jph \0\0\0\0jph ", true);
        eprintln!("decode_jph capability rejection: {label}");
        assert!(
            matches!(
                decode_jph(&bytes, &options),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidProfile,
                    ..
                })
            ),
            "{label}"
        );
    }

    // Valid legacy JP2 coding is not a JPH codestream, even with JPH branding.
    let mut legacy = prepared_codestream(7, 5, 1, &[0x40, 0x40], &[0]);
    let cod = legacy.windows(2).position(|w| w == [0xff, 0x52]).unwrap();
    legacy[cod + 9] = 0;
    let bytes = container(&legacy, size, &planes, b"jph \0\0\0\0jph ", true);
    assert!(matches!(
        decode_jph(&bytes, &options),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidProfile,
            ..
        })
    ));

    // Pair valid packet data with a valid non-JPH container: the JPH entry must
    // reject rather than silently acting as decode_jp2.
    let jp2 = container(&legacy, size, &planes, b"jp2 \0\0\0\0jp2 ", true);
    assert_zero_image(&decode_jp2(&jp2, &options).unwrap(), size, &planes);
    assert!(matches!(
        decode_jph(&jp2, &options),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidProfile,
            ..
        })
    ));
}
