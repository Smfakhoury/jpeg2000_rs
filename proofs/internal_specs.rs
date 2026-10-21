use vstd::prelude::*;

use crate::specs::{Part2CompositionGeometry, part2_composition_canvas_prefix};

verus! {

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
