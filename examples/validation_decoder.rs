use std::{env, fs, process};

use jpeg2000_rs::{
    DatabinCache, DecodeError, DecodeOptions, DecodeSelection, ImageRegion, ResourceLimits,
    TypedImage, TypedSamples, decode_codestream, decode_codestream_selected, decode_jp2,
    decode_jp2_selected, decode_jph, decode_jph_selected, decode_jpx_codestream,
    decode_jpx_codestream_selected, ingest_jpip_response, inspect_jph,
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

fn print_selected_image(image: &TypedImage) {
    println!("FRAME\t{}\t{}", image.x_origin(), image.y_origin());
    print_image(image);
}

fn main() {
    let arguments = env::args().collect::<Vec<_>>();
    let selecting = arguments.get(1).is_some_and(|value| value == "select");
    if arguments.len() != if selecting { 7 } else { 4 } {
        eprintln!(
            "usage: validation_decoder <decode|inspect|ingest> <format> <input>\n\
             or: validation_decoder select <format> <input> <discard-levels> \
             <all|tile-indices> <all|x,y,width,height>"
        );
        process::exit(2);
    }
    let operation = arguments[1].as_str();
    let format = arguments[2].as_str();
    let selection = if selecting {
        match parse_selection(&arguments[4..]) {
            Ok(selection) => selection,
            Err(error) => {
                eprintln!("invalid selection: {error}");
                process::exit(2);
            }
        }
    } else {
        DecodeSelection::full_image()
    };
    let input = match fs::read(&arguments[3]) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("cannot read input: {error}");
            process::exit(2);
        }
    };
    let options = DecodeOptions::default();
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
