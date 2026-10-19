use std::{env, fs, process};

use jpeg2000_rs::{
    DatabinCache, DecodeError, DecodeOptions, ResourceLimits, TypedImage, TypedSamples,
    decode_codestream, decode_jp2, decode_jph, decode_jpx_codestream, ingest_jpip_response,
    inspect_jph,
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

fn main() {
    let arguments = env::args().collect::<Vec<_>>();
    if arguments.len() != 4 {
        eprintln!("usage: validation_decoder <decode|inspect|ingest> <format> <input>");
        process::exit(2);
    }
    let operation = arguments[1].as_str();
    let format = arguments[2].as_str();
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
