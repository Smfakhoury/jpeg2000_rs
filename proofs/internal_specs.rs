use vstd::prelude::*;

use crate::specs::{Part2CompositionGeometry, part2_composition_canvas_prefix};

verus! {

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
