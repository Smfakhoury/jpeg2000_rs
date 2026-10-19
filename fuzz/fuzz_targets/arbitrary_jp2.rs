#![no_main]

use jpeg2000_rs::{
    DecodeOptions, DecodeSelection, ImageRegion, decode_jp2, decode_jp2_color,
    decode_jp2_color_selected, decode_jp2_selected, decode_jph, decode_jph_color,
    decode_jph_color_selected, decode_jph_selected, decode_jpx_codestream,
    decode_jpx_codestream_color, decode_jpx_codestream_color_selected,
    decode_jpx_codestream_selected, inspect_jph, inspect_jpx_layers, jp2, render_jpx_composition,
    render_jpx_frame, render_jpx_layer,
};
use jpeg2000_rs_fuzz::resource_limits;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = resource_limits();
    let seed = data.first().copied().unwrap_or(0);
    let selection = DecodeSelection::full_image()
        .with_region(ImageRegion::new(
            u32::from(seed & 3),
            u32::from((seed >> 2) & 3),
            u32::from(((seed >> 4) & 3) + 1),
            u32::from(((seed >> 6) & 3) + 1),
        ))
        .with_discard_levels(seed & 3);
    let _ = decode_jp2(data, &DecodeOptions::new(limits));
    let _ = decode_jp2_color(data, &DecodeOptions::new(limits));
    let _ = decode_jp2_selected(data, &DecodeOptions::new(limits), &selection);
    let _ = decode_jp2_color_selected(data, &DecodeOptions::new(limits), &selection);
    let _ = inspect_jph(data, &limits);
    let _ = decode_jph(data, &DecodeOptions::new(limits));
    let _ = decode_jph_color(data, &DecodeOptions::new(limits));
    let _ = decode_jph_selected(data, &DecodeOptions::new(limits), &selection);
    let _ = decode_jph_color_selected(data, &DecodeOptions::new(limits), &selection);
    let selected_index = usize::from(data.first().copied().unwrap_or(0) & 3);
    let _ = decode_jpx_codestream(data, selected_index, &DecodeOptions::new(limits));
    let _ = decode_jpx_codestream_color(data, selected_index, &DecodeOptions::new(limits));
    let _ = decode_jpx_codestream_selected(
        data,
        selected_index,
        &DecodeOptions::new(limits),
        &selection,
    );
    let _ = decode_jpx_codestream_color_selected(
        data,
        selected_index,
        &DecodeOptions::new(limits),
        &selection,
    );
    let _ = inspect_jpx_layers(data, &limits);
    let _ = render_jpx_layer(data, selected_index, &DecodeOptions::new(limits));
    let _ = render_jpx_composition(data, &DecodeOptions::new(limits));
    let _ = render_jpx_frame(data, selected_index, &DecodeOptions::new(limits));
    let _ = jp2::parse(data, &limits);
    if let Ok(reader) = jp2::BoxReader::new(data, &limits) {
        for result in reader {
            if result.is_err() {
                break;
            }
        }
    }
});
