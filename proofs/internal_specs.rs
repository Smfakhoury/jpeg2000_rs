use vstd::prelude::*;

use crate::specs::{Part2CompositionGeometry, part2_composition_canvas_prefix};

verus! {

#[verifier::opaque]
pub open spec fn part2_chroma_byte_bound(count: nat) -> nat
    decreases count,
{
    if count == 0 { 0 } else { part2_chroma_byte_bound((count - 1) as nat) * 256 + 255 }
}

pub open spec fn part2_chroma_key_prefix_matches(
    colors: Seq<Seq<u16>>, keys: Seq<u16>, pixel: int, count: nat,
) -> bool {
    forall|channel: int| 0 <= channel < count ==>
        #[trigger] crate::specs::part2_chroma_key_channel_matches(colors, keys, pixel, channel)
}

pub open spec fn color_crop_ready(
    image: crate::specs::ColorImageModel, x: u32, y: u32, width: u32, height: u32, x1: u32, y1: u32, count: usize,
) -> bool {
    width > 0 && height > 0
        && x1 as nat == (x as nat) + width as nat
        && y1 as nat == (y as nat) + height as nat
        && x1 <= image.0 && y1 <= image.1
        && !(x == 0 && y == 0 && width == image.0 && height == image.1)
        && crate::specs::part2_layer_area(width, height) == Ok(count)
}

pub open spec fn color_conversion_ready(
    components: Seq<crate::specs::Part1TypedComponentModel>,
    definitions: Seq<Seq<crate::specs::ColorDefinition>>, geometry: crate::specs::ColorGeometry,
    method: crate::jp2::ColorMethod, layout: crate::specs::ColorLayoutState,
    format: (usize, crate::color::ConvertedColorSpace, bool), area: usize, byte_limit: u64, work_limit: u64,
) -> bool {
    geometry.0 > 0 && geometry.1 > 0
        && crate::specs::color_output_format(method) == Ok(format)
        && crate::specs::color_channel_layout(definitions, format.0) == Ok(layout)
        && crate::specs::color_validate_components(components, geometry, layout) == Ok(())
        && crate::specs::color_validate_output_size(
            geometry.0, geometry.1, format.0, layout.1.len() as usize, byte_limit, work_limit,
        ) == Ok(())
        && crate::specs::part2_layer_area(geometry.0, geometry.1) == Ok(area)
}

pub open spec fn part2_timeline_state_valid(
    state: crate::specs::Part2TimelineState, layers: usize, maximum_frames: u32, limit: usize,
) -> bool {
    state.2 <= layers && state.3 as nat <= state.1.len() <= limit as nat && state.0.len() <= maximum_frames as nat
}

pub open spec fn magnitude_left_prefix(limbs: Seq<u64>, shift: u16, completed: nat) -> Seq<u64> {
    Seq::new(6, |i: int| {
        let source = i - shift / 64;
        let partial = (shift % 64) as u32;
        let low = if 0 <= source < completed { limbs[source] << partial } else { 0u64 };
        let high = if partial > 0 && 0 <= source - 1 < completed {
            limbs[source - 1] >> ((64 - partial) as u32)
        } else { 0u64 };
        low | high
    })
}

pub open spec fn part2_layer_alpha_product_prefix(
    first: Seq<u16>, second: Seq<u16>, count: nat,
) -> Seq<u16> {
    Seq::new(first.len(), |i: int| if 0 <= i < count {
        crate::specs::part2_normalized_sample_product(first[i], second[i]) as u16
    } else {
        first[i]
    })
}

pub open spec fn part2_composition_layer_prefix_matches(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool, initial_work: nat,
    final_colors: Seq<Seq<u16>>, final_alpha: Seq<u16>, final_work: nat, completed: nat,
) -> bool {
    let state = part2_composition_canvas_prefix(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, completed,
    );
    completed <= geometry.copy_width * geometry.copy_height
        && state.0 && final_colors == state.1 && final_alpha == state.2
        && (final_work == initial_work + completed * (destination.len() + 1)
            || (completed > 0 && final_work == initial_work + ((completed - 1) as nat) * (destination.len() + 1)))
}

pub open spec fn part2_composition_layer_prefix_safe(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool, initial_work: nat,
    final_colors: Seq<Seq<u16>>, final_alpha: Seq<u16>, final_work: nat,
) -> bool {
    if geometry.source_crop_width == 0 || geometry.source_crop_height == 0
        || geometry.target_width == 0 || geometry.target_height == 0
        || geometry.copy_width == 0 || geometry.copy_height == 0 {
        final_colors == destination && final_alpha == destination_alpha && final_work == initial_work
    } else {
        exists |completed: nat| part2_composition_layer_prefix_matches(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, initial_work,
            final_colors, final_alpha, final_work, completed,
        )
    }
}

}
