use std::{env, fs, process};

use jpeg2000_rs::{
    ConvertedImage, DatabinCache, DatabinId, DecodeError, DecodeOptions, DecodeSelection,
    ImageRegion, JpipAutomaticProgressiveDecoder, JpipFragment, JpipMessage, JpipMessageParser,
    JpipPrecinctCodestreamPlan, JpipPrecinctPacket, JpipProgressiveDecoder, MalformedKind,
    ResourceLimits, TypedImage, TypedSamples, decode_codestream, decode_codestream_selected,
    decode_jp2, decode_jp2_color, decode_jp2_color_selected, decode_jp2_selected, decode_jph,
    decode_jph_color, decode_jph_color_selected, decode_jph_selected, decode_jpx_codestream,
    decode_jpx_codestream_color, decode_jpx_codestream_color_selected,
    decode_jpx_codestream_selected, ingest_jpip_response, inspect_jph, render_jpx_composition,
    render_jpx_frame, render_jpx_layer,
};

fn print_error(error: DecodeError) -> ! {
    let debug = format!("{error:?}");
    let category = debug.split([' ', '{', '(']).next().unwrap_or("DecodeError");
    println!("ERR\t{category}\t{error}");
    process::exit(1);
}

fn print_image(image: &TypedImage) {
    println!(
        "IMAGE\n{} {} {}",
        image.width(),
        image.height(),
        image.components().len()
    );
    for component in image.components() {
        let info = component.info();
        let (kind, samples) = match component.samples() {
            TypedSamples::Signed(values) => (
                "signed",
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            TypedSamples::Unsigned(values) => (
                "unsigned",
                values
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            component.width(),
            component.height(),
            info.precision,
            u8::from(info.signed),
            kind,
            samples
        );
    }
}

fn parse_selection(arguments: &[String]) -> Result<DecodeSelection, String> {
    let [discard, tiles, region] = arguments else {
        return Err("expected discard levels, tiles, and region".to_owned());
    };
    let discard = discard
        .parse::<u8>()
        .map_err(|error| format!("invalid discard levels: {error}"))?;
    let selection = if tiles == "all" {
        DecodeSelection::full_image()
    } else {
        let indices = tiles
            .split(',')
            .map(str::parse::<u16>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("invalid tile indices: {error}"))?;
        match indices.as_slice() {
            [index] => DecodeSelection::tile(*index),
            _ => DecodeSelection::tiles(indices),
        }
    }
    .with_discard_levels(discard);
    if region == "all" {
        return Ok(selection);
    }
    let coordinates = region
        .split(',')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid region coordinates: {error}"))?;
    let [x, y, width, height] = coordinates.as_slice() else {
        return Err("region must contain x,y,width,height".to_owned());
    };
    Ok(selection.with_region(ImageRegion::new(*x, *y, *width, *height)))
}

fn parse_resource_limit(arguments: &[String]) -> Result<ResourceLimits, String> {
    let [resource, value] = arguments else {
        return Err("expected resource name and limit".to_owned());
    };
    let value = value
        .parse::<u64>()
        .map_err(|error| format!("invalid resource limit: {error}"))?;
    let limits = ResourceLimits::default();
    match resource.as_str() {
        "decoded-bytes" => limits.with_maximum_decoded_bytes(value),
        "work-units" => limits.with_maximum_work_units(value),
        _ => return Err(format!("unsupported resource limit: {resource}")),
    }
    .map_err(|error| error.to_string())
}

fn parse_packet_lengths(mode: &str) -> Result<Option<Vec<u64>>, String> {
    if mode == "auto" {
        return Ok(None);
    }
    let lengths = mode
        .split(',')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid packet lengths: {error}"))?;
    if lengths.contains(&0) || u16::try_from(lengths.len()).is_err() {
        return Err("packet lengths require 1..65535 positive lengths".to_owned());
    }
    Ok(Some(lengths))
}

struct Invocation<'a> {
    operation: &'a str,
    format: &'a str,
    input: &'a str,
    selection: DecodeSelection,
    options: DecodeOptions,
}

fn parse_invocation(arguments: &[String]) -> Result<Invocation<'_>, String> {
    let requested = arguments.get(1).ok_or("missing operation")?;
    let (operation, limited) = match requested.strip_suffix("-limited") {
        Some(operation)
            if matches!(
                operation,
                "decode"
                    | "convert"
                    | "select"
                    | "select-color"
                    | "render"
                    | "render-layer"
                    | "render-frame"
                    | "reconstruct"
                    | "progressive"
            ) =>
        {
            (operation, true)
        }
        Some(_) => return Err(format!("unsupported limited operation: {requested}")),
        None => (requested.as_str(), false),
    };
    let selecting = matches!(operation, "select" | "select-color");
    let base_length = if selecting { 7 } else { 4 };
    if arguments.len() != base_length + if limited { 2 } else { 0 } {
        return Err("incorrect number of operation arguments".to_owned());
    }
    let format = arguments[2].as_str();
    let supported = match operation {
        "decode" | "select" => matches!(format, "raw" | "jp2" | "jph" | "jpx"),
        "convert" | "select-color" => matches!(format, "jp2" | "jph" | "jpx"),
        "render" => format == "jpx",
        "render-layer" | "render-frame" => format.parse::<usize>().is_ok(),
        "reconstruct" | "ingest" => format == "jpip",
        "inspect" => format == "jph",
        "progressive" => parse_packet_lengths(format).is_ok(),
        _ => false,
    };
    if !supported {
        return Err(format!(
            "unsupported operation/selector: {operation}/{format}"
        ));
    }
    let selection = if selecting {
        parse_selection(&arguments[4..7])?
    } else {
        DecodeSelection::full_image()
    };
    let options = if limited {
        DecodeOptions::new(parse_resource_limit(&arguments[base_length..])?)
    } else {
        DecodeOptions::default()
    };
    Ok(Invocation {
        operation,
        format,
        input: &arguments[3],
        selection,
        options,
    })
}

fn print_selected_image(image: &TypedImage) {
    println!("FRAME\t{}\t{}", image.x_origin(), image.y_origin());
    print_image(image);
}

fn print_converted_image(image: &ConvertedImage) {
    println!(
        "IMAGE\n{} {} {}",
        image.width(),
        image.height(),
        image.channels().len()
    );
    for channel in image.channels() {
        let samples = channel
            .samples()
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}\t{}\t{}\t0\tunsigned\t{}",
            image.width(),
            image.height(),
            image.precision(),
            samples
        );
    }
}

fn index_argument(value: &str) -> usize {
    value.parse().unwrap_or_else(|error| {
        eprintln!("invalid index: {error}");
        process::exit(2);
    })
}

fn report_progressive(
    cache: &DatabinCache,
    manual: &mut Option<JpipProgressiveDecoder>,
    automatic: &mut JpipAutomaticProgressiveDecoder,
    options: &DecodeOptions,
) -> Result<(), DecodeError> {
    let update = match manual {
        Some(decoder) => decoder.update(cache, options)?,
        None => automatic.update(cache, options)?,
    };
    println!(
        "UPDATE\t{:?}\t{}\t{}\t{}\t{}",
        update.status(),
        update.previous_layers(),
        update.available_layers(),
        u8::from(update.image().is_some()),
        update.missing_ranges().len()
    );
    if let Some(image) = update.image() {
        print_image(image);
    }
    Ok(())
}

fn progressive(input: &[u8], mode: &str, options: &DecodeOptions) -> Result<(), DecodeError> {
    let limits = options.limits();
    let mut manual = if let Some(lengths) = parse_packet_lengths(mode).unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(2);
    }) {
        let layers = u16::try_from(lengths.len()).unwrap_or_else(|error| {
            eprintln!("invalid layer count: {error}");
            process::exit(2);
        });
        let mut packets = Vec::new();
        let mut offset = 0_u64;
        for (layer, length) in (0..layers).zip(lengths) {
            packets.push(JpipPrecinctPacket::new(
                DatabinId::new(0, 0, 0)?,
                layer,
                offset,
                length,
            )?);
            offset = offset
                .checked_add(length)
                .ok_or(DecodeError::MalformedInput {
                    offset,
                    kind: MalformedKind::ArithmeticOverflow,
                })?;
        }
        Some(JpipProgressiveDecoder::new(
            JpipPrecinctCodestreamPlan::new(
                DatabinId::new(6, 0, 0)?,
                DatabinId::new(2, 0, 0)?,
                packets,
                layers,
                limits,
            )?,
        ))
    } else {
        None
    };
    let mut automatic = JpipAutomaticProgressiveDecoder::new(0);
    let mut cache = DatabinCache::new(*limits);
    let mut parser = JpipMessageParser::new(*limits);
    report_progressive(&cache, &mut manual, &mut automatic, options)?;
    let mut offset = 0;
    while offset < input.len() {
        let (message, consumed) = parser.parse(&input[offset..])?;
        offset += consumed;
        if let JpipMessage::Data { header, body } = message {
            cache.insert(JpipFragment::from_message(header, body, limits)?)?;
            report_progressive(&cache, &mut manual, &mut automatic, options)?;
            report_progressive(&cache, &mut manual, &mut automatic, options)?;
        }
    }
    Ok(())
}

fn main() {
    let arguments = env::args().collect::<Vec<_>>();
    let Invocation {
        operation,
        format,
        input,
        selection,
        options,
    } = parse_invocation(&arguments).unwrap_or_else(|error| {
        eprintln!(
            "{error}\nusage: validation_decoder <decode|convert|inspect|ingest> <format> <input>\n\
             or: validation_decoder <select|select-color> <format> <input> <discard-levels> \
             <all|tile-indices> <all|x,y,width,height>\n\
             or: validation_decoder decode-limited <format> <input> \
             <decoded-bytes|work-units> <limit>\n\
             or: validation_decoder <render|render-layer|render-frame|reconstruct|progressive> \
             <jpx|layer-index|frame-index|jpip|auto|packet-lengths> <input>\n\
             Limited decode/convert/select/select-color/render/render-layer/render-frame/\
             reconstruct/progressive: append '-limited' to the operation and append \
             <decoded-bytes|work-units> <limit> after its ordinary arguments."
        );
        process::exit(2);
    });
    let input = match fs::read(input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("cannot read input: {error}");
            process::exit(2);
        }
    };
    match (operation, format) {
        ("decode", "raw") => match decode_codestream(&input, &options) {
            Ok(image) => print_image(&image),
            Err(error) => print_error(error),
        },
        ("decode", "jp2") => match decode_jp2(&input, &options) {
            Ok(image) => print_image(&image),
            Err(error) => print_error(error),
        },
        ("decode", "jph") => match decode_jph(&input, &options) {
            Ok(image) => print_image(&image),
            Err(error) => print_error(error),
        },
        ("decode", "jpx") => match decode_jpx_codestream(&input, 0, &options) {
            Ok(image) => print_image(&image),
            Err(error) => print_error(error),
        },
        ("convert", "jp2") => match decode_jp2_color(&input, &options) {
            Ok(image) => print_converted_image(&image),
            Err(error) => print_error(error),
        },
        ("convert", "jph") => match decode_jph_color(&input, &options) {
            Ok(image) => print_converted_image(&image),
            Err(error) => print_error(error),
        },
        ("convert", "jpx") => match decode_jpx_codestream_color(&input, 0, &options) {
            Ok(image) => print_converted_image(&image),
            Err(error) => print_error(error),
        },
        ("select-color", "jp2") => match decode_jp2_color_selected(&input, &options, &selection) {
            Ok(image) => print_converted_image(&image),
            Err(error) => print_error(error),
        },
        ("select-color", "jph") => match decode_jph_color_selected(&input, &options, &selection) {
            Ok(image) => print_converted_image(&image),
            Err(error) => print_error(error),
        },
        ("select-color", "jpx") => {
            match decode_jpx_codestream_color_selected(&input, 0, &options, &selection) {
                Ok(image) => print_converted_image(&image),
                Err(error) => print_error(error),
            }
        }
        ("render", "jpx") => match render_jpx_composition(&input, &options) {
            Ok(image) => print_converted_image(&image),
            Err(error) => print_error(error),
        },
        ("render-layer", index) => {
            match render_jpx_layer(&input, index_argument(index), &options) {
                Ok(image) => print_converted_image(&image),
                Err(error) => print_error(error),
            }
        }
        ("render-frame", index) => {
            match render_jpx_frame(&input, index_argument(index), &options) {
                Ok(image) => print_converted_image(&image),
                Err(error) => print_error(error),
            }
        }
        ("reconstruct", "jpip") => {
            let mut cache = DatabinCache::new(*options.limits());
            let result = (|| {
                ingest_jpip_response(&input, &mut cache)?;
                let view = cache.reconstruct_codestream(&[
                    DatabinId::new(6, 0, 0)?,
                    DatabinId::new(2, 0, 0)?,
                    DatabinId::new(4, 0, 0)?,
                ])?;
                let image = decode_codestream(view.as_ref(), &options)?;
                let bytes = view
                    .as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                println!("BYTES\t{bytes}");
                print_image(&image);
                Ok::<_, DecodeError>(())
            })();
            if let Err(error) = result {
                print_error(error);
            }
        }
        ("progressive", mode) => {
            if let Err(error) = progressive(&input, mode, &options) {
                print_error(error);
            }
        }
        ("select", "raw") => match decode_codestream_selected(&input, &options, &selection) {
            Ok(image) => print_selected_image(&image),
            Err(error) => print_error(error),
        },
        ("select", "jp2") => match decode_jp2_selected(&input, &options, &selection) {
            Ok(image) => print_selected_image(&image),
            Err(error) => print_error(error),
        },
        ("select", "jph") => match decode_jph_selected(&input, &options, &selection) {
            Ok(image) => print_selected_image(&image),
            Err(error) => print_error(error),
        },
        ("select", "jpx") => {
            match decode_jpx_codestream_selected(&input, 0, &options, &selection) {
                Ok(image) => print_selected_image(&image),
                Err(error) => print_error(error),
            }
        }
        ("inspect", "jph") => match inspect_jph(&input, &ResourceLimits::default()) {
            Ok(inspection) => println!(
                "META\tjph\tcodestreams={}",
                inspection.file().codestreams().len()
            ),
            Err(error) => print_error(error),
        },
        ("ingest", "jpip") => {
            let mut cache = DatabinCache::new(ResourceLimits::default());
            match ingest_jpip_response(&input, &mut cache) {
                Ok(summary) => println!(
                    "META\tjpip\tmessages={}\teor={:?}",
                    summary.messages(),
                    summary.end_of_response_reason()
                ),
                Err(error) => print_error(error),
            }
        }
        _ => {
            eprintln!("unsupported operation/format: {operation}/{format}");
            process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jpeg2000_rs::TileSelection;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn limited_invocations_reuse_checked_limits_and_preserve_ordinary_modes() {
        for (operation, selector) in [
            ("decode", "raw"),
            ("convert", "jp2"),
            ("select", "raw"),
            ("select-color", "jp2"),
            ("render", "jpx"),
            ("render-layer", "0"),
            ("render-frame", "1"),
            ("reconstruct", "jpip"),
            ("progressive", "auto"),
            ("progressive", "5,7"),
        ] {
            let mut ordinary = arguments(&["subject", operation, selector, "input"]);
            if matches!(operation, "select" | "select-color") {
                ordinary.extend(arguments(&["0", "0", "all"]));
            }
            let parsed = parse_invocation(&ordinary).unwrap();
            assert_eq!(parsed.operation, operation);
            assert_eq!(parsed.options.limits(), &ResourceLimits::default());
            for resource in ["decoded-bytes", "work-units"] {
                let mut limited = ordinary.clone();
                limited[1] = format!("{operation}-limited");
                limited.extend(arguments(&[resource, "0"]));
                let parsed = parse_invocation(&limited).unwrap();
                assert_eq!(parsed.operation, operation);
                assert_eq!(
                    parsed.options.limits(),
                    &parse_resource_limit(&arguments(&[resource, "0"])).unwrap()
                );
                if matches!(operation, "select" | "select-color") {
                    assert!(matches!(
                        parsed.selection.tile_selection(),
                        TileSelection::Index(0)
                    ));
                }
            }
        }
    }

    #[test]
    fn invalid_limited_modes_selectors_and_budgets_are_usage_errors() {
        for values in [
            vec![],
            vec!["subject"],
            vec![
                "subject",
                "inspect-limited",
                "jph",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "render-limited",
                "raw",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "render-layer-limited",
                "-1",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "render-frame-limited",
                "x",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "reconstruct-limited",
                "raw",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "progressive-limited",
                "",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "progressive-limited",
                "1,",
                "input",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "progressive-limited",
                "0",
                "input",
                "work-units",
                "1",
            ],
            vec!["subject", "decode-limited", "raw", "input", "work-units"],
            vec!["subject", "decode-limited", "raw", "input", "pixels", "1"],
            vec![
                "subject",
                "decode-limited",
                "raw",
                "input",
                "work-units",
                "-1",
            ],
            vec![
                "subject",
                "decode-limited",
                "raw",
                "input",
                "work-units",
                "1000000001",
            ],
            vec![
                "subject",
                "select-limited",
                "raw",
                "input",
                "0",
                "0",
                "work-units",
                "1",
            ],
            vec![
                "subject",
                "select-color-limited",
                "jp2",
                "input",
                "x",
                "all",
                "all",
                "work-units",
                "1",
            ],
        ] {
            assert!(parse_invocation(&arguments(&values)).is_err(), "{values:?}");
        }
        assert!(parse_packet_lengths(&vec!["1"; 65536].join(",")).is_err());
    }

    #[test]
    fn parses_only_tightened_resource_limits_without_changing_other_defaults() {
        let defaults = ResourceLimits::default();
        for value in [0, 1, defaults.maximum_decoded_bytes()] {
            let limits =
                parse_resource_limit(&arguments(&["decoded-bytes", &value.to_string()])).unwrap();
            assert_eq!(limits.maximum_decoded_bytes(), value);
            assert_eq!(limits.maximum_work_units(), defaults.maximum_work_units());
        }
        for value in [0, 1, defaults.maximum_work_units()] {
            let limits =
                parse_resource_limit(&arguments(&["work-units", &value.to_string()])).unwrap();
            assert_eq!(limits.maximum_work_units(), value);
            assert_eq!(
                limits.maximum_decoded_bytes(),
                defaults.maximum_decoded_bytes()
            );
        }
        for values in [
            vec![],
            vec!["decoded-bytes"],
            vec!["decoded-bytes", "1", "extra"],
            vec!["pixels", "1"],
            vec!["decoded-bytes", "-1"],
            vec!["work-units", "not-a-number"],
            vec!["decoded-bytes", "18446744073709551615"],
            vec!["work-units", "18446744073709551615"],
        ] {
            assert!(parse_resource_limit(&arguments(&values)).is_err());
        }
    }

    #[test]
    fn parses_full_single_tile_and_tile_set_selections() {
        let full = parse_selection(&arguments(&["2", "all", "all"])).unwrap();
        assert_eq!(full.discard_levels(), 2);
        assert!(matches!(full.tile_selection(), TileSelection::All));
        assert_eq!(full.region(), None);
        let tile = parse_selection(&arguments(&["0", "3", "4,6,17,19"])).unwrap();
        assert!(matches!(tile.tile_selection(), TileSelection::Index(3)));
        assert_eq!(tile.region(), Some(ImageRegion::new(4, 6, 17, 19)));
        let tiles = parse_selection(&arguments(&["1", "3,5", "all"])).unwrap();
        assert_eq!(tiles.tile_selection(), &TileSelection::Set(vec![3, 5]));
    }

    #[test]
    fn rejects_incomplete_or_non_numeric_selection_arguments() {
        for values in [
            vec![],
            vec!["1", "all"],
            vec!["256", "all", "all"],
            vec!["0", "65536", "all"],
            vec!["0", "1,", "all"],
            vec!["0", "all", "1,2,3"],
            vec!["0", "all", "1,2,3,4,5"],
            vec!["0", "all", "1,2,-3,4"],
        ] {
            assert!(parse_selection(&arguments(&values)).is_err());
        }
    }
}
