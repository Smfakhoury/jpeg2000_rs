use crate::{
    DecodeError, DecodeOptions, MalformedKind, Resource, ResourceLimits,
    codestream::ComponentInfo,
    color::{ConvertedChannelType, ConvertedColorSpace, ConvertedImage},
    image::{
        ChannelAssociation, ChannelDefinition, ChannelType, TypedComponent, TypedImage,
        TypedSamples,
    },
    jp2::{
        self, ComponentBitDepth, CompositingLayer, CompositionInstruction, Jp2File, LayerOpacity,
    },
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpxLayerInfo {
    index: usize,
    codestream_indices: Vec<u16>,
    horizontal_grid_size: u16,
    vertical_grid_size: u16,
    horizontal_offset: u32,
    vertical_offset: u32,
    width: u32,
    height: u32,
    has_color_group: bool,
    has_opacity_box: bool,
    nested_metadata_boxes: u32,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpxCompositionInfo {
    width: u32,
    height: u32,
    loop_count: u8,
    instruction_sets: u32,
    frames: Vec<JpxFrameInfo>,
    total_duration_milliseconds: u64,
    has_unknown_boxes: bool,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpxFrameInfo {
    index: usize,
    start_milliseconds: u64,
    duration_milliseconds: u32,
    persistent: bool,
    instructions: Vec<JpxInstructionInfo>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpxInstructionInfo {
    layer_index: usize,
    target_x: u32,
    target_y: u32,
    target_width: u32,
    target_height: u32,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
    persistent: bool,
    next_reuse: u32,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpxInspection {
    layers: Vec<JpxLayerInfo>,
    composition: Option<JpxCompositionInfo>,
    metadata_boxes: u32,
    metadata_depth: u32,
}

impl JpxLayerInfo {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn codestream_indices(&self) -> &[u16] {
        &self.codestream_indices
    }

    pub const fn horizontal_grid_size(&self) -> u16 {
        self.horizontal_grid_size
    }

    pub const fn vertical_grid_size(&self) -> u16 {
        self.vertical_grid_size
    }

    pub const fn horizontal_offset(&self) -> u32 {
        self.horizontal_offset
    }

    pub const fn vertical_offset(&self) -> u32 {
        self.vertical_offset
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn has_color_group(&self) -> bool {
        self.has_color_group
    }

    pub const fn has_opacity_box(&self) -> bool {
        self.has_opacity_box
    }

    pub const fn nested_metadata_boxes(&self) -> u32 {
        self.nested_metadata_boxes
    }
}

impl JpxCompositionInfo {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn loop_count(&self) -> u8 {
        self.loop_count
    }

    pub const fn instruction_sets(&self) -> u32 {
        self.instruction_sets
    }

    pub fn frames(&self) -> &[JpxFrameInfo] {
        &self.frames
    }

    pub const fn total_duration_milliseconds(&self) -> u64 {
        self.total_duration_milliseconds
    }

    pub const fn has_unknown_boxes(&self) -> bool {
        self.has_unknown_boxes
    }
}

impl JpxFrameInfo {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn duration_milliseconds(&self) -> u32 {
        self.duration_milliseconds
    }

    pub const fn start_milliseconds(&self) -> u64 {
        self.start_milliseconds
    }

    pub const fn persistent(&self) -> bool {
        self.persistent
    }

    pub fn instructions(&self) -> &[JpxInstructionInfo] {
        &self.instructions
    }
}

impl JpxInstructionInfo {
    pub const fn layer_index(&self) -> usize {
        self.layer_index
    }

    pub const fn target_x(&self) -> u32 {
        self.target_x
    }

    pub const fn target_y(&self) -> u32 {
        self.target_y
    }

    pub const fn target_width(&self) -> u32 {
        self.target_width
    }

    pub const fn target_height(&self) -> u32 {
        self.target_height
    }

    pub const fn source_region(&self) -> (u32, u32, u32, u32) {
        (
            self.source_x,
            self.source_y,
            self.source_width,
            self.source_height,
        )
    }

    pub const fn persistent(&self) -> bool {
        self.persistent
    }

    pub const fn next_reuse(&self) -> u32 {
        self.next_reuse
    }
}

impl JpxInspection {
    pub fn layers(&self) -> &[JpxLayerInfo] {
        &self.layers
    }

    pub fn layer(&self, index: usize) -> Result<&JpxLayerInfo, DecodeError> {
        self.layers
            .get(index)
            .ok_or(DecodeError::LayerIndexOutOfRange {
                index,
                layers: self.layers.len(),
            })
    }

    pub const fn composition(&self) -> Option<&JpxCompositionInfo> {
        self.composition.as_ref()
    }

    pub const fn metadata_boxes(&self) -> u32 {
        self.metadata_boxes
    }

    pub const fn metadata_depth(&self) -> u32 {
        self.metadata_depth
    }
}

pub fn inspect_jpx_layers(
    input: &[u8],
    limits: &ResourceLimits,
) -> Result<JpxInspection, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, limits)?;
    ensure_jpx(inspected.file())?;
    inspect_file(inspected.file(), inspected.codestreams(), limits)
}

fn inspect_file(
    file: &Jp2File<'_>,
    codestreams: &[crate::codestream::CodestreamInfo],
    limits: &ResourceLimits,
) -> Result<JpxInspection, DecodeError> {
    let layer_count = effective_layer_count(file);
    let mut layers = Vec::with_capacity(layer_count);
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= layer_count,
            decreases layer_count - index,
        )
    )]
    while index < layer_count {
        let mut codestream_indices = Vec::new();
        if file.compositing_layers().is_empty() {
            let source = u16::try_from(index).map_err(|_| arithmetic_overflow())?;
            codestream_indices.push(source);
            let codestream = codestreams.get(index).ok_or_else(invalid_layer)?;
            layers.push(JpxLayerInfo {
                index,
                codestream_indices,
                horizontal_grid_size: 1,
                vertical_grid_size: 1,
                horizontal_offset: 0,
                vertical_offset: 0,
                width: codestream.image_size.width,
                height: codestream.image_size.height,
                has_color_group: false,
                has_opacity_box: false,
                nested_metadata_boxes: 0,
            });
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            continue;
        }
        let layer = file.compositing_layer(index)?;
        let (horizontal_offset, vertical_offset, width, height) =
            if layer.registrations().is_empty() {
                let source = u16::try_from(index).map_err(|_| arithmetic_overflow())?;
                codestream_indices.push(source);
                let codestream = codestreams.get(index).ok_or_else(invalid_layer)?;
                (
                    0,
                    0,
                    codestream.image_size.width,
                    codestream.image_size.height,
                )
            } else {
                let mut registration_index = 0_usize;
                let registration_count = layer.registrations().len();
                let mut width = u32::MAX;
                let mut height = u32::MAX;
                #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant registration_index <= registration_count,
                    decreases registration_count - registration_index,
                )
            )]
                while registration_index < registration_count {
                    let registration = layer
                        .registrations()
                        .get(registration_index)
                        .ok_or_else(arithmetic_overflow)?;
                    codestream_indices.push(registration.codestream_index);
                    let codestream = codestreams
                        .get(usize::from(registration.codestream_index))
                        .ok_or_else(invalid_layer)?;
                    width = width.min(registered_extent(
                        codestream.image_size.width,
                        u32::from(registration.horizontal_subsampling),
                        u32::from(layer.horizontal_grid_size()),
                        registration.horizontal_offset,
                    )?);
                    height = height.min(registered_extent(
                        codestream.image_size.height,
                        u32::from(registration.vertical_subsampling),
                        u32::from(layer.vertical_grid_size()),
                        registration.vertical_offset,
                    )?);
                    registration_index = registration_index
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?;
                }
                (0, 0, width, height)
            };
        layers.push(JpxLayerInfo {
            index,
            codestream_indices,
            horizontal_grid_size: layer.horizontal_grid_size(),
            vertical_grid_size: layer.vertical_grid_size(),
            horizontal_offset,
            vertical_offset,
            width,
            height,
            has_color_group: layer.has_color_group(),
            has_opacity_box: layer.has_opacity_box(),
            nested_metadata_boxes: layer.nested_metadata_boxes(),
        });
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let composition = match file.composition() {
        Some(composition) => {
            let frames = build_timeline(
                composition,
                effective_layer_count(file),
                limits.maximum_frames(),
            )?;
            let mut total_duration_milliseconds = 0_u64;
            let mut frame_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant frame_index <= frames.len(),
                    decreases frames.len() - frame_index,
                )
            )]
            while frame_index < frames.len() {
                total_duration_milliseconds = total_duration_milliseconds
                    .checked_add(u64::from(
                        frames
                            .get(frame_index)
                            .ok_or_else(arithmetic_overflow)?
                            .duration_milliseconds,
                    ))
                    .ok_or_else(arithmetic_overflow)?;
                frame_index = frame_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Some(JpxCompositionInfo {
                width: composition.width,
                height: composition.height,
                loop_count: composition.loop_count,
                instruction_sets: u32::try_from(composition.instruction_sets.len())
                    .map_err(|_| arithmetic_overflow())?,
                frames,
                total_duration_milliseconds,
                has_unknown_boxes: composition.has_unknown_boxes,
            })
        }
        None => None,
    };
    Ok(JpxInspection {
        layers,
        composition,
        metadata_boxes: file.metadata_boxes(),
        metadata_depth: file.metadata_depth(),
    })
}

fn registered_extent(
    samples: u32,
    subsampling: u32,
    grid_size: u32,
    offset: u32,
) -> Result<u32, DecodeError> {
    if grid_size == 0 {
        return Err(invalid_layer());
    }
    let numerator = u64::from(samples)
        .checked_mul(u64::from(subsampling))
        .and_then(|value| value.checked_add(u64::from(offset)))
        .ok_or_else(arithmetic_overflow)?;
    let grid = u64::from(grid_size);
    if numerator > u64::MAX - (grid - 1) {
        return Err(arithmetic_overflow());
    }
    u32::try_from(crate::refinement::registered_extent(numerator, grid))
        .map_err(|_| arithmetic_overflow())
}

struct TimelineState {
    frames: Vec<JpxFrameInfo>,
    assignments: Vec<Option<usize>>,
    next_layer: usize,
    instruction_index: usize,
    milliseconds: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn timeline_instruction_view(instruction: CompositionInstruction) -> crate::specs::Part2TimelineInstruction {
        ((0, instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
            instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
            instruction.persistent, instruction.next_reuse), instruction.life)
    }

    #[verifier::inline]
    spec fn timeline_instruction_views(instructions: Seq<CompositionInstruction>) -> Seq<crate::specs::Part2TimelineInstruction> {
        Seq::new(instructions.len(), |i: int| timeline_instruction_view(instructions[i]))
    }

    #[verifier::inline]
    spec fn timeline_set_views(composition: &jp2::Composition) -> Seq<crate::specs::Part2TimelineSet> {
        Seq::new(composition.instruction_sets.len() as nat, |i: int|
            (composition.instruction_sets@[i].repeat, timeline_instruction_views(composition.instruction_sets@[i].instructions@)))
    }

    #[verifier::inline]
    spec fn timeline_state_view(state: &TimelineState) -> crate::specs::Part2TimelineState {
        (frame_views(state.frames@), state.assignments@, state.next_layer, state.instruction_index, state.milliseconds)
    }

    #[verifier::inline]
    spec fn timeline_step_result(result: Result<bool, DecodeError>, state: &TimelineState) -> crate::specs::Part2TimelineExecution {
        match result { Err(error) => Err(error), Ok(active) => Ok((active, timeline_state_view(state))) }
    }

    #[verifier::inline]
    spec fn timeline_frames_result(result: Result<Vec<JpxFrameInfo>, DecodeError>) -> Result<Seq<crate::specs::Part2Frame>, DecodeError> {
        match result { Err(error) => Err(error), Ok(frames) => Ok(frame_views(frames@)) }
    }

    #[verifier::inline]
    spec fn timeline_frame_result(result: Result<(), DecodeError>, frames: Seq<JpxFrameInfo>, time: u64)
        -> Result<(Seq<crate::specs::Part2Frame>, u64), DecodeError>
    {
        match result { Err(error) => Err(error), Ok(()) => Ok((frame_views(frames), time)) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_timeline_limit(layer_count, maximum_frames),
    )
)]
fn timeline_instruction_limit(
    layer_count: usize,
    maximum_frames: u32,
) -> Result<usize, DecodeError> {
    let frames = usize::try_from(maximum_frames).map_err(|_| arithmetic_overflow())?;
    let layers = if layer_count == 0 { 1 } else { layer_count };
    #[cfg(verus_keep_ghost)]
    proof! { vstd::arithmetic::mul::lemma_mul_is_commutative(frames as int, layers as int); }
    match frames.checked_mul(layers) {
        Some(limit) => Ok(limit),
        None => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures timeline_frames_result(result) == crate::specs::part2_build_timeline(
            timeline_set_views(composition), layer_count, maximum_frames),
    )
)]
fn build_timeline(
    composition: &jp2::Composition,
    layer_count: usize,
    maximum_frames: u32,
) -> Result<Vec<JpxFrameInfo>, DecodeError> {
    let maximum_instructions = timeline_instruction_limit(layer_count, maximum_frames)?;
    let mut state = TimelineState {
        frames: Vec::new(),
        assignments: Vec::new(),
        next_layer: 0,
        instruction_index: 0,
        milliseconds: 0,
    };
    let mut set_index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = timeline_set_views(composition);
        let ghost initial = Ok::<(bool, crate::specs::Part2TimelineState), DecodeError>((true, timeline_state_view(&state)));
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(frame_views(state.frames@) =~= Seq::<crate::specs::Part2Frame>::empty());
        crate::proofs::production_impl::production_timeline_set_base(input, initial, layer_count, maximum_frames, maximum_instructions);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant set_index <= composition.instruction_sets.len(),
                input == timeline_set_views(composition),
                initial == Ok((true, (Seq::empty(), Seq::empty(), 0, 0, 0))),
                crate::specs::part2_timeline_limit(layer_count, maximum_frames) == Ok(maximum_instructions),
                crate::proofs::internal_specs::part2_timeline_state_valid(timeline_state_view(&state), layer_count, maximum_frames, maximum_instructions),
                crate::specs::part2_timeline_sets(input, initial, set_index as nat, layer_count, maximum_frames, maximum_instructions)
                    == Ok((true, timeline_state_view(&state))),
            decreases composition.instruction_sets.len() - set_index,
        )
    )]
    while set_index < composition.instruction_sets.len() {
        let set = composition
            .instruction_sets
            .get(set_index)
            .ok_or_else(arithmetic_overflow)?;
        let repetitions = u32::from(set.repeat)
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        let mut repetition = 0_u32;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost set_initial = Ok::<(bool, crate::specs::Part2TimelineState), DecodeError>((true, timeline_state_view(&state)));
            let ghost program = timeline_instruction_views(set.instructions@);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_timeline_repetition_base(program, set_initial, layer_count, maximum_frames, maximum_instructions);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant repetition <= repetitions,
                    input == timeline_set_views(composition), set_index < input.len(),
                    initial == Ok((true, (Seq::empty(), Seq::empty(), 0, 0, 0))),
                    set == composition.instruction_sets@[set_index as int],
                    program == input[set_index as int].1,
                    repetitions as nat == input[set_index as int].0 as nat + 1,
                    crate::specs::part2_timeline_limit(layer_count, maximum_frames) == Ok(maximum_instructions),
                    crate::proofs::internal_specs::part2_timeline_state_valid(timeline_state_view(&state), layer_count, maximum_frames, maximum_instructions),
                    set_initial == crate::specs::part2_timeline_sets(input, initial, set_index as nat, layer_count, maximum_frames, maximum_instructions),
                    crate::specs::part2_timeline_repetitions(program, set_initial, repetition as nat, layer_count, maximum_frames, maximum_instructions)
                        == Ok((true, timeline_state_view(&state))),
                decreases repetitions - repetition,
            )
        )]
        while repetition < repetitions {
            let mut local_index = 0_usize;
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost repeat_initial = Ok::<(bool, crate::specs::Part2TimelineState), DecodeError>((true, timeline_state_view(&state)));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_timeline_instruction_base(program, repeat_initial, layer_count, maximum_frames, maximum_instructions);
            }
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant local_index <= set.instructions.len(),
                        input == timeline_set_views(composition), set_index < input.len(),
                        initial == Ok((true, (Seq::empty(), Seq::empty(), 0, 0, 0))),
                        set == composition.instruction_sets@[set_index as int],
                        program == input[set_index as int].1, program == timeline_instruction_views(set.instructions@),
                        repetitions as nat == input[set_index as int].0 as nat + 1,
                        repetition < repetitions,
                        crate::specs::part2_timeline_limit(layer_count, maximum_frames) == Ok(maximum_instructions),
                        crate::proofs::internal_specs::part2_timeline_state_valid(timeline_state_view(&state), layer_count, maximum_frames, maximum_instructions),
                        set_initial == crate::specs::part2_timeline_sets(input, initial, set_index as nat, layer_count, maximum_frames, maximum_instructions),
                        repeat_initial == crate::specs::part2_timeline_repetitions(program, set_initial, repetition as nat, layer_count, maximum_frames, maximum_instructions),
                        crate::specs::part2_timeline_instructions(program, repeat_initial, local_index as nat, layer_count, maximum_frames, maximum_instructions)
                            == Ok((true, timeline_state_view(&state))),
                    decreases set.instructions.len() - local_index,
                )
            )]
            while local_index < set.instructions.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_timeline_instruction_step(program, repeat_initial, local_index as nat, layer_count, maximum_frames, maximum_instructions);
                }
                let outcome = apply_timeline_instruction(
                    &mut state,
                    set.instructions[local_index],
                    layer_count,
                    maximum_frames,
                    maximum_instructions,
                );
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(crate::specs::part2_timeline_instructions(program, repeat_initial, local_index as nat + 1, layer_count, maximum_frames, maximum_instructions)
                        == timeline_step_result(outcome, &state));
                    if crate::specs::part2_timeline_finished(timeline_step_result(outcome, &state)) {
                        crate::proofs::production_impl::production_timeline_instruction_halt(program, repeat_initial, local_index as nat + 1, program.len(), layer_count, maximum_frames, maximum_instructions);
                        crate::proofs::production_impl::production_timeline_repetition_step(program, set_initial, repetition as nat, layer_count, maximum_frames, maximum_instructions);
                        crate::proofs::production_impl::production_timeline_repetition_halt(program, set_initial, repetition as nat + 1, repetitions as nat, layer_count, maximum_frames, maximum_instructions);
                        crate::proofs::production_impl::production_timeline_set_step(input, initial, set_index as nat, layer_count, maximum_frames, maximum_instructions);
                        crate::proofs::production_impl::production_timeline_set_halt(input, initial, set_index as nat + 1, input.len(), layer_count, maximum_frames, maximum_instructions);
                    }
                }
                let active = outcome?;
                if !active {
                    return Ok(state.frames);
                }
                local_index = local_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_timeline_repetition_step(program, set_initial, repetition as nat, layer_count, maximum_frames, maximum_instructions);
            }
            repetition = repetition.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_timeline_set_step(input, initial, set_index as nat, layer_count, maximum_frames, maximum_instructions);
        }
        set_index = set_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(state.frames)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires crate::proofs::internal_specs::part2_timeline_state_valid(timeline_state_view(old(state)), layer_count, maximum_frames, maximum_instructions),
        ensures timeline_step_result(result, final(state)) == crate::specs::part2_timeline_step(
            timeline_state_view(old(state)), timeline_instruction_view(instruction), layer_count, maximum_frames, maximum_instructions),
            result.is_ok() ==> crate::proofs::internal_specs::part2_timeline_state_valid(timeline_state_view(final(state)), layer_count, maximum_frames, maximum_instructions),
    )
)]
fn apply_timeline_instruction(
    state: &mut TimelineState,
    instruction: CompositionInstruction,
    layer_count: usize,
    maximum_frames: u32,
    maximum_instructions: usize,
) -> Result<bool, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before = timeline_state_view(state); }
    if state.instruction_index >= maximum_instructions {
        let actual = (state.instruction_index as u64)
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        return Err(DecodeError::LimitExceeded {
            resource: Resource::WorkUnits,
            limit: maximum_instructions as u64,
            actual,
        });
    }
    if state.instruction_index >= state.assignments.len() {
        state.assignments.push(None);
    }
    let assigned = match state.assignments.get(state.instruction_index) {
        Some(value) => *value,
        None => None,
    };
    let layer_index = match assigned {
        Some(index) => index,
        None => {
            if state.next_layer >= layer_count {
                return Ok(false);
            }
            let layer = state.next_layer;
            state.next_layer = state
                .next_layer
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            layer
        }
    };
    append_timeline_frame(
        &mut state.frames,
        &mut state.milliseconds,
        instruction_info(layer_index, instruction),
        instruction.life,
        maximum_frames,
    )?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost assignments = state.assignments@; }
    if instruction.next_reuse != 0 {
        let reuse = usize::try_from(instruction.next_reuse).map_err(|_| arithmetic_overflow())?;
        let target = state
            .instruction_index
            .checked_add(reuse)
            .ok_or_else(arithmetic_overflow)?;
        if target >= maximum_instructions {
            let actual = (target as u64)
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: maximum_instructions as u64,
                actual,
            });
        }
        let mut assignment_count = state.assignments.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant target < maximum_instructions,
                    assignment_count == state.assignments.len(),
                    state.instruction_index == before.3, state.next_layer <= layer_count,
                    before.3 < maximum_instructions,
                    assignments == if before.3 as nat >= before.1.len() { before.1.push(None) } else { before.1 },
                    (before.3 as nat) < assignments.len(),
                    assigned == assignments[before.3 as int],
                    assigned.is_some() ==> layer_index == assigned.unwrap() && state.next_layer == before.2,
                    assigned.is_none() ==> layer_index == before.2 && (state.next_layer as nat) == before.2 as nat + 1 && before.2 < layer_count,
                    crate::specs::part2_timeline_frame(before.0, before.4,
                        crate::specs::part2_timeline_assign(layer_index, timeline_instruction_view(instruction)), instruction.life, maximum_frames)
                        == Ok((frame_views(state.frames@), state.milliseconds)),
                    state.frames.len() <= maximum_frames,
                    assignments.len() <= state.assignments.len() <= maximum_instructions,
                    state.assignments.len() <= if assignments.len() > target as nat { assignments.len() } else { target as nat + 1 },
                    state.assignments@ == assignments + Seq::new((state.assignments.len() - assignments.len()) as nat, |i: int| None),
                decreases target + 1 - assignment_count,
            )
        )]
        while assignment_count <= target {
            state.assignments.push(None);
            assignment_count += 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(state.assignments@ =~= assignments + Seq::new((state.assignments.len() - assignments.len()) as nat, |i: int| None));
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
                if (target as nat) < assignments.len() {
                    assert(state.assignments.len() == assignments.len());
                    assert(Seq::<Option<usize>>::new(0, |i: int| None) =~= Seq::empty());
                    assert(assignments + Seq::<Option<usize>>::empty() =~= assignments);
                } else {
                    assert(assignment_count > target);
                    assert(state.assignments.len() <= target as nat + 1);
                    assert(state.assignments.len() == target as nat + 1);
                }
                assert(state.assignments@ =~= if (target as nat) < assignments.len() { assignments } else {
                    assignments + Seq::new((target as nat + 1 - assignments.len()) as nat, |i: int| None)
                });
        }
        let slot = state
            .assignments
            .get_mut(target)
            .ok_or_else(arithmetic_overflow)?;
        let conflicting = match slot {
            Some(assigned) => *assigned != layer_index,
            None => false,
        };
        if conflicting {
            return Err(invalid_layer());
        }
        *slot = Some(layer_index);
    }
    state.instruction_index = state
        .instruction_index
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        let goal = crate::specs::part2_timeline_step(before, timeline_instruction_view(instruction), layer_count, maximum_frames, maximum_instructions);
        assert(goal.is_ok());
        assert(goal.ok().unwrap().1.0 == frame_views(state.frames@));
        assert(goal.ok().unwrap().1.1 == state.assignments@);
        assert(goal.ok().unwrap().1.2 == state.next_layer);
        assert(goal.ok().unwrap().1.3 == state.instruction_index);
        assert(goal.ok().unwrap().1.4 == state.milliseconds);
    }
    Ok(true)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires old(frames).len() <= maximum_frames,
        ensures timeline_frame_result(result, final(frames)@, *final(time))
            == crate::specs::part2_timeline_frame(frame_views(old(frames)@), *old(time), instruction_view(instruction), life, maximum_frames),
            result.is_ok() ==> final(frames).len() <= maximum_frames,
    )
)]
fn append_timeline_frame(
    frames: &mut Vec<JpxFrameInfo>,
    time: &mut u64,
    instruction: JpxInstructionInfo,
    life: u32,
    maximum_frames: u32,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost before = frame_views(frames@);
        let ghost old_time = *time;
    }
    let fresh = match frames.last() {
        Some(frame) => frame.duration_milliseconds != 0,
        None => true,
    };
    if fresh {
        if frames.len() as u64 >= u64::from(maximum_frames) {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Frames,
                limit: u64::from(maximum_frames),
                actual: frames.len() as u64 + 1,
            });
        }
        let next = JpxFrameInfo {
            index: frames.len(),
            start_milliseconds: *time,
            duration_milliseconds: 0,
            persistent: true,
            instructions: Vec::new(),
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            frame_views_push(frames@, next);
            assert(instruction_views(next.instructions@) =~= Seq::<crate::specs::Part2Instruction>::empty());
        }
        frames.push(next);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(frame_views(frames@) =~= before.push((before.len() as usize, old_time, 0, true, Seq::empty())));
        }
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost prior = frame_views(frames@); }
    let frame = frames.last_mut().ok_or_else(arithmetic_overflow)?;
    frame.duration_milliseconds = life;
    frame.persistent = instruction.persistent;
    #[cfg(verus_keep_ghost)]
    proof! { instruction_views_push(frame.instructions@, instruction); }
    frame.instructions.push(instruction);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(frame_views(frames@) =~= prior.update(prior.len() as int - 1,
            (prior.last().0, prior.last().1, life, instruction.persistent, prior.last().4.push(instruction_view(instruction)))));
    }
    if life != 0 {
        *time = time
            .checked_add(u64::from(life))
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        if fresh {
            assert(frame_views(frames@) =~= before.push((before.len() as usize, old_time, life, instruction.persistent, seq![instruction_view(instruction)])));
        }
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures instruction_view(result) == crate::specs::part2_timeline_assign(layer_index, timeline_instruction_view(instruction)),
    )
)]
fn instruction_info(layer_index: usize, instruction: CompositionInstruction) -> JpxInstructionInfo {
    JpxInstructionInfo {
        layer_index,
        target_x: instruction.target_x,
        target_y: instruction.target_y,
        target_width: instruction.target_width,
        target_height: instruction.target_height,
        source_x: instruction.source_x,
        source_y: instruction.source_y,
        source_width: instruction.source_width,
        source_height: instruction.source_height,
        persistent: instruction.persistent,
        next_reuse: instruction.next_reuse,
    }
}

/// Renders one layer while retaining its straight/premultiplied sample representation.
/// The alpha channel records that representation; frame/composition output is straight alpha.
pub fn render_jpx_layer(
    input: &[u8],
    layer_index: usize,
    options: &DecodeOptions,
) -> Result<ConvertedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    ensure_jpx(inspected.file())?;
    decode_effective_layer(inspected.file(), layer_index, options)?.into_image()
}

pub fn render_jpx_composition(
    input: &[u8],
    options: &DecodeOptions,
) -> Result<ConvertedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    let file = inspected.file();
    ensure_jpx(file)?;
    let metadata = inspect_file(file, inspected.codestreams(), options.limits())?;
    let has_frames = match metadata.composition() {
        Some(composition) => !composition.frames().is_empty(),
        None => false,
    };
    if has_frames {
        return render_inspected_frame(file, &metadata, 0, options);
    }
    let (width, height) = composition_dimensions(&metadata, options.limits())?;
    let mut layers = Vec::new();
    let layer_count = effective_layer_count(file);
    let mut layer_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant layer_index <= layer_count,
            decreases layer_count - layer_index,
        )
    )]
    while layer_index < layer_count {
        layers.push((
            decode_effective_layer(file, layer_index, options)?,
            JpxInstructionInfo {
                layer_index,
                target_x: 0,
                target_y: 0,
                target_width: metadata.layer(layer_index)?.width(),
                target_height: metadata.layer(layer_index)?.height(),
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                persistent: true,
                next_reuse: 0,
            },
        ));
        layer_index = layer_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    render_decoded_layers(
        width,
        height,
        &layers,
        options.limits().maximum_decoded_bytes(),
        options.limits(),
    )
}

pub fn render_jpx_frame(
    input: &[u8],
    frame_index: usize,
    options: &DecodeOptions,
) -> Result<ConvertedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    let file = inspected.file();
    ensure_jpx(file)?;
    let metadata = inspect_file(file, inspected.codestreams(), options.limits())?;
    render_inspected_frame(file, &metadata, frame_index, options)
}

fn render_inspected_frame(
    file: &Jp2File<'_>,
    metadata: &JpxInspection,
    requested_frame: usize,
    options: &DecodeOptions,
) -> Result<ConvertedImage, DecodeError> {
    let composition = metadata
        .composition()
        .ok_or(DecodeError::FrameIndexOutOfRange {
            index: requested_frame,
            frames: 0,
        })?;
    let instructions = select_frame_instructions(
        composition.frames(),
        composition.loop_count(),
        requested_frame,
    )?;
    let mut supplied = Vec::new();
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= instructions.len(),
            decreases instructions.len() - index,
        )
    )]
    while index < instructions.len() {
        let instruction = instructions[index];
        let outcome = decode_effective_layer(file, instruction.layer_index(), options);
        let failed = outcome.is_err();
        supplied.push((instruction.layer_index, Some(outcome)));
        if failed {
            break;
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    render_normalized_frame(
        composition.width(),
        composition.height(),
        composition.frames(),
        composition.loop_count(),
        requested_frame,
        supplied,
        options.limits().maximum_decoded_bytes(),
        options.limits(),
    )
}

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn instruction_view(instruction: JpxInstructionInfo) -> crate::specs::Part2Instruction {
        (instruction.layer_index, instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
            instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
            instruction.persistent, instruction.next_reuse)
    }

    #[verifier::inline]
    spec fn instruction_views(instructions: Seq<JpxInstructionInfo>) -> Seq<crate::specs::Part2Instruction> {
        Seq::new(instructions.len(), |i: int| instruction_view(instructions[i]))
    }

    proof fn instruction_views_push(instructions: Seq<JpxInstructionInfo>, instruction: JpxInstructionInfo)
        ensures instruction_views(instructions.push(instruction)) == instruction_views(instructions).push(instruction_view(instruction)),
    {
        assert(instruction_views(instructions.push(instruction)) =~= instruction_views(instructions).push(instruction_view(instruction)));
    }

    #[verifier::inline]
    spec fn frame_views(frames: Seq<JpxFrameInfo>) -> Seq<crate::specs::Part2Frame> {
        Seq::new(frames.len(), |i: int| (frames[i].index, frames[i].start_milliseconds,
            frames[i].duration_milliseconds, frames[i].persistent, instruction_views(frames[i].instructions@)))
    }

    proof fn frame_views_push(frames: Seq<JpxFrameInfo>, frame: JpxFrameInfo)
        ensures frame_views(frames.push(frame)) == frame_views(frames).push(
            (frame.index, frame.start_milliseconds, frame.duration_milliseconds, frame.persistent, instruction_views(frame.instructions@))),
    {
        assert(frame_views(frames.push(frame)) =~= frame_views(frames).push(
            (frame.index, frame.start_milliseconds, frame.duration_milliseconds, frame.persistent, instruction_views(frame.instructions@))));
    }

    #[verifier::inline]
    spec fn frame_selection_result(result: Result<Vec<JpxInstructionInfo>, DecodeError>)
        -> Result<Seq<crate::specs::Part2Instruction>, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(instructions) => Ok(instruction_views(instructions@)) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures frame_selection_result(result) == crate::specs::part2_select_frame(frame_views(frames@), requested_frame, loop_count),
    )
)]
fn select_frame_instructions(
    frames: &[JpxFrameInfo],
    loop_count: u8,
    requested_frame: usize,
) -> Result<Vec<JpxInstructionInfo>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = frame_views(frames@); }
    if frames.is_empty() {
        return Err(DecodeError::FrameIndexOutOfRange {
            index: requested_frame,
            frames: 0,
        });
    }
    let frame_index = if loop_count == 0 {
        crate::refinement::loop_frame(requested_frame, frames.len())
    } else {
        let total = frames
            .len()
            .checked_mul(usize::from(loop_count))
            .ok_or_else(arithmetic_overflow)?;
        if requested_frame >= total {
            return Err(DecodeError::FrameIndexOutOfRange {
                index: requested_frame,
                frames: total,
            });
        }
        crate::refinement::loop_frame(requested_frame, frames.len())
    };
    let mut placements = Vec::new();
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::specs::part2_frame_index(requested_frame, input.len(), loop_count) == Ok(frame_index));
        crate::proofs::production_impl::production_frame_sequence_empty(input, frame_index as nat);
        assert(instruction_views(placements@) =~= Seq::<crate::specs::Part2Instruction>::empty());
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= frame_index + 1,
                frame_index < frames.len(),
                input == frame_views(frames@),
                crate::specs::part2_frame_index(requested_frame, input.len(), loop_count) == Ok(frame_index),
                instruction_views(placements@) == crate::specs::part2_frame_instruction_sequence(input, frame_index as nat, index as nat),
            decreases frame_index + 1 - index,
        )
    )]
    while index <= frame_index {
        let frame = frames.get(index).ok_or_else(arithmetic_overflow)?;
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_frame_sequence_step(input, frame_index as nat, index as nat); }
        if index == frame_index || frame.persistent {
            let mut instruction_index = 0_usize;
            let instruction_count = frame.instructions.len();
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost prior = instruction_views(placements@); }
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant instruction_index <= instruction_count,
                        index <= frame_index < frames.len(),
                        input == frame_views(frames@),
                        frame == frames@[index as int],
                        instruction_count == frame.instructions.len(),
                        index == frame_index || frame.persistent,
                        prior == crate::specs::part2_frame_instruction_sequence(input, frame_index as nat, index as nat),
                        instruction_views(placements@) == prior + instruction_views(frame.instructions@).take(instruction_index as int),
                    decreases instruction_count - instruction_index,
                )
            )]
            while instruction_index < instruction_count {
                let instruction = *frame
                    .instructions
                    .get(instruction_index)
                    .ok_or_else(arithmetic_overflow)?;
                #[cfg(verus_keep_ghost)]
                proof! {
                    instruction_views_push(placements@, instruction);
                    assert(instruction_views(frame.instructions@).take(instruction_index as int + 1)
                        =~= instruction_views(frame.instructions@).take(instruction_index as int).push(instruction_view(instruction)));
                }
                placements.push(instruction);
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(instruction_views(placements@) =~= prior + instruction_views(frame.instructions@).take(instruction_index as int + 1));
                }
                instruction_index = instruction_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(placements)
}

fn ensure_jpx(file: &Jp2File<'_>) -> Result<(), DecodeError> {
    if file.file_type().is_jpx_compatible() {
        Ok(())
    } else {
        Err(DecodeError::malformed(0, MalformedKind::InvalidProfile))
    }
}

fn effective_layer_count(file: &Jp2File<'_>) -> usize {
    crate::refinement::effective_layer_count(
        file.compositing_layers().len(),
        file.codestream_count(),
    )
}

fn decode_effective_layer(
    file: &Jp2File<'_>,
    layer_index: usize,
    options: &DecodeOptions,
) -> Result<OwnedLayer, DecodeError> {
    if file.compositing_layers().is_empty() {
        if layer_index >= file.codestream_count() {
            return Err(DecodeError::LayerIndexOutOfRange {
                index: layer_index,
                layers: file.codestream_count(),
            });
        }
        return owned_layer_from_image(&decode_layer(file, layer_index, None, options)?);
    }
    let layer = file.compositing_layer(layer_index)?;
    decode_compositing_layer(file, layer, layer_index, options)
}

fn decode_layer(
    file: &Jp2File<'_>,
    codestream_index: usize,
    color_override: Option<&jp2::ColorSpecification>,
    options: &DecodeOptions,
) -> Result<ConvertedImage, DecodeError> {
    let (decoded, header) = crate::decode::decode_jpx_file(file, codestream_index, options)?;
    let header = header.unwrap_or(file.header());
    crate::color::convert_jp2_color(
        &decoded,
        color_override.unwrap_or(&header.color),
        options.limits(),
    )
}

struct OwnedLayer {
    width: u32,
    height: u32,
    color_space: ConvertedColorSpace,
    colors: Vec<Vec<u16>>,
    alpha: Option<(Vec<u16>, bool)>,
}

type SuppliedLayer = (usize, Option<Result<OwnedLayer, DecodeError>>);

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn layer_borrowed_planes(colors: Seq<&[u16]>) -> Seq<Seq<u16>> {
        Seq::new(colors.len(), |i: int| colors[i]@)
    }

    proof fn layer_borrowed_planes_push(colors: Seq<&[u16]>, samples: &[u16])
        ensures layer_borrowed_planes(colors.push(samples)) == layer_borrowed_planes(colors).push(samples@),
    {
        assert(layer_borrowed_planes(colors.push(samples)) =~= layer_borrowed_planes(colors).push(samples@));
    }

    proof fn layer_owned_planes_push(colors: Seq<Vec<u16>>, samples: Vec<u16>)
        ensures crate::specs::part2_composition_planes(colors.push(samples))
            == crate::specs::part2_composition_planes(colors).push(samples@),
    {
        assert(crate::specs::part2_composition_planes(colors.push(samples))
            =~= crate::specs::part2_composition_planes(colors).push(samples@));
    }

    #[verifier::inline]
    spec fn layer_color_slots(colors: Seq<Option<&[u16]>>) -> Seq<Option<Seq<u16>>> {
        Seq::new(colors.len(), |i: int| match colors[i] {
            Some(samples) => Some(samples@),
            None => None,
        })
    }

    #[verifier::inline]
    spec fn layer_alpha_planes(alphas: Seq<(&[u16], bool)>) -> Seq<crate::specs::Part2LayerAlpha> {
        Seq::new(alphas.len(), |i: int| (alphas[i].0@, alphas[i].1))
    }

    #[verifier::inline]
    spec fn layer_owned_alpha(alpha: Option<(Vec<u16>, bool)>) -> Option<crate::specs::Part2LayerAlpha> {
        match alpha {
            Some((samples, premultiplied)) => Some((samples@, premultiplied)),
            None => None,
        }
    }

    #[verifier::inline]
    spec fn owned_layer_view(layer: &OwnedLayer) -> crate::specs::Part2OwnedLayer {
        (layer.width, layer.height, layer.color_space,
            crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha))
    }

    #[verifier::inline]
    spec fn owned_layer_outcome(result: Result<OwnedLayer, DecodeError>) -> crate::specs::Part2OwnedLayerResult {
        match result {
            Err(error) => Err(error),
            Ok(layer) => Ok(owned_layer_view(&layer)),
        }
    }

    #[verifier::inline]
    spec fn supplied_views(supplied: Seq<SuppliedLayer>) -> Seq<crate::specs::Part2SuppliedLayer> {
        Seq::new(supplied.len(), |i: int| (supplied[i].0, match supplied[i].1 {
            None => None, Some(outcome) => Some(owned_layer_outcome(outcome)),
        }))
    }

    #[verifier::inline]
    spec fn collected_layers_result(result: Result<Vec<(OwnedLayer, JpxInstructionInfo)>, DecodeError>)
        -> Result<Seq<crate::specs::Part2Placement>, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(placements) => Ok(placement_views(placements@)) }
    }

        #[verifier::inline]
        spec fn placement_view(placement: &(OwnedLayer, JpxInstructionInfo)) -> crate::specs::Part2Placement {
            let layer = &placement.0;
            let instruction = placement.1;
            ((layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@),
                layer_owned_alpha(layer.alpha)),
                (instruction.layer_index, instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                    instruction.persistent, instruction.next_reuse))
        }

        #[verifier::inline]
        spec fn placement_views(placements: Seq<(OwnedLayer, JpxInstructionInfo)>) -> Seq<crate::specs::Part2Placement> {
            Seq::new(placements.len(), |i: int| placement_view(&placements[i]))
    }

    proof fn placement_views_push(placements: Seq<(OwnedLayer, JpxInstructionInfo)>, placement: (OwnedLayer, JpxInstructionInfo))
        ensures placement_views(placements.push(placement)) == placement_views(placements).push(placement_view(&placement)),
    {
        assert(placement_views(placements.push(placement)) =~= placement_views(placements).push(placement_view(&placement)));
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (owned_layer_outcome(result), supplied_views(final(supplied)@))
            == crate::specs::part2_take_supplied_layer(supplied_views(old(supplied)@), reference),
    )
)]
fn take_supplied_layer(
    supplied: &mut [SuppliedLayer],
    reference: usize,
) -> Result<OwnedLayer, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = supplied_views(supplied@); }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= supplied.len(), supplied_views(supplied@) == input,
                input == supplied_views(old(supplied)@),
                crate::specs::part2_supply_search(input, reference, 0) == crate::specs::part2_supply_search(input, reference, index as nat),
            decreases supplied.len() - index,
        )
    )]
    while index < supplied.len() {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_supply_search_step(input, reference, index as nat); }
        if supplied[index].0 == reference && supplied[index].1.is_some() {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(crate::specs::part2_supply_search(input, reference, 0) == Some(index as nat));
            }
            let outcome = supplied[index].1.take().ok_or_else(invalid_layer)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(supplied_views(supplied@) =~= input.update(index as int, (reference, None)));
                assert(owned_layer_outcome(outcome) == input[index as int].1.unwrap());
                assert((owned_layer_outcome(outcome), supplied_views(supplied@))
                    == crate::specs::part2_take_supplied_layer(input, reference));
            }
            let layer = outcome?;
            return Ok(layer);
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_supply_search_step(input, reference, index as nat); }
    Err(invalid_layer())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures collected_layers_result(result) == crate::specs::part2_collect_supplied_layers(
            instruction_views(instructions@), supplied_views(supplied@)),
    )
)]
fn collect_supplied_layers(
    instructions: &[JpxInstructionInfo],
    supplied: Vec<SuppliedLayer>,
) -> Result<Vec<(OwnedLayer, JpxInstructionInfo)>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = instruction_views(instructions@);
        let ghost bank = supplied_views(supplied@);
    }
    let mut available = supplied;
    let mut placements = Vec::new();
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_gather_base(input, bank);
        assert(placement_views(placements@) =~= Seq::<crate::specs::Part2Placement>::empty());
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= instructions.len(), input == instruction_views(instructions@),
                bank == supplied_views(supplied@),
                crate::specs::part2_gather_layers(input, bank, index as nat)
                    == (Ok(()), placement_views(placements@), supplied_views(available@)),
            decreases instructions.len() - index,
        )
    )]
    while index < instructions.len() {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_gather_step(input, bank, index as nat); }
        let instruction = instructions[index];
        let outcome = take_supplied_layer(&mut available, instruction.layer_index);
        #[cfg(verus_keep_ghost)]
        proof! {
            if outcome.is_err() {
                assert(crate::specs::part2_gather_layers(input, bank, index as nat + 1).0 == Err(outcome.err().unwrap()));
                crate::proofs::production_impl::production_gather_error(input, bank, index as nat + 1);
            }
        }
        let layer = outcome?;
        let placement = (layer, instruction);
        #[cfg(verus_keep_ghost)]
        proof! { placement_views_push(placements@, placement); }
        placements.push(placement);
        index += 1;
    }
    Ok(placements)
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::color::converted_image_result(result) == crate::specs::part2_render_normalized_frame(
            width, height, frame_views(frames@), loop_count, requested_frame, supplied_views(supplied@),
            maximum_decoded_bytes, crate::limits::resource_work_limit(limits)),
    )
)]
fn render_normalized_frame(
    width: u32,
    height: u32,
    frames: &[JpxFrameInfo],
    loop_count: u8,
    requested_frame: usize,
    supplied: Vec<SuppliedLayer>,
    maximum_decoded_bytes: u64,
    limits: &ResourceLimits,
) -> Result<ConvertedImage, DecodeError> {
    let instructions = select_frame_instructions(frames, loop_count, requested_frame)?;
    let placements = collect_supplied_layers(&instructions, supplied)?;
    render_decoded_layers(width, height, &placements, maximum_decoded_bytes, limits)
}

#[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures owned_layer_outcome(result) == crate::specs::part2_owned_layer_input(
                crate::color::converted_image_view(image).0, crate::color::converted_image_view(image).1,
                crate::color::converted_image_view(image).2,
                crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
            ),
        )
    )]
fn owned_layer_from_image(image: &ConvertedImage) -> Result<OwnedLayer, DecodeError> {
    let channels = layer_channels(image)?;
    let mut colors = Vec::with_capacity(channels.colors.len());
    let mut index = 0_usize;
    #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= channels.colors.len(),
                    colors.len() == index,
                    layer_channels_outcome(Ok(channels)) == crate::specs::part2_layer_input(
                        crate::color::converted_image_view(image).0, crate::color::converted_image_view(image).1,
                        crate::color::converted_image_view(image).2,
                        crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                    ),
                    crate::specs::part2_composition_planes(colors@)
                        == layer_borrowed_planes(channels.colors@).take(index as int),
                decreases channels.colors.len() - index,
            )
        )]
    while index < channels.colors.len() {
        let samples = *channels.colors.get(index).ok_or_else(invalid_layer)?;
        let copied = copy_samples(samples)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(samples@ == layer_borrowed_planes(channels.colors@)[index as int]);
            assert(copied@ == samples@);
            layer_owned_planes_push(colors@, copied);
        }
        colors.push(copied);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_composition_planes(colors@)
                =~= layer_borrowed_planes(channels.colors@).take(index as int + 1));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::specs::part2_composition_planes(colors@) =~= layer_borrowed_planes(channels.colors@));
    }
    Ok(OwnedLayer {
        width: channels.width,
        height: channels.height,
        color_space: image.color_space(),
        colors,
        alpha: channels.alpha,
    })
}

impl OwnedLayer {
    fn into_image(self) -> Result<ConvertedImage, DecodeError> {
        let pixels = checked_area(self.width, self.height)?;
        let (alpha, premultiplied) = match self.alpha {
            Some(value) => value,
            None => (vec![u16::MAX; pixels], false),
        };
        crate::color::from_layer(
            self.width,
            self.height,
            self.color_space,
            self.colors,
            alpha,
            premultiplied,
        )
    }
}

fn decode_compositing_layer(
    file: &Jp2File<'_>,
    layer: &CompositingLayer<'_>,
    layer_index: usize,
    options: &DecodeOptions,
) -> Result<OwnedLayer, DecodeError> {
    if !layer.channel_definition_records().is_empty()
        || layer.registrations().len() > 1
        || matches!(layer.opacity(), Some(LayerOpacity::ChromaKey { .. }))
    {
        return decode_general_compositing_layer(file, layer, layer_index, options);
    }
    let color_override = layer.color_group().first();
    if layer.registrations().is_empty() {
        let image = decode_layer(file, layer_index, color_override, options)?;
        let mut owned = owned_layer_from_image(&image)?;
        apply_layer_opacity(file, layer, layer_index, &mut owned, None, options)?;
        return Ok(owned);
    }
    let first = layer
        .registrations()
        .first()
        .ok_or_else(arithmetic_overflow)?;
    let image = decode_layer(
        file,
        usize::from(first.codestream_index),
        color_override,
        options,
    )?;
    let owned = owned_layer_from_image(&image)?;
    let mut registered = register_layer(
        &owned,
        first,
        layer.horizontal_grid_size(),
        layer.vertical_grid_size(),
    )?;
    let alpha_registration = layer.registrations().get(1);
    apply_layer_opacity(
        file,
        layer,
        usize::from(first.codestream_index),
        &mut registered,
        alpha_registration,
        options,
    )?;
    Ok(registered)
}

fn decode_general_compositing_layer(
    file: &Jp2File<'_>,
    layer: &CompositingLayer<'_>,
    layer_index: usize,
    options: &DecodeOptions,
) -> Result<OwnedLayer, DecodeError> {
    let color = layer.color_group().first().unwrap_or(&file.header().color);
    let color_count = layer_color_count(color)?;
    let source_count = if layer.registrations().is_empty() {
        1
    } else {
        layer.registrations().len()
    };
    let mut sources = Vec::with_capacity(source_count);
    let mut width = u32::MAX;
    let mut height = u32::MAX;
    let mut source_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant source_index <= source_count,
            decreases source_count - source_index,
        )
    )]
    while source_index < source_count {
        let (
            codestream_index,
            horizontal_subsampling,
            vertical_subsampling,
            horizontal_offset,
            vertical_offset,
        ) = if layer.registrations().is_empty() {
            (layer_index, 1_u8, 1_u8, 0_u32, 0_u32)
        } else {
            let registration = layer
                .registrations()
                .get(source_index)
                .ok_or_else(invalid_layer)?;
            (
                usize::from(registration.codestream_index),
                registration.horizontal_subsampling,
                registration.vertical_subsampling,
                registration.horizontal_offset,
                registration.vertical_offset,
            )
        };
        let (image, _) = crate::decode::decode_jpx_file(file, codestream_index, options)?;
        let registered_width = registered_extent(
            image.width(),
            u32::from(horizontal_subsampling),
            u32::from(layer.horizontal_grid_size()),
            horizontal_offset,
        )?;
        let registered_height = registered_extent(
            image.height(),
            u32::from(vertical_subsampling),
            u32::from(layer.vertical_grid_size()),
            vertical_offset,
        )?;
        width = width.min(registered_width);
        height = height.min(registered_height);
        sources.push((
            image,
            horizontal_subsampling,
            vertical_subsampling,
            horizontal_offset,
            vertical_offset,
        ));
        source_index = source_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    if width == 0 || height == 0 || width == u32::MAX || height == u32::MAX {
        return Err(invalid_layer());
    }
    enforce_limit(
        Resource::Pixels,
        options.limits().maximum_pixels(),
        u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(arithmetic_overflow)?,
    )?;

    let mut components = Vec::new();
    let mut descriptors = Vec::new();
    source_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant source_index <= sources.len(),
            decreases sources.len() - source_index,
        )
    )]
    while source_index < sources.len() {
        let (
            image,
            horizontal_subsampling,
            vertical_subsampling,
            horizontal_offset,
            vertical_offset,
        ) = sources.get(source_index).ok_or_else(invalid_layer)?;
        let mut component_index = 0_usize;
        let component_count = image.components().len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= component_count,
                decreases component_count - component_index,
            )
        )]
        while component_index < component_count {
            let component = image
                .components()
                .get(component_index)
                .ok_or_else(invalid_layer)?;
            let normalized = crate::color::normalize_component(
                component,
                image.width(),
                image.height(),
                image.x_origin(),
                image.y_origin(),
            )?;
            let samples = register_samples(
                &normalized,
                image.width(),
                image.height(),
                width,
                height,
                u32::from(*horizontal_subsampling),
                u32::from(*vertical_subsampling),
                u32::from(layer.horizontal_grid_size()),
                u32::from(layer.vertical_grid_size()),
                *horizontal_offset,
                *vertical_offset,
            )?;
            let mut assembled = TypedComponent::from_parts(
                width,
                height,
                ComponentInfo {
                    precision: 16,
                    signed: false,
                    x_subsampling: 1,
                    y_subsampling: 1,
                    x_registration: 0,
                    y_registration: 0,
                },
                TypedSamples::Unsigned(widen_samples(&samples)?),
            );
            let mut definition_index = 0_usize;
            let definition_count = component.channel_definitions().len();
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant definition_index <= definition_count,
                    decreases definition_count - definition_index,
                )
            )]
            while definition_index < definition_count {
                assembled.add_channel_definition(
                    *component
                        .channel_definitions()
                        .get(definition_index)
                        .ok_or_else(invalid_layer)?,
                );
                definition_index = definition_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }

            fn widen_samples(samples: &[u16]) -> Result<Vec<u64>, DecodeError> {
                let mut widened = Vec::with_capacity(samples.len());
                let mut index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant index <= samples.len(),
                        decreases samples.len() - index,
                    )
                )]
                while index < samples.len() {
                    widened.push(u64::from(*samples.get(index).ok_or_else(invalid_layer)?));
                    index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                Ok(widened)
            }
            descriptors.push((component.info().precision, component.info().signed));
            components.push(assembled);
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        source_index = source_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    if components.len() > usize::from(options.limits().maximum_components()) {
        return Err(DecodeError::limit(
            Resource::Components,
            u64::from(options.limits().maximum_components()),
            components.len() as u64,
        ));
    }

    if !layer.channel_definition_records().is_empty() {
        clear_channel_definitions(&mut components);
        let mut record_index = 0_usize;
        let record_count = layer.channel_definition_records().len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant record_index <= record_count,
                decreases record_count - record_index,
            )
        )]
        while record_index < record_count {
            let record = layer
                .channel_definition_records()
                .get(record_index)
                .ok_or_else(invalid_layer)?;
            validate_layer_definition(record.definition, color_count)?;
            components
                .get_mut(usize::from(record.channel))
                .ok_or_else(invalid_layer)?
                .add_channel_definition(record.definition);
            record_index = record_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
    } else if let Some(opacity) = layer.opacity() {
        clear_channel_definitions(&mut components);
        if components.len() < color_count {
            return Err(invalid_layer());
        }
        let mut color_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant color_index <= color_count,
                decreases color_count - color_index,
            )
        )]
        while color_index < color_count {
            components
                .get_mut(color_index)
                .ok_or_else(invalid_layer)?
                .add_channel_definition(ChannelDefinition::new(
                    ChannelType::Color,
                    ChannelAssociation::Color(
                        u16::try_from(color_index + 1).map_err(|_| arithmetic_overflow())?,
                    ),
                ));
            color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        if matches!(
            opacity,
            LayerOpacity::Straight | LayerOpacity::Premultiplied
        ) {
            let expected_channels = color_count.checked_add(1).ok_or_else(arithmetic_overflow)?;
            if components.len() != expected_channels {
                return Err(invalid_layer());
            }
            components
                .get_mut(color_count)
                .ok_or_else(invalid_layer)?
                .add_channel_definition(ChannelDefinition::new(
                    if matches!(opacity, LayerOpacity::Premultiplied) {
                        ChannelType::PremultipliedOpacity
                    } else {
                        ChannelType::Opacity
                    },
                    ChannelAssociation::WholeImage,
                ));
        }
    }

    let assembled =
        TypedImage::from_components_with_origin(width, height, 0, 0, components, options.limits())?;
    let converted = crate::color::convert_jp2_color(&assembled, color, options.limits())?;
    let mut owned = owned_layer_from_image(&converted)?;
    if let Some(LayerOpacity::ChromaKey { channels, values }) = layer.opacity() {
        if usize::from(*channels) != color_count || descriptors.len() < color_count {
            return Err(invalid_layer());
        }
        let keys = normalized_chroma_keys_for_components(
            values,
            descriptors.get(..color_count).ok_or_else(invalid_layer)?,
        )?;
        apply_chroma_key(&mut owned, &keys)?;
    }
    Ok(owned)
}

fn layer_color_count(color: &jp2::ColorSpecification) -> Result<usize, DecodeError> {
    match color.method() {
        jp2::ColorMethod::Enumerated {
            color_space: jp2::ENUMERATED_GRAYSCALE,
        } => Ok(1),
        jp2::ColorMethod::Enumerated {
            color_space: jp2::ENUMERATED_SRGB | jp2::ENUMERATED_SYCC,
        } => Ok(3),
        jp2::ColorMethod::RestrictedIcc { .. } | jp2::ColorMethod::AnyIcc { .. } => {
            Err(DecodeError::UnsupportedColorConversion {
                reason: crate::UnsupportedColorConversion::IccProfile,
            })
        }
        jp2::ColorMethod::Enumerated { .. } => Err(DecodeError::UnsupportedColorConversion {
            reason: crate::UnsupportedColorConversion::EnumeratedColorSpace,
        }),
    }
}

fn clear_channel_definitions(components: &mut [TypedComponent]) {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= components.len(),
            decreases components.len() - index,
        )
    )]
    while index < components.len() {
        if let Some(component) = components.get_mut(index) {
            component.clear_channel_definitions();
        }
        index += 1;
    }
}

fn validate_layer_definition(
    definition: ChannelDefinition,
    color_count: usize,
) -> Result<(), DecodeError> {
    match definition.association() {
        ChannelAssociation::Color(value) if value == 0 || usize::from(value) > color_count => {
            Err(invalid_layer())
        }
        ChannelAssociation::Unassociated
            if matches!(definition.channel_type(), ChannelType::Color) =>
        {
            Err(invalid_layer())
        }
        _ => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn register_samples(
    source: &[u16],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    horizontal_subsampling: u32,
    vertical_subsampling: u32,
    horizontal_grid: u32,
    vertical_grid: u32,
    horizontal_offset: u32,
    vertical_offset: u32,
) -> Result<Vec<u16>, DecodeError> {
    if source.len() != checked_area(source_width, source_height)?
        || horizontal_subsampling == 0
        || vertical_subsampling == 0
        || horizontal_grid == 0
        || vertical_grid == 0
    {
        return Err(invalid_layer());
    }
    let mut output = vec![0_u16; checked_area(width, height)?];
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= height && vertical_subsampling > 0 && vertical_grid > 0,
            decreases height - y,
        )
    )]
    while y < height {
        let source_y_numerator = y
            .checked_mul(vertical_grid)
            .ok_or_else(arithmetic_overflow)?;
        if source_y_numerator >= vertical_offset {
            let source_y = crate::refinement::registered_source_coordinate(
                source_y_numerator,
                vertical_offset,
                vertical_subsampling,
            );
            if source_y < source_height {
                let mut x = 0_u32;
                if horizontal_subsampling == 0 || horizontal_grid == 0 {
                    return Err(invalid_layer());
                }
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant x <= width && horizontal_subsampling > 0 && horizontal_grid > 0,
                        decreases width - x,
                    )
                )]
                while x < width {
                    let source_x_numerator = x
                        .checked_mul(horizontal_grid)
                        .ok_or_else(arithmetic_overflow)?;
                    if source_x_numerator >= horizontal_offset {
                        let source_x = crate::refinement::registered_source_coordinate(
                            source_x_numerator,
                            horizontal_offset,
                            horizontal_subsampling,
                        );
                        if source_x < source_width {
                            let destination = index_of(width, x, y)?;
                            *output.get_mut(destination).ok_or_else(invalid_layer)? = *source
                                .get(index_of(source_width, source_x, source_y)?)
                                .ok_or_else(invalid_layer)?;
                        }
                    }
                    x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
            }
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(output)
}

fn normalized_chroma_keys_for_components(
    values: &[u8],
    descriptors: &[(u8, bool)],
) -> Result<Vec<u16>, DecodeError> {
    let mut expected = 0_usize;
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= descriptors.len(),
            decreases descriptors.len() - index,
        )
    )]
    while index < descriptors.len() {
        let bits = descriptors.get(index).ok_or_else(invalid_layer)?.0;
        if bits == 0 || bits > 38 {
            return Err(invalid_layer());
        }
        expected = expected
            .checked_add(usize::from(bits.saturating_add(7) / 8))
            .ok_or_else(arithmetic_overflow)?;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if values.len() != expected {
        return Err(invalid_layer());
    }
    let mut keys = Vec::with_capacity(descriptors.len());
    let mut offset = 0_usize;
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= descriptors.len(),
            decreases descriptors.len() - index,
        )
    )]
    while index < descriptors.len() {
        let (bits, signed) = *descriptors.get(index).ok_or_else(invalid_layer)?;
        let bytes = usize::from(bits.saturating_add(7) / 8);
        let end = offset.checked_add(bytes).ok_or_else(arithmetic_overflow)?;
        let key = normalized_chroma_keys(
            values.get(offset..end).ok_or_else(invalid_layer)?,
            1,
            ComponentBitDepth::Uniform { bits, signed },
        )?;
        keys.push(*key.first().ok_or_else(invalid_layer)?);
        offset = end;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(keys)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (result, owned_layer_view(final(owned)))
            == crate::specs::part2_apply_chroma_key(owned_layer_view(old(owned)), keys@),
    )
)]
fn apply_chroma_key(owned: &mut OwnedLayer, keys: &[u16]) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = owned_layer_view(owned); }
    if keys.len() != owned.colors.len() {
        return Err(invalid_layer());
    }
    let pixels = checked_area(owned.width, owned.height)?;
    let mut alpha = vec![u16::MAX; pixels];
    let mut pixel = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant pixel <= pixels, alpha.len() == pixels,
                owned_layer_view(owned) == input,
                input == owned_layer_view(old(owned)),
                keys.len() == input.3.len(),
                crate::specs::part2_layer_area(input.0, input.1) == Ok(pixels),
                forall|index: int| 0 <= index < pixel ==>
                    alpha@[index] == crate::specs::part2_chroma_key_pixel(input.3, keys@, index),
            decreases pixels - pixel,
        )
    )]
    while pixel < pixels {
        let mut matches = true;
        let mut color = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant color <= owned.colors.len(),
                    owned_layer_view(owned) == input,
                    keys.len() == input.3.len(),
                    matches == crate::proofs::internal_specs::part2_chroma_key_prefix_matches(
                        input.3, keys@, pixel as int, color as nat),
                decreases owned.colors.len() - color,
            )
        )]
        while color < owned.colors.len() {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost previous = matches; }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(color < input.3.len());
                assert(owned.colors@[color as int]@ == input.3[color as int]);
            }
            let sample = match owned.colors.get(color) {
                Some(samples) => samples.get(pixel),
                None => None,
            };
            let key = keys.get(color);
            let matches_key = match (sample, key) {
                (Some(sample), Some(key)) => *sample == *key,
                (None, None) => true,
                _ => false,
            };
            if !matches_key {
                matches = false;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(key.is_some());
                if let Some(value) = key { assert(*value == keys@[color as int]); }
                assert(sample.is_some() == (pixel < input.3[color as int].len()));
                if let Some(value) = sample { assert(*value == input.3[color as int][pixel as int]); }
                assert(matches_key == (pixel < input.3[color as int].len()
                    && input.3[color as int][pixel as int] == keys@[color as int]));
                assert(matches == (previous && matches_key));
                crate::proofs::production_impl::production_chroma_key_prefix_step(
                    input.3, keys@, pixel as int, color as nat);
            }
            color = color.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_chroma_key_prefix_complete(input.3, keys@, pixel as int);
            assert(crate::specs::part2_chroma_key_pixel(input.3, keys@, pixel as int)
                == crate::specs::part2_chroma_key_alpha(matches));
        }
        *alpha.get_mut(pixel).ok_or_else(invalid_layer)? =
            crate::refinement::chroma_key_alpha(matches);
        pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(alpha@ =~= Seq::new(pixels as nat, |index: int|
            crate::specs::part2_chroma_key_pixel(input.3, keys@, index)));
    }
    owned.alpha = Some((alpha, false));
    Ok(())
}

fn register_layer(
    source: &OwnedLayer,
    registration: &jp2::CodestreamRegistration,
    horizontal_grid_size: u16,
    vertical_grid_size: u16,
) -> Result<OwnedLayer, DecodeError> {
    let horizontal_subsampling = u32::from(registration.horizontal_subsampling);
    let vertical_subsampling = u32::from(registration.vertical_subsampling);
    let horizontal_grid = u32::from(horizontal_grid_size);
    let vertical_grid = u32::from(vertical_grid_size);
    if horizontal_subsampling == 0
        || vertical_subsampling == 0
        || horizontal_grid == 0
        || vertical_grid == 0
    {
        return Err(invalid_layer());
    }
    let width = registered_extent(
        source.width,
        horizontal_subsampling,
        horizontal_grid,
        registration.horizontal_offset,
    )?;
    let height = registered_extent(
        source.height,
        vertical_subsampling,
        vertical_grid,
        registration.vertical_offset,
    )?;
    let pixels = checked_area(width, height)?;
    let mut colors = vec![vec![0_u16; pixels]; source.colors.len()];
    let mut alpha = vec![0_u16; pixels];
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= height && vertical_subsampling > 0,
            decreases height - y,
        )
    )]
    while y < height {
        let source_y_numerator = y
            .checked_mul(vertical_grid)
            .ok_or_else(arithmetic_overflow)?;
        if source_y_numerator >= registration.vertical_offset {
            let source_y = crate::refinement::registered_source_coordinate(
                source_y_numerator,
                registration.vertical_offset,
                vertical_subsampling,
            );
            if source_y < source.height {
                let mut x = 0_u32;
                if horizontal_subsampling == 0 {
                    return Err(invalid_layer());
                }
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant x <= width && horizontal_subsampling > 0,
                        decreases width - x,
                    )
                )]
                while x < width {
                    let source_x_numerator = x
                        .checked_mul(horizontal_grid)
                        .ok_or_else(arithmetic_overflow)?;
                    if source_x_numerator >= registration.horizontal_offset {
                        let source_x = crate::refinement::registered_source_coordinate(
                            source_x_numerator,
                            registration.horizontal_offset,
                            horizontal_subsampling,
                        );
                        if source_x < source.width {
                            let source_index = index_of(source.width, source_x, source_y)?;
                            let destination = index_of(width, x, y)?;
                            let mut color = 0_usize;
                            #[cfg_attr(
                                verus_keep_ghost,
                                verus_spec(
                                    invariant color <= colors.len(),
                                    decreases colors.len() - color,
                                )
                            )]
                            while color < colors.len() {
                                let destination_samples =
                                    colors.get_mut(color).ok_or_else(invalid_layer)?;
                                *destination_samples
                                    .get_mut(destination)
                                    .ok_or_else(invalid_layer)? = *source
                                    .colors
                                    .get(color)
                                    .and_then(|samples| samples.get(source_index))
                                    .ok_or_else(invalid_layer)?;
                                color = color.checked_add(1).ok_or_else(arithmetic_overflow)?;
                            }
                            *alpha.get_mut(destination).ok_or_else(invalid_layer)? =
                                match source.alpha.as_ref() {
                                    Some(value) => {
                                        *value.0.get(source_index).ok_or_else(invalid_layer)?
                                    }
                                    None => u16::MAX,
                                };
                        }
                    }
                    x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
            }
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(OwnedLayer {
        width,
        height,
        color_space: source.color_space,
        colors,
        alpha: Some((
            alpha,
            match source.alpha.as_ref() {
                Some(value) => value.1,
                None => false,
            },
        )),
    })
}

fn apply_layer_opacity(
    file: &Jp2File<'_>,
    layer: &CompositingLayer<'_>,
    color_codestream: usize,
    owned: &mut OwnedLayer,
    alpha_registration: Option<&jp2::CodestreamRegistration>,
    options: &DecodeOptions,
) -> Result<(), DecodeError> {
    let Some(opacity) = layer.opacity() else {
        return Ok(());
    };
    match opacity {
        LayerOpacity::Straight | LayerOpacity::Premultiplied => {
            let premultiplied = matches!(opacity, LayerOpacity::Premultiplied);
            if let Some(registration) = alpha_registration {
                let image = decode_layer(
                    file,
                    usize::from(registration.codestream_index),
                    None,
                    options,
                )?;
                let alpha_source = owned_layer_from_image(&image)?;
                let alpha_registered = register_layer(
                    &alpha_source,
                    registration,
                    layer.horizontal_grid_size(),
                    layer.vertical_grid_size(),
                )?;
                let samples = alpha_registered.colors.first().ok_or_else(invalid_layer)?;
                let mut alpha = vec![0_u16; checked_area(owned.width, owned.height)?];
                let copy_width = owned.width.min(alpha_registered.width);
                let copy_height = owned.height.min(alpha_registered.height);
                let mut y = 0_u32;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant y <= copy_height,
                        decreases copy_height - y,
                    )
                )]
                while y < copy_height {
                    let mut x = 0_u32;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant x <= copy_width,
                            decreases copy_width - x,
                        )
                    )]
                    while x < copy_width {
                        let destination = index_of(owned.width, x, y)?;
                        *alpha.get_mut(destination).ok_or_else(invalid_layer)? = *samples
                            .get(index_of(alpha_registered.width, x, y)?)
                            .ok_or_else(invalid_layer)?;
                        x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                    }
                    y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                owned.alpha = Some((alpha, premultiplied));
            } else if let Some(alpha) = owned.alpha.as_mut() {
                alpha.1 = premultiplied;
            } else {
                return Err(DecodeError::malformed(
                    layer.box_header().offset(),
                    MalformedKind::MissingRequiredStructure,
                ));
            }
        }
        LayerOpacity::ChromaKey { channels, values } => {
            if usize::from(*channels) != owned.colors.len() {
                return Err(DecodeError::malformed(
                    layer.box_header().offset(),
                    MalformedKind::InvalidBoxContents,
                ));
            }
            let header = file
                .codestream(color_codestream)?
                .codestream_header()
                .and_then(|association| association.parsed_header())
                .unwrap_or(file.header());
            let keys = normalized_chroma_keys(values, *channels, header.image.bit_depth)?;
            let pixels = checked_area(owned.width, owned.height)?;
            let mut alpha = vec![u16::MAX; pixels];
            let mut pixel = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant pixel <= pixels,
                    decreases pixels - pixel,
                )
            )]
            while pixel < pixels {
                let mut matches = true;
                let mut color = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant color <= owned.colors.len(),
                        decreases owned.colors.len() - color,
                    )
                )]
                while color < owned.colors.len() {
                    let sample = owned
                        .colors
                        .get(color)
                        .and_then(|samples| samples.get(pixel))
                        .ok_or_else(invalid_layer)?;
                    let key = keys.get(color).ok_or_else(invalid_layer)?;
                    if sample != key {
                        matches = false;
                    }
                    color = color.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                *alpha.get_mut(pixel).ok_or_else(invalid_layer)? =
                    crate::refinement::chroma_key_alpha(matches);
                pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            owned.alpha = Some((alpha, false));
        }
    }
    Ok(())
}

fn normalized_chroma_keys(
    values: &[u8],
    channels: u8,
    bit_depth: ComponentBitDepth,
) -> Result<Vec<u16>, DecodeError> {
    let ComponentBitDepth::Uniform { bits, signed } = bit_depth else {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::MissingRequiredStructure,
        ));
    };
    if bits == 0 || bits > 38 {
        return Err(invalid_layer());
    }
    let bytes = usize::from(bits.saturating_add(7) / 8);
    if values.len()
        != bytes
            .checked_mul(usize::from(channels))
            .ok_or_else(arithmetic_overflow)?
    {
        return Err(invalid_layer());
    }
    let mut keys = Vec::with_capacity(usize::from(channels));
    let mut channel = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant channel <= channels as usize,
            decreases channels as usize - channel,
        )
    )]
    while channel < usize::from(channels) {
        let start = channel.checked_mul(bytes).ok_or_else(arithmetic_overflow)?;
        let mut raw = 0_u64;
        let mut byte = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant byte <= bytes,
                decreases bytes - byte,
            )
        )]
        while byte < bytes {
            raw = raw
                .checked_mul(256)
                .and_then(|value| {
                    values
                        .get(start.checked_add(byte)?)
                        .and_then(|sample| value.checked_add(u64::from(*sample)))
                })
                .ok_or_else(arithmetic_overflow)?;
            byte = byte.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let mut mask = 0_u64;
        let mut mask_bit = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant mask_bit <= bits,
                decreases bits - mask_bit,
            )
        )]
        while mask_bit < bits {
            mask = mask
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(arithmetic_overflow)?;
            mask_bit = mask_bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        if mask == 0 {
            return Err(invalid_layer());
        }
        raw &= mask;
        let biased = if signed {
            let sign_bit = mask.checked_add(1).ok_or_else(arithmetic_overflow)? / 2;
            raw ^ sign_bit
        } else {
            raw
        };
        let normalized = u128::from(biased)
            .checked_mul(u128::from(u16::MAX))
            .and_then(|value| value.checked_add(u128::from(mask) / 2))
            .ok_or_else(arithmetic_overflow)?
            / u128::from(mask);
        keys.push(u16::try_from(normalized).map_err(|_| arithmetic_overflow())?);
        channel = channel.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(keys)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::color::converted_image_result(result) == crate::specs::part2_render_layers(
            width, height, placement_views(layers@), maximum_decoded_bytes, crate::limits::resource_work_limit(limits),
        ),
    )
)]
fn render_decoded_layers(
    width: u32,
    height: u32,
    layers: &[(OwnedLayer, JpxInstructionInfo)],
    maximum_decoded_bytes: u64,
    limits: &ResourceLimits,
) -> Result<ConvertedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = placement_views(layers@);
        let ghost initial = crate::specs::part2_render_initial(width, height, input);
        let ghost work_limit = crate::limits::resource_work_limit(limits);
    }
    if layers.is_empty() {
        return Err(invalid_layer());
    }
    let mut output_space = ConvertedColorSpace::Grayscale;
    let mut scan = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant scan <= layers.len(), input == placement_views(layers@),
                matches!(output_space, ConvertedColorSpace::Srgb)
                    <==> exists|i: int| 0 <= i < scan && #[trigger] input[i].0.2 == ConvertedColorSpace::Srgb,
            decreases layers.len() - scan,
        )
    )]
    while scan < layers.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(input[scan as int].0.2 == layers@[scan as int].0.color_space);
        }
        if matches!(layers[scan].0.color_space, ConvertedColorSpace::Srgb) {
            output_space = ConvertedColorSpace::Srgb;
        }
        scan += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(output_space == crate::specs::part2_render_space(input));
        crate::proofs::production_impl::production_render_empty(width, height, input, initial, maximum_decoded_bytes, work_limit);
    }
    let color_count = if matches!(output_space, ConvertedColorSpace::Srgb) {
        3
    } else {
        1
    };
    let pixels = match checked_area(width, height) {
        Ok(pixels) => pixels,
        Err(error) => {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_render_error(width, height, input, initial, maximum_decoded_bytes, work_limit, 0);
            }
            return Err(error);
        }
    };
    let mut output_colors = Vec::with_capacity(color_count);
    let mut color = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant color <= color_count,
                input == placement_views(layers@),
                initial == crate::specs::part2_render_initial(width, height, input),
                crate::specs::part2_layer_area(width, height) == Ok(pixels),
                output_space == crate::specs::part2_render_space(input),
                color_count as nat == crate::specs::part2_layer_color_count(output_space),
                crate::specs::part2_composition_planes(output_colors@) == Seq::new(color as nat, |i: int| Seq::new(pixels as nat, |j: int| 0u16)),
            decreases color_count - color,
        )
    )]
    while color < color_count {
        let samples = vec![0_u16; pixels];
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(samples@ =~= Seq::new(pixels as nat, |j: int| 0u16));
            layer_owned_planes_push(output_colors@, samples);
        }
        output_colors.push(samples);
        color += 1;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_composition_planes(output_colors@) =~= Seq::new(color as nat, |i: int| Seq::new(pixels as nat, |j: int| 0u16)));
        }
    }
    let mut output_alpha = vec![0_u16; pixels];
    let output_channels = (color_count as u64 + 1) * 2;
    let mut decoded_bytes = match (pixels as u64).checked_mul(output_channels) {
        Some(bytes) => bytes,
        None => {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_render_error(width, height, input, initial, maximum_decoded_bytes, work_limit, 0);
            }
            return Err(arithmetic_overflow());
        }
    };
    let mut work = pixels as u64;
    let mut layer_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant layer_index <= layers.len(),
                input == placement_views(layers@),
                initial == crate::specs::part2_render_initial(width, height, input),
                output_space == crate::specs::part2_render_space(input),
                work_limit == crate::limits::resource_work_limit(limits),
                crate::specs::part2_render_prefix(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat)
                    == (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, work as nat, decoded_bytes as nat),
            decreases layers.len() - layer_index,
        )
    )]
    while layer_index < layers.len() {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost before = (Ok::<(), DecodeError>(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, work as nat, decoded_bytes as nat);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_render_step(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat);
        }
        let (layer, instruction) = &layers[layer_index];
        let charge = charge_layer_bytes(layer, &mut decoded_bytes, maximum_decoded_bytes);
        #[cfg(verus_keep_ghost)]
        proof! {
            if charge.is_err() {
                assert(crate::specs::part2_render_prefix(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat + 1)
                    == (charge, crate::specs::part2_composition_planes(output_colors@), output_alpha@, work as nat, decoded_bytes as nat));
                crate::proofs::production_impl::production_render_error(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat + 1);
            }
        }
        charge?;
        let outcome = composite_placed_layer(
            width,
            height,
            output_space,
            layer,
            *instruction,
            &mut output_colors,
            &mut output_alpha,
            &mut work,
            limits,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_render_step(width, height, output_space, input[layer_index as int], before,
                maximum_decoded_bytes, work_limit) == (outcome, crate::specs::part2_composition_planes(output_colors@), output_alpha@, work as nat, decoded_bytes as nat));
            assert(crate::specs::part2_render_prefix(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat + 1)
                == (outcome, crate::specs::part2_composition_planes(output_colors@), output_alpha@, work as nat, decoded_bytes as nat));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if outcome.is_err() {
                crate::proofs::production_impl::production_render_error(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat + 1);
            }
        }
        outcome?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(outcome == Ok(()));
            assert(crate::specs::part2_render_prefix(width, height, input, initial, maximum_decoded_bytes, work_limit, layer_index as nat + 1)
                == (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, work as nat, decoded_bytes as nat));
        }
        layer_index = layer_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    crate::color::from_composited(width, height, output_space, output_colors, output_alpha)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (result, *final(decoded_bytes) as nat) == crate::specs::part2_charge_layer_bytes(
            (layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha)),
            *old(decoded_bytes) as nat, maximum_decoded_bytes),
    )
)]
fn charge_layer_bytes(
    layer: &OwnedLayer,
    decoded_bytes: &mut u64,
    maximum_decoded_bytes: u64,
) -> Result<(), DecodeError> {
    let layer_bytes = layer_decoded_bytes(layer)?;
    *decoded_bytes = decoded_bytes
        .checked_add(layer_bytes)
        .ok_or_else(arithmetic_overflow)?;
    enforce_limit(
        Resource::DecodedBytes,
        maximum_decoded_bytes,
        *decoded_bytes,
    )
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures match result {
            Ok(bytes) => crate::specs::part2_render_layer_bytes(
                (layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha)))
                == Ok(bytes as nat),
            Err(error) => crate::specs::part2_render_layer_bytes(
                (layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha)))
                == Err(error),
        },
    )
)]
fn layer_decoded_bytes(layer: &OwnedLayer) -> Result<u64, DecodeError> {
    let channels = match (layer.colors.len() as u64).checked_add(1) {
        Some(channels) => channels,
        None => return Err(arithmetic_overflow()),
    };
    let bytes_per_pixel = match channels.checked_mul(2) {
        Some(bytes) => bytes,
        None => return Err(arithmetic_overflow()),
    };
    let pixels = checked_area(layer.width, layer.height)?;
    match (pixels as u64).checked_mul(bytes_per_pixel) {
        Some(bytes) => Ok(bytes),
        None => Err(arithmetic_overflow()),
    }
}

fn composition_dimensions(
    inspection: &JpxInspection,
    limits: &ResourceLimits,
) -> Result<(u32, u32), DecodeError> {
    let (width, height) = match inspection.composition() {
        Some(composition) => (composition.width(), composition.height()),
        None => {
            let mut width = 0_u32;
            let mut height = 0_u32;
            let layer_count = inspection.layers().len();
            let mut index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= layer_count,
                    decreases layer_count - index,
                )
            )]
            while index < layer_count {
                let layer = inspection.layer(index)?;
                width = width.max(
                    layer
                        .horizontal_offset()
                        .checked_add(layer.width())
                        .ok_or_else(arithmetic_overflow)?,
                );
                height = height.max(
                    layer
                        .vertical_offset()
                        .checked_add(layer.height())
                        .ok_or_else(arithmetic_overflow)?,
                );
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            (width, height)
        }
    };
    if width == 0 || height == 0 {
        return Err(invalid_layer());
    }
    enforce_limit(
        Resource::Width,
        u64::from(limits.maximum_width()),
        u64::from(width),
    )?;
    enforce_limit(
        Resource::Height,
        u64::from(limits.maximum_height()),
        u64::from(height),
    )?;
    enforce_limit(
        Resource::Pixels,
        limits.maximum_pixels(),
        u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(arithmetic_overflow)?,
    )?;
    Ok((width, height))
}

struct LayerChannels<'a> {
    width: u32,
    height: u32,
    colors: Vec<&'a [u16]>,
    alpha: Option<(Vec<u16>, bool)>,
}

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn layer_channels_outcome(result: Result<LayerChannels<'_>, DecodeError>) -> crate::specs::Part2LayerInputResult {
        match result {
            Err(error) => Err(error),
            Ok(layer) => Ok((layer.width, layer.height, layer_borrowed_planes(layer.colors@), layer_owned_alpha(layer.alpha))),
        }
    }
}

#[allow(clippy::collapsible_if)]
#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures layer_channels_outcome(result) == crate::specs::part2_layer_input(
            crate::color::converted_image_view(image).0, crate::color::converted_image_view(image).1,
            crate::color::converted_image_view(image).2,
            crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
        ),
    )
)]
fn layer_channels(image: &ConvertedImage) -> Result<LayerChannels<'_>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = crate::color::converted_channel_views(crate::color::converted_image_view(image).4);
    }
    let color_count = match image.color_space() {
        ConvertedColorSpace::Srgb => 3_usize,
        ConvertedColorSpace::Grayscale => 1_usize,
    };
    let mut colors: Vec<Option<&[u16]>> = vec![None; color_count];
    let mut alphas = Vec::new();
    let channel_count = image.channels().len();
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_layer_scan_empty(input, color_count as nat);
        assert(layer_color_slots(colors@) =~= Seq::new(color_count as nat, |i: int| None));
        assert(layer_alpha_planes(alphas@) =~= Seq::empty());
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= channel_count,
                input == crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                channel_count == input.len(),
                color_count as nat == crate::specs::part2_layer_color_count(crate::color::converted_image_view(image).2),
                colors.len() == color_count,
                crate::specs::part2_layer_channel_scan(input, color_count as nat, index as nat)
                    == Ok((layer_color_slots(colors@), layer_alpha_planes(alphas@))),
            decreases channel_count - index,
        )
    )]
    while index < channel_count {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost scan_colors = layer_color_slots(colors@);
            let ghost scan_alphas = layer_alpha_planes(alphas@);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_layer_scan_step(input, color_count as nat, index as nat);
        }
        let channel = image.channels().get(index).ok_or_else(invalid_layer)?;
        let color_index = match channel.channel_type() {
            ConvertedChannelType::Red => Some(0),
            ConvertedChannelType::Green => Some(1),
            ConvertedChannelType::Blue => Some(2),
            ConvertedChannelType::Gray => Some(0),
            ConvertedChannelType::Alpha {
                premultiplied,
                association,
            } => {
                let _ = association;
                alphas.push((channel.samples(), premultiplied));
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(layer_alpha_planes(alphas@) =~= scan_alphas.push((crate::color::converted_channel_view(channel).1, premultiplied)));
                }
                None
            }
        };
        if let Some(color_index) = color_index {
            if color_index >= colors.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_layer_scan_error(
                        input, color_count as nat, index as nat + 1, input.len(), crate::specs::part2_composition_error(false),
                    );
                }
                return Err(invalid_layer());
            }
            if colors[color_index].is_some() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_layer_scan_error(
                        input, color_count as nat, index as nat + 1, input.len(), crate::specs::part2_composition_error(false),
                    );
                }
                return Err(invalid_layer());
            }
            *colors.get_mut(color_index).ok_or_else(invalid_layer)? = Some(channel.samples());
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(layer_color_slots(colors@) =~= scan_colors.update(color_index as int, Some(crate::color::converted_channel_view(channel).1)));
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_layer_channel_scan(input, color_count as nat, index as nat + 1)
                == Ok((layer_color_slots(colors@), layer_alpha_planes(alphas@))));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost scanned_colors = layer_color_slots(colors@);
        let ghost scanned_alphas = layer_alpha_planes(alphas@);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_layer_area);
    }
    let expected = checked_area(image.width(), image.height())?;
    let mut resolved = Vec::with_capacity(color_count);
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= colors.len(),
                resolved.len() == index,
                input == crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                colors.len() == color_count,
                color_count as nat == crate::specs::part2_layer_color_count(crate::color::converted_image_view(image).2),
                expected as nat == (crate::color::converted_image_view(image).0 as nat) * (crate::color::converted_image_view(image).1 as nat),
                crate::specs::part2_layer_channel_scan(input, color_count as nat, input.len()) == Ok((scanned_colors, scanned_alphas)),
                scanned_colors == layer_color_slots(colors@),
                scanned_alphas == layer_alpha_planes(alphas@),
                forall |i: int| #![trigger scanned_colors[i]] 0 <= i < index ==> scanned_colors[i].is_some() && scanned_colors[i].unwrap().len() == expected,
                layer_borrowed_planes(resolved@) == Seq::new(index as nat, |i: int| scanned_colors[i].unwrap()),
            decreases colors.len() - index,
        )
    )]
    while index < colors.len() {
        let samples = match *colors.get(index).ok_or_else(invalid_layer)? {
            Some(samples) => samples,
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(scanned_colors[index as int].is_none());
                }
                return Err(invalid_layer());
            }
        };
        if samples.len() != expected {
            return Err(invalid_layer());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(samples@ == scanned_colors[index as int].unwrap());
            layer_borrowed_planes_push(resolved@, samples);
        }
        resolved.push(samples);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(layer_borrowed_planes(resolved@) =~= Seq::new(index as nat + 1, |i: int| scanned_colors[i].unwrap()));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut resolved_alpha = None;
    let mut alpha_index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(layer_borrowed_planes(resolved@) =~= Seq::new(scanned_colors.len(), |i: int| scanned_colors[i].unwrap()));
        crate::proofs::production_impl::production_layer_alpha_empty(scanned_alphas, expected as nat);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant alpha_index <= alphas.len(),
                input == crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                color_count as nat == crate::specs::part2_layer_color_count(crate::color::converted_image_view(image).2),
                expected as nat == (crate::color::converted_image_view(image).0 as nat) * (crate::color::converted_image_view(image).1 as nat),
                crate::specs::part2_layer_channel_scan(input, color_count as nat, input.len()) == Ok((scanned_colors, scanned_alphas)),
                scanned_alphas == layer_alpha_planes(alphas@),
                crate::specs::part2_layer_resolve_colors(scanned_colors, expected as nat) == Ok(layer_borrowed_planes(resolved@)),
                crate::specs::part2_layer_alpha_fold(scanned_alphas, expected as nat, alpha_index as nat) == Ok(layer_owned_alpha(resolved_alpha)),
                alpha_index == 0 ==> resolved_alpha.is_none(),
                alpha_index > 0 ==> resolved_alpha.is_some() && resolved_alpha.unwrap().0.len() == expected,
            decreases alphas.len() - alpha_index,
        )
    )]
    while alpha_index < alphas.len() {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost previous_alpha = layer_owned_alpha(resolved_alpha);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_layer_alpha_step(scanned_alphas, expected as nat, alpha_index as nat);
        }
        let (samples, premultiplied) = *alphas.get(alpha_index).ok_or_else(invalid_layer)?;
        if samples.len() != expected {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_layer_alpha_error(
                    scanned_alphas, expected as nat, alpha_index as nat + 1, scanned_alphas.len(), crate::specs::part2_composition_error(false),
                );
            }
            return Err(invalid_layer());
        }
        if alpha_index == 0 {
            resolved_alpha = Some((copy_samples(samples)?, premultiplied));
        } else {
            let (combined, combined_premultiplied) =
                resolved_alpha.as_mut().ok_or_else(invalid_layer)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost initial_combined = combined@;
            }
            let mut pixel = 0_usize;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(combined@ =~= crate::proofs::internal_specs::part2_layer_alpha_product_prefix(initial_combined, samples@, 0));
            }
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant pixel <= combined.len(),
                        combined.len() == expected,
                        samples.len() == expected,
                        initial_combined.len() == expected,
                        combined@ == crate::proofs::internal_specs::part2_layer_alpha_product_prefix(initial_combined, samples@, pixel as nat),
                    decreases combined.len() - pixel,
                )
            )]
            while pixel < combined.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_layer_alpha_product_step(initial_combined, samples@, pixel as nat);
                }
                let combined_sample = *combined.get(pixel).ok_or_else(invalid_layer)?;
                let alpha_sample = *samples.get(pixel).ok_or_else(invalid_layer)?;
                *combined.get_mut(pixel).ok_or_else(invalid_layer)? =
                    crate::refinement::normalized_sample_product(combined_sample, alpha_sample)
                        .ok_or_else(arithmetic_overflow)?;
                pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(combined@ =~= Seq::new(expected as nat, |i: int| crate::specs::part2_normalized_sample_product(initial_combined[i], samples@[i]) as u16));
            }
            *combined_premultiplied = false;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_layer_alpha_fold(scanned_alphas, expected as nat, alpha_index as nat + 1)
                == Ok(layer_owned_alpha(resolved_alpha)));
        }
        alpha_index = alpha_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(LayerChannels {
        width: image.width(),
        height: image.height(),
        colors: resolved,
        alpha: resolved_alpha,
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result.is_ok() ==> (result.is_ok(), crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@, *final(work_units) as nat)
            == crate::specs::part2_composition_layer_success(
                crate::specs::part2_composition_planes(source.colors@),
                match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                crate::specs::part2_composition_geometry(
                    source.width, source.height, canvas_width, canvas_height,
                    instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                ),
                matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                *old(work_units) as nat,
            ),
            crate::specs::part2_composition_layer_prefix_safe(
                crate::specs::part2_composition_planes(source.colors@),
                match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                crate::specs::part2_composition_geometry(
                    source.width, source.height, canvas_width, canvas_height,
                    instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                ),
                matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                *old(work_units) as nat,
                crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@, *final(work_units) as nat,
            ),
            (result, crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@, *final(work_units) as nat)
                == crate::specs::part2_composition_layer_transition(
                    crate::specs::part2_composition_planes(source.colors@),
                    match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                    crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                    crate::specs::part2_composition_geometry(
                        source.width, source.height, canvas_width, canvas_height,
                        instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                        instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                    ),
                    matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                    *old(work_units) as nat, crate::limits::resource_work_limit(limits),
                ),
    )
)]
fn composite_placed_layer(
    canvas_width: u32,
    canvas_height: u32,
    output_space: ConvertedColorSpace,
    source: &OwnedLayer,
    instruction: JpxInstructionInfo,
    output_colors: &mut [Vec<u16>],
    output_alpha: &mut [u16],
    work_units: &mut u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let expand_grayscale = matches!(
        (output_space, source.color_space),
        (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)
    );
    #[allow(
        clippy::manual_map,
        reason = "Explicit cases preserve the verified slice and representation relation"
    )]
    let source_alpha = match source.alpha.as_ref() {
        Some((samples, flag)) => Some((samples.as_slice(), *flag)),
        None => None,
    };
    let maximum_work_units = limits.maximum_work_units();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost geometry = crate::specs::part2_composition_geometry(
            source.width, source.height, canvas_width, canvas_height,
            instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
            instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
        );
        let ghost source_planes = crate::specs::part2_composition_planes(source.colors@);
        let ghost layer_alpha = match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None };
        let ghost initial_colors = crate::specs::part2_composition_planes(output_colors@);
        let ghost initial_alpha = output_alpha@;
        let ghost initial_work = *work_units;
    }
    let source_x = instruction.source_x.min(source.width);
    let source_y = instruction.source_y.min(source.height);
    let source_width = if instruction.source_width == 0 {
        source.width.saturating_sub(source_x)
    } else {
        instruction
            .source_width
            .min(source.width.saturating_sub(source_x))
    };
    let source_height = if instruction.source_height == 0 {
        source.height.saturating_sub(source_y)
    } else {
        instruction
            .source_height
            .min(source.height.saturating_sub(source_y))
    };
    if source_width == 0 || source_height == 0 {
        return Ok(());
    }
    let target_width = instruction.target_width;
    let target_height = instruction.target_height;
    if target_width == 0 || target_height == 0 {
        return Err(invalid_layer());
    }
    let copy_width =
        crate::refinement::composition_clip(instruction.target_x, target_width, canvas_width);
    let copy_height =
        crate::refinement::composition_clip(instruction.target_y, target_height, canvas_height);
    if copy_width == 0 || copy_height == 0 {
        return Ok(());
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_composition_canvas_empty(
            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
        );
        crate::proofs::production_impl::production_composition_budgeted_empty(
            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
            initial_work as nat, maximum_work_units,
        );
        crate::proofs::production_impl::production_composition_prefix_observation(
            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
            crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, 0,
        );
    }
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                y <= copy_height,
                copy_width > 0 && copy_height > 0,
                geometry == crate::specs::part2_composition_geometry(
                    source.width, source.height, canvas_width, canvas_height,
                    instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                ),
                geometry.source_x == source_x as nat && geometry.source_y == source_y as nat,
                geometry.source_crop_width == source_width as nat && geometry.source_crop_height == source_height as nat,
                geometry.target_width == target_width as nat && geometry.target_height == target_height as nat,
                geometry.copy_width == copy_width as nat && geometry.copy_height == copy_height as nat,
                source_planes == crate::specs::part2_composition_planes(source.colors@),
                layer_alpha == match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                layer_alpha == match source_alpha { Some((samples, flag)) => Some((samples@, flag)), None => None },
                expand_grayscale == matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                initial_colors == crate::specs::part2_composition_planes(old(output_colors)@),
                initial_alpha == old(output_alpha)@,
                initial_work == *old(work_units),
                maximum_work_units == crate::limits::resource_work_limit(limits),
                output_colors.len() == initial_colors.len(),
                (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                    == crate::specs::part2_composition_budgeted_prefix(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, (y as nat) * (copy_width as nat),
                    ),
                (true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                    == crate::specs::part2_composition_canvas_prefix(source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, (y as nat) * (copy_width as nat)),
                *work_units as nat == initial_work as nat + (y as nat) * (copy_width as nat) * (initial_colors.len() + 1),
                crate::specs::part2_composition_layer_prefix_safe(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                    crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat,
                ),
                target_width > 0 && target_height > 0,
                copy_width <= target_width && copy_height <= target_height,
                source_x <= source.width && source_y <= source.height,
                source_width > 0 && source_height > 0,
                source_width <= source.width - source_x,
                source_height <= source.height - source_y,
                copy_width > 0 ==> instruction.target_x < canvas_width
                    && copy_width <= canvas_width - instruction.target_x,
                copy_height > 0 ==> instruction.target_y < canvas_height
                    && copy_height <= canvas_height - instruction.target_y,
            decreases copy_height - y,
        )
    )]
    while y < copy_height {
        let mapped_y =
            crate::refinement::composition_mapped_offset(y, source_height, target_height);
        let sy = source_y
            .checked_add(u32::try_from(mapped_y).map_err(|_| arithmetic_overflow())?)
            .ok_or_else(arithmetic_overflow)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(source_y <= sy && sy < source.height);
            assert((sy as nat) < (source_y as nat) + source_height as nat);
            assert(sy as nat == (source_y as nat)
                + crate::specs::part2_composition_mapped_offset(y, source_height, target_height));
        }
        let mut x = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    x <= copy_width,
                    copy_width > 0 && copy_height > 0,
                    geometry == crate::specs::part2_composition_geometry(
                        source.width, source.height, canvas_width, canvas_height,
                        instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                        instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                    ),
                    geometry.source_x == source_x as nat && geometry.source_y == source_y as nat,
                    geometry.source_crop_width == source_width as nat && geometry.source_crop_height == source_height as nat,
                    geometry.target_width == target_width as nat && geometry.target_height == target_height as nat,
                    geometry.copy_width == copy_width as nat && geometry.copy_height == copy_height as nat,
                    source_planes == crate::specs::part2_composition_planes(source.colors@),
                    layer_alpha == match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                    layer_alpha == match source_alpha { Some((samples, flag)) => Some((samples@, flag)), None => None },
                    expand_grayscale == matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                    initial_colors == crate::specs::part2_composition_planes(old(output_colors)@),
                    initial_alpha == old(output_alpha)@,
                    initial_work == *old(work_units),
                    maximum_work_units == crate::limits::resource_work_limit(limits),
                    output_colors.len() == initial_colors.len(),
                    (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                        == crate::specs::part2_composition_budgeted_prefix(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, (y as nat) * (copy_width as nat) + x as nat,
                        ),
                    (true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                        == crate::specs::part2_composition_canvas_prefix(source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, (y as nat) * (copy_width as nat) + x as nat),
                    *work_units as nat == initial_work as nat + ((y as nat) * (copy_width as nat) + x as nat) * (initial_colors.len() + 1),
                    crate::specs::part2_composition_layer_prefix_safe(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                        crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat,
                    ),
                    target_width > 0 && target_height > 0,
                    copy_width <= target_width,
                    y < copy_height && copy_height <= target_height,
                    source_height > 0,
                    source_x <= source.width && source_width > 0,
                    source_width <= source.width - source_x,
                    copy_width > 0 ==> instruction.target_x < canvas_width
                        && copy_width <= canvas_width - instruction.target_x,
                    sy < source.height,
                    source_y <= sy,
                    (sy as nat) < (source_y as nat) + source_height as nat,
                    sy as nat == (source_y as nat)
                        + crate::specs::part2_composition_mapped_offset(y, source_height, target_height),
                    (instruction.target_y as nat) + (y as nat) < canvas_height as nat,
                decreases copy_width - x,
            )
        )]
        while x < copy_width {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost completed = (y as nat) * (copy_width as nat) + x as nat;
                let ghost previous_state = crate::specs::part2_composition_canvas_prefix(
                    source_planes, layer_alpha, initial_colors, initial_alpha,
                    geometry, expand_grayscale, completed,
                );
                let ghost previous_work = *work_units;
                let ghost previous_budgeted = crate::specs::part2_composition_budgeted_prefix(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                    initial_work as nat, maximum_work_units, completed,
                );
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(previous_state == (true, crate::specs::part2_composition_planes(output_colors@), output_alpha@));
                assert(previous_work as nat == initial_work as nat + completed * (initial_colors.len() + 1));
                assert(previous_budgeted == (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, previous_work as nat));
                crate::proofs::production_impl::production_composition_raster_prefix_bound(copy_width as nat, copy_height as nat, x as nat, y as nat);
                crate::proofs::production_impl::production_composition_budgeted_step(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                    initial_work as nat, maximum_work_units, x as nat, y as nat,
                );
            }
            let mapped_x =
                crate::refinement::composition_mapped_offset(x, source_width, target_width);
            let sx = source_x
                .checked_add(u32::try_from(mapped_x).map_err(|_| arithmetic_overflow())?)
                .ok_or_else(arithmetic_overflow)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(sx as nat == geometry.source_x + (x as nat) * geometry.source_crop_width / geometry.target_width);
                assert(sy as nat == geometry.source_y + (y as nat) * geometry.source_crop_height / geometry.target_height);
            }
            #[allow(
                clippy::question_mark,
                reason = "The index-error branch proves its exact unchanged state"
            )]
            let source_index = match index_of(source.width, sx, sy) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert((Err(error), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                            == crate::specs::part2_composition_budgeted_prefix(
                                source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                                initial_work as nat, maximum_work_units, completed + 1,
                            ));
                        crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                        );
                    }
                    return Err(error);
                }
            };
            let destination_x = instruction
                .target_x
                .checked_add(x)
                .ok_or_else(arithmetic_overflow)?;
            let destination_y = instruction
                .target_y
                .checked_add(y)
                .ok_or_else(arithmetic_overflow)?;
            #[allow(
                clippy::question_mark,
                reason = "The index-error branch proves its exact unchanged state"
            )]
            let destination_index = match index_of(canvas_width, destination_x, destination_y) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert((Err(error), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                            == crate::specs::part2_composition_budgeted_prefix(
                                source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                                initial_work as nat, maximum_work_units, completed + 1,
                            ));
                        crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                        );
                    }
                    return Err(error);
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(source_x <= sx && sx < source.width);
                assert((sx as nat) < (source_x as nat) + source_width as nat);
                assert(sx as nat == (source_x as nat)
                    + crate::specs::part2_composition_mapped_offset(x, source_width, target_width));
                assert(destination_x < canvas_width && destination_y < canvas_height);
                crate::proofs::production_impl::production_composition_index_bounds(source.width, source.height, sx, sy);
                crate::proofs::production_impl::production_composition_index_bounds(canvas_width, canvas_height, destination_x, destination_y);
                assert(source_index as nat == crate::specs::part2_row_major_index(source.width, sx, sy));
                assert(destination_index as nat == crate::specs::part2_row_major_index(canvas_width, destination_x, destination_y));
                assert((source_index as nat) < (source.width as nat) * (source.height as nat));
                assert((destination_index as nat) < (canvas_width as nat) * (canvas_height as nat));
                crate::proofs::production_impl::production_composition_canvas_step(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                    x as nat, y as nat, source_index as nat, destination_index as nat,
                );
            }
            if !crate::refinement::compose_planar_pixel(
                &source.colors,
                source_alpha,
                output_colors,
                output_alpha,
                source_index,
                destination_index,
                expand_grayscale,
            ) {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_composition_prefix_observation(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                        crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, completed,
                    );
                    assert((Err(crate::specs::part2_composition_error(false)), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                        == crate::specs::part2_composition_budgeted_prefix(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1,
                        ));
                    crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                    );
                }
                return Err(invalid_layer());
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert((true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                    == crate::specs::part2_composition_pixel_transition(
                        source_planes, layer_alpha, previous_state.1, previous_state.2,
                        source_index as nat, destination_index as nat, expand_grayscale,
                    ));
                assert((true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                    == crate::specs::part2_composition_canvas_prefix(source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, completed + 1));
                crate::proofs::production_impl::production_composition_prefix_observation(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                    crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, completed + 1,
                );
            }
            let pixel_work = (output_colors.len() as u64)
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            #[allow(
                clippy::question_mark,
                reason = "The overflow branch proves the exact stored-but-uncharged state"
            )]
            let next_work = match work_units.checked_add(pixel_work) {
                Some(value) => value,
                None => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert((Err(crate::specs::part2_composition_error(true)), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                            == crate::specs::part2_composition_budgeted_prefix(
                                source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                                initial_work as nat, maximum_work_units, completed + 1,
                            ));
                        crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                        );
                    }
                    return Err(arithmetic_overflow());
                }
            };
            *work_units = next_work;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(pixel_work as nat == initial_colors.len() + 1);
                assert((completed + 1) * (initial_colors.len() + 1)
                    == completed * (initial_colors.len() + 1) + (initial_colors.len() + 1)) by(nonlinear_arith);
                assert(*work_units as nat == initial_work as nat + (completed + 1) * (initial_colors.len() + 1));
                crate::proofs::production_impl::production_composition_prefix_observation(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                    crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, completed + 1,
                );
            }
            #[allow(
                clippy::question_mark,
                reason = "The rejection branch proves the exact charged failure state"
            )]
            if let Err(error) = enforce_limit(Resource::WorkUnits, maximum_work_units, *work_units)
            {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert((Err(error), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                        == crate::specs::part2_composition_budgeted_prefix(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1,
                        ));
                    crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                    );
                }
                return Err(error);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert((Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                    == crate::specs::part2_composition_budgeted_prefix(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, completed + 1,
                    ));
            }
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert((y as nat) * (copy_width as nat) + (copy_width as nat)
                == ((y as nat) + 1) * (copy_width as nat)) by(nonlinear_arith);
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((copy_height as nat) * (copy_width as nat)
            == (copy_width as nat) * (copy_height as nat)) by(nonlinear_arith);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn composite_layer(
    canvas_width: u32,
    canvas_height: u32,
    offset_x: u32,
    offset_y: u32,
    source: &LayerChannels<'_>,
    output_colors: &mut [Vec<u16>],
    output_alpha: &mut [u16],
    work_units: &mut u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let copy_width = source.width.min(canvas_width.saturating_sub(offset_x));
    let copy_height = source.height.min(canvas_height.saturating_sub(offset_y));
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= copy_height,
            decreases copy_height - y,
        )
    )]
    while y < copy_height {
        let mut x = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant x <= copy_width,
                decreases copy_width - x,
            )
        )]
        while x < copy_width {
            let source_index = index_of(source.width, x, y)?;
            let destination_index = index_of(
                canvas_width,
                offset_x.checked_add(x).ok_or_else(arithmetic_overflow)?,
                offset_y.checked_add(y).ok_or_else(arithmetic_overflow)?,
            )?;
            let (source_alpha, premultiplied) = match source.alpha.as_ref() {
                Some((samples, premultiplied)) => (
                    *samples.get(source_index).ok_or_else(invalid_layer)?,
                    *premultiplied,
                ),
                None => (u16::MAX, false),
            };
            let destination_alpha = *output_alpha
                .get(destination_index)
                .ok_or_else(invalid_layer)?;
            let output_alpha_numerator =
                crate::refinement::source_over_alpha_numerator(source_alpha, destination_alpha);
            let mut color_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant color_index <= source.colors.len(),
                    decreases source.colors.len() - color_index,
                )
            )]
            while color_index < source.colors.len() {
                let source_sample = *source
                    .colors
                    .get(color_index)
                    .and_then(|samples| samples.get(source_index))
                    .ok_or_else(invalid_layer)?;
                if premultiplied && source_sample > source_alpha {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
                }
                let destination_sample = *output_colors
                    .get(color_index)
                    .and_then(|samples| samples.get(destination_index))
                    .ok_or_else(invalid_layer)?;
                let color_numerator = crate::refinement::source_over_color_numerator(
                    source_sample,
                    destination_sample,
                    source_alpha,
                    destination_alpha,
                    premultiplied,
                );
                let output_sample = crate::refinement::source_over_rounded_ratio(
                    color_numerator,
                    output_alpha_numerator,
                );
                let destination_samples = output_colors
                    .get_mut(color_index)
                    .ok_or_else(invalid_layer)?;
                let sample = u16::try_from(output_sample).map_err(|_| arithmetic_overflow())?;
                if !crate::refinement::try_store_composition_sample(
                    destination_samples,
                    destination_index,
                    sample,
                ) {
                    return Err(invalid_layer());
                }
                color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            let alpha = crate::refinement::source_over_rounded_ratio(
                output_alpha_numerator,
                u64::from(u16::MAX),
            );
            let sample = u16::try_from(alpha).map_err(|_| arithmetic_overflow())?;
            if !crate::refinement::try_store_composition_sample(
                output_alpha,
                destination_index,
                sample,
            ) {
                return Err(invalid_layer());
            }
            *work_units = work_units
                .checked_add(
                    (source.colors.len() as u64)
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?,
                )
                .ok_or_else(arithmetic_overflow)?;
            enforce_limit(
                Resource::WorkUnits,
                limits.maximum_work_units(),
                *work_units,
            )?;
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(index =>
        ensures index.ok() == crate::specs::part2_composition_pixel_index(width, x, y),
            index.is_err() ==> index.err().unwrap() == crate::specs::part2_composition_error(true),
    )
)]
fn index_of(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    crate::refinement::composition_pixel_index(width, x, y).ok_or_else(arithmetic_overflow)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(area =>
        ensures area == crate::specs::part2_layer_area(width, height),
    )
)]
fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_layer_area);
        vstd::arithmetic::mul::lemma_mul_inequality(width as int, 4294967295, height as int);
        vstd::arithmetic::mul::lemma_mul_inequality(height as int, 4294967295, 4294967295);
        assert((width as int) * (height as int) <= 18446744065119617025) by(nonlinear_arith);
    }
    let area = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(arithmetic_overflow)?;
    match usize::try_from(area) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output.is_ok(), output.ok().unwrap()@ == input@,
    )
)]
fn copy_samples(input: &[u16]) -> Result<Vec<u16>, DecodeError> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= input.len(), output@ == input@.take(index as int),
            decreases input.len() - index,
        )
    )]
    while index < input.len() {
        output.push(*input.get(index).ok_or_else(arithmetic_overflow)?);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(output)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == if actual > limit {
            Err(DecodeError::LimitExceeded { resource, limit, actual })
        } else { Ok(()) },
    )
)]
fn enforce_limit(resource: Resource, limit: u64, actual: u64) -> Result<(), DecodeError> {
    if actual > limit {
        Err(DecodeError::LimitExceeded {
            resource,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error => ensures error == crate::specs::part2_composition_error(true),)
)]
fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error => ensures error == crate::specs::part2_composition_error(false),)
)]
fn invalid_layer() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidBoxContents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jp2::{Composition, CompositionInstructionSet};
    use crate::refinement::{
        compose_planar_pixel, gather_composition_pixel, scatter_composition_pixel,
        source_over_alpha_numerator, source_over_color_numerator, source_over_rounded_ratio,
    };

    #[test]
    fn chroma_key_preserves_raw_planes_and_replaces_alpha_at_exact_matches() {
        let mut cases = 0;
        for space in [ConvertedColorSpace::Grayscale, ConvertedColorSpace::Srgb] {
            for width in 0..=2 {
                for height in 0..=2 {
                    let pixels = usize::try_from(width * height).unwrap();
                    for channels in 0..=3 {
                        for missing in 0..=channels {
                            let colors = (0..channels)
                                .map(|channel| {
                                    (0..pixels.saturating_sub(usize::from(channel < missing)))
                                        .map(|pixel| if pixel % 2 == 0 { 17 } else { u16::MAX })
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();
                            for key in [0, 17, u16::MAX] {
                                for key_count in 0..=channels + 1 {
                                    let keys = vec![key; key_count];
                                    for prior_alpha in [
                                        None,
                                        Some((vec![], false)),
                                        Some((vec![123, 456, 789], true)),
                                    ] {
                                        let mut layer = OwnedLayer {
                                            width,
                                            height,
                                            color_space: space,
                                            colors: colors.clone(),
                                            alpha: prior_alpha.clone(),
                                        };
                                        let result = apply_chroma_key(&mut layer, &keys);
                                        assert_eq!(layer.width, width);
                                        assert_eq!(layer.height, height);
                                        assert_eq!(layer.color_space, space);
                                        assert_eq!(layer.colors, colors);
                                        if key_count != channels {
                                            assert_eq!(result, Err(invalid_layer()));
                                            assert_eq!(layer.alpha, prior_alpha);
                                        } else {
                                            result.unwrap();
                                            let transparent = (0..pixels)
                                                .filter(|&pixel| {
                                                    colors
                                                        .iter()
                                                        .all(|plane| plane.get(pixel) == Some(&key))
                                                })
                                                .collect::<std::collections::BTreeSet<_>>();
                                            let expected = (0..pixels)
                                                .map(|pixel| {
                                                    if transparent.contains(&pixel) {
                                                        0
                                                    } else {
                                                        u16::MAX
                                                    }
                                                })
                                                .collect::<Vec<_>>();
                                            assert_eq!(layer.alpha, Some((expected, false)));
                                        }
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 6480);
    }

    #[test]
    fn chroma_key_retains_zero_geometry_and_all_u16_key_values() {
        for sample in 0..=u16::MAX {
            for key in [sample, sample.wrapping_add(1)] {
                let mut layer = OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Srgb,
                    colors: vec![vec![sample]],
                    alpha: Some((vec![42], true)),
                };
                apply_chroma_key(&mut layer, &[key]).unwrap();
                assert_eq!(
                    layer.alpha,
                    Some((vec![if sample == key { 0 } else { u16::MAX }], false))
                );
                assert_eq!(layer.colors, vec![vec![sample]]);
            }
        }
        let mut layer = OwnedLayer {
            width: u32::MAX,
            height: 0,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![vec![]],
            alpha: Some((vec![1, 2], true)),
        };
        apply_chroma_key(&mut layer, &[0]).unwrap();
        assert_eq!(layer.alpha, Some((vec![], false)));
        layer.height = u32::MAX;
        assert_eq!(apply_chroma_key(&mut layer, &[]), Err(invalid_layer()));
        assert_eq!(layer.alpha, Some((vec![], false)));
    }

    fn visit_channel_permutations(
        order: &mut [usize],
        fixed: usize,
        visit: &mut impl FnMut(&[usize]),
    ) {
        if fixed == order.len() {
            visit(order);
            return;
        }
        for index in fixed..order.len() {
            order.swap(fixed, index);
            visit_channel_permutations(order, fixed + 1, visit);
            order.swap(fixed, index);
        }
    }

    #[test]
    fn layer_input_permutations_preserve_complete_planes_and_metadata() {
        let expected_colors = [vec![101, 103], vec![211, 223], vec![307, 311]];
        let first_alpha = vec![1_u16, 40_001];
        let second_alpha = vec![32_768_u16, 50_003];
        let expected_alpha = first_alpha
            .iter()
            .zip(&second_alpha)
            .map(|(&first, &second)| {
                u16::try_from((u128::from(first) * u128::from(second) + 32_767) / 65_535).unwrap()
            })
            .collect::<Vec<_>>();
        let mut cases = 0;
        for association in [
            ChannelAssociation::WholeImage,
            ChannelAssociation::Color(1),
            ChannelAssociation::Unassociated,
        ] {
            for first_premultiplied in [false, true] {
                for second_premultiplied in [false, true] {
                    let channels = [
                        (ConvertedChannelType::Red, expected_colors[0].clone()),
                        (ConvertedChannelType::Green, expected_colors[1].clone()),
                        (ConvertedChannelType::Blue, expected_colors[2].clone()),
                        (
                            ConvertedChannelType::Alpha {
                                premultiplied: first_premultiplied,
                                association,
                            },
                            first_alpha.clone(),
                        ),
                        (
                            ConvertedChannelType::Alpha {
                                premultiplied: second_premultiplied,
                                association,
                            },
                            second_alpha.clone(),
                        ),
                    ];
                    visit_channel_permutations(&mut [0, 1, 2, 3, 4], 0, &mut |order| {
                        let image = ConvertedImage::from_test_channels(
                            2,
                            1,
                            ConvertedColorSpace::Srgb,
                            order.iter().map(|&index| channels[index].clone()).collect(),
                        );
                        let borrowed = layer_channels(&image).unwrap();
                        assert_eq!((borrowed.width, borrowed.height), (2, 1));
                        assert_eq!(
                            borrowed.colors,
                            expected_colors
                                .iter()
                                .map(Vec::as_slice)
                                .collect::<Vec<_>>(),
                        );
                        assert_eq!(borrowed.alpha, Some((expected_alpha.clone(), false)));
                        let owned = owned_layer_from_image(&image).unwrap();
                        drop(image);
                        assert_eq!((owned.width, owned.height), (2, 1));
                        assert_eq!(owned.color_space, ConvertedColorSpace::Srgb);
                        assert_eq!(owned.colors, expected_colors);
                        assert_eq!(owned.alpha, Some((expected_alpha.clone(), false)));
                        cases += 1;
                    });
                }
            }
        }
        assert_eq!(cases, 1_440);
    }

    #[test]
    fn layer_input_absent_and_single_alpha_preserve_representation_and_color_aliases() {
        for space in [ConvertedColorSpace::Grayscale, ConvertedColorSpace::Srgb] {
            for first_kind in [ConvertedChannelType::Gray, ConvertedChannelType::Red] {
                let mut colors = vec![(first_kind, vec![17, 19])];
                if space == ConvertedColorSpace::Srgb {
                    colors.push((ConvertedChannelType::Blue, vec![29, 31]));
                    colors.push((ConvertedChannelType::Green, vec![23, 27]));
                }
                let image = ConvertedImage::from_test_channels(2, 1, space, colors.clone());
                let absent = owned_layer_from_image(&image).unwrap();
                assert_eq!(absent.color_space, space);
                assert_eq!(absent.alpha, None);
                let expected = if space == ConvertedColorSpace::Srgb {
                    vec![vec![17, 19], vec![23, 27], vec![29, 31]]
                } else {
                    vec![vec![17, 19]]
                };
                assert_eq!(absent.colors, expected);
                for premultiplied in [false, true] {
                    for position in 0..=colors.len() {
                        let mut with_alpha = colors.clone();
                        with_alpha.insert(
                            position,
                            (
                                ConvertedChannelType::Alpha {
                                    premultiplied,
                                    association: ChannelAssociation::WholeImage,
                                },
                                vec![0, u16::MAX],
                            ),
                        );
                        let image = ConvertedImage::from_test_channels(2, 1, space, with_alpha);
                        let layer = owned_layer_from_image(&image).unwrap();
                        assert_eq!(layer.colors, expected);
                        assert_eq!(layer.alpha, Some((vec![0, u16::MAX], premultiplied)));
                    }
                }
            }
        }
    }

    #[test]
    fn layer_input_alpha_combination_rounds_each_ordered_step() {
        let construct = |values: &[u16]| {
            let mut channels = vec![(ConvertedChannelType::Gray, vec![0])];
            channels.extend(values.iter().map(|&value| {
                (
                    ConvertedChannelType::Alpha {
                        premultiplied: true,
                        association: ChannelAssociation::WholeImage,
                    },
                    vec![value],
                )
            }));
            ConvertedImage::from_test_channels(1, 1, ConvertedColorSpace::Grayscale, channels)
        };
        let first = owned_layer_from_image(&construct(&[1, 32_768, 32_768])).unwrap();
        let second = owned_layer_from_image(&construct(&[32_768, 32_768, 1])).unwrap();
        assert_eq!(first.alpha, Some((vec![1], false)));
        assert_eq!(second.alpha, Some((vec![0], false)));
    }

    #[test]
    fn layer_input_rejects_missing_duplicate_out_of_range_and_malformed_planes() {
        let alpha = ConvertedChannelType::Alpha {
            premultiplied: false,
            association: ChannelAssociation::WholeImage,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidBoxContents,
        };
        let cases = [
            (ConvertedColorSpace::Grayscale, vec![]),
            (ConvertedColorSpace::Grayscale, vec![(alpha, vec![1, 2])]),
            (
                ConvertedColorSpace::Grayscale,
                vec![
                    (ConvertedChannelType::Gray, vec![1, 2]),
                    (ConvertedChannelType::Red, vec![3, 4]),
                ],
            ),
            (
                ConvertedColorSpace::Grayscale,
                vec![(ConvertedChannelType::Green, vec![1, 2])],
            ),
            (
                ConvertedColorSpace::Grayscale,
                vec![(ConvertedChannelType::Blue, vec![1, 2])],
            ),
            (
                ConvertedColorSpace::Grayscale,
                vec![(ConvertedChannelType::Gray, vec![1])],
            ),
            (
                ConvertedColorSpace::Grayscale,
                vec![(ConvertedChannelType::Gray, vec![1, 2, 3])],
            ),
            (
                ConvertedColorSpace::Grayscale,
                vec![(ConvertedChannelType::Gray, vec![1, 2]), (alpha, vec![])],
            ),
            (
                ConvertedColorSpace::Grayscale,
                vec![
                    (ConvertedChannelType::Gray, vec![1, 2]),
                    (alpha, vec![1, 2]),
                    (alpha, vec![1]),
                ],
            ),
            (
                ConvertedColorSpace::Srgb,
                vec![
                    (ConvertedChannelType::Red, vec![1, 2]),
                    (ConvertedChannelType::Green, vec![3, 4]),
                ],
            ),
            (
                ConvertedColorSpace::Srgb,
                vec![
                    (ConvertedChannelType::Red, vec![1, 2]),
                    (ConvertedChannelType::Green, vec![3]),
                    (ConvertedChannelType::Blue, vec![5, 6]),
                ],
            ),
        ];
        for (space, channels) in cases {
            let image = ConvertedImage::from_test_channels(2, 1, space, channels);
            assert_eq!(layer_channels(&image).err(), Some(invalid));
            assert_eq!(owned_layer_from_image(&image).err(), Some(invalid));
        }
        let empty = ConvertedImage::from_test_channels(
            0,
            u32::MAX,
            ConvertedColorSpace::Grayscale,
            vec![(ConvertedChannelType::Gray, vec![]), (alpha, vec![])],
        );
        let layer = owned_layer_from_image(&empty).unwrap();
        assert_eq!((layer.width, layer.height), (0, u32::MAX));
        assert_eq!(layer.colors, vec![Vec::<u16>::new()]);
        assert_eq!(layer.alpha, Some((vec![], false)));
    }

    #[test]
    fn normalized_alpha_products_match_wide_integer_reference_at_boundaries() {
        for first in 0..=u16::MAX {
            for second in [0, 1, 32_767, 32_768, 65_534, u16::MAX] {
                let expected =
                    u16::try_from((u128::from(first) * u128::from(second) + 32_767) / 65_535)
                        .unwrap();
                assert_eq!(
                    crate::refinement::normalized_sample_product(first, second),
                    Some(expected),
                );
            }
        }
    }

    #[test]
    fn complete_planar_transition_matches_independent_read_rational_and_store_oracle() {
        let shapes: [&[usize]; 8] = [
            &[],
            &[0],
            &[1],
            &[2],
            &[2, 1, 2],
            &[2, 2, 0],
            &[2, 2, 2],
            &[2, 2, 2, 2],
        ];
        let planes = |shape: &[usize], base: u16| {
            shape
                .iter()
                .enumerate()
                .map(|(channel, &len)| {
                    (0..len)
                        .map(|i| base + u16::try_from(10 * channel + i).unwrap())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let alphas = [
            None,
            Some((vec![], false)),
            Some((vec![], true)),
            Some((vec![41], false)),
            Some((vec![41], true)),
            Some((vec![41, 43], false)),
            Some((vec![41, 43], true)),
            Some((vec![0, 1], false)),
            Some((vec![0, 1], true)),
            Some((vec![32_768, u16::MAX], false)),
            Some((vec![32_768, u16::MAX], true)),
            Some((vec![u16::MAX, u16::MAX], false)),
            Some((vec![u16::MAX, u16::MAX], true)),
        ];
        let spaces = [ConvertedColorSpace::Grayscale, ConvertedColorSpace::Srgb];
        let mut cases = 0;
        for source_shape in shapes {
            for destination_shape in shapes {
                for source_alpha in &alphas {
                    for alpha_length in 0..=2 {
                        for source_space in spaces {
                            for output_space in spaces {
                                let source = OwnedLayer {
                                    width: 2,
                                    height: 1,
                                    color_space: source_space,
                                    colors: planes(source_shape, 100),
                                    alpha: source_alpha.clone(),
                                };
                                let colors = planes(destination_shape, 200);
                                let alpha = vec![47; alpha_length];
                                for source_index in [0, 1, 2, usize::MAX] {
                                    for destination_index in [0, 1, 2, usize::MAX] {
                                        let expected = (|| {
                                            if colors.len() > 3 {
                                                return None;
                                            }
                                            let mut input = [0; 3];
                                            let mut destination = [0; 3];
                                            for (channel, samples) in colors.iter().enumerate() {
                                                let selected = if output_space
                                                    == ConvertedColorSpace::Srgb
                                                    && source_space
                                                        == ConvertedColorSpace::Grayscale
                                                {
                                                    0
                                                } else {
                                                    channel
                                                };
                                                input[channel] = *source
                                                    .colors
                                                    .get(selected)?
                                                    .get(source_index)?;
                                                destination[channel] =
                                                    *samples.get(destination_index)?;
                                            }
                                            let (a, premultiplied) = match &source.alpha {
                                                Some((samples, flag)) => {
                                                    (*samples.get(source_index)?, *flag)
                                                }
                                                None => (u16::MAX, false),
                                            };
                                            Some((
                                                input,
                                                destination,
                                                a,
                                                *alpha.get(destination_index)?,
                                                premultiplied,
                                            ))
                                        })();
                                        assert_eq!(
                                            gather_composition_pixel(
                                                &source.colors,
                                                source.alpha.as_ref().map(|(samples, flag)| (
                                                    samples.as_slice(),
                                                    *flag
                                                )),
                                                &colors,
                                                &alpha,
                                                source_index,
                                                destination_index,
                                                output_space == ConvertedColorSpace::Srgb
                                                    && source_space
                                                        == ConvertedColorSpace::Grayscale,
                                            ),
                                            expected
                                        );
                                        let mut expected_colors = colors.clone();
                                        let mut expected_alpha = alpha.clone();
                                        let composed = match expected {
                                            Some((s, d, sa, da, premultiplied))
                                                if !premultiplied
                                                    || s[..colors.len()]
                                                        .iter()
                                                        .all(|&sample| sample <= sa) =>
                                            {
                                                let maximum = u128::from(u16::MAX);
                                                let sa = u128::from(sa);
                                                let da = u128::from(da);
                                                let denominator =
                                                    sa * maximum + da * (maximum - sa);
                                                for (channel, samples) in
                                                    expected_colors.iter_mut().enumerate()
                                                {
                                                    let source = u128::from(s[channel]);
                                                    let numerator = if premultiplied {
                                                        source * maximum * maximum
                                                    } else {
                                                        source * sa * maximum
                                                    } + u128::from(d[channel])
                                                        * da
                                                        * (maximum - sa);
                                                    samples[destination_index] =
                                                        u16::try_from(if denominator == 0 {
                                                            0
                                                        } else {
                                                            (numerator + denominator / 2)
                                                                / denominator
                                                        })
                                                        .unwrap();
                                                }
                                                expected_alpha[destination_index] = u16::try_from(
                                                    (denominator + maximum / 2) / maximum,
                                                )
                                                .unwrap();
                                                true
                                            }
                                            _ => false,
                                        };
                                        let mut actual_colors = colors.clone();
                                        let mut actual_alpha = alpha.clone();
                                        let actual = compose_planar_pixel(
                                            &source.colors,
                                            source
                                                .alpha
                                                .as_ref()
                                                .map(|(samples, flag)| (samples.as_slice(), *flag)),
                                            &mut actual_colors,
                                            &mut actual_alpha,
                                            source_index,
                                            destination_index,
                                            output_space == ConvertedColorSpace::Srgb
                                                && source_space == ConvertedColorSpace::Grayscale,
                                        );
                                        assert_eq!(actual, composed);
                                        assert_eq!(actual_colors, expected_colors);
                                        assert_eq!(actual_alpha, expected_alpha);
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 159_744);
    }

    #[test]
    fn planar_scatter_matches_exact_success_and_partial_failure_states() {
        let shapes: [&[usize]; 8] = [
            &[],
            &[0],
            &[1],
            &[2],
            &[2, 1, 2],
            &[2, 2, 0],
            &[2, 2, 2],
            &[2, 2, 2, 2],
        ];
        let mut cases = 0;
        for shape in shapes {
            for alpha_length in 0..=2 {
                for index in [0, 1, 2, usize::MAX] {
                    for pixel in [[0; 3], [u16::MAX; 3], [200, 210, 220]] {
                        for alpha in [0, 47, u16::MAX] {
                            let mut colors = shape
                                .iter()
                                .enumerate()
                                .map(|(channel, &len)| {
                                    (0..len)
                                        .map(|i| 200 + u16::try_from(channel * 10 + i).unwrap())
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();
                            let mut alphas = vec![47; alpha_length];
                            let mut expected_colors = colors.clone();
                            let mut expected_alpha = alphas.clone();
                            let mut stored = colors.len() <= 3;
                            if stored {
                                for (channel, samples) in expected_colors.iter_mut().enumerate() {
                                    match samples.get_mut(index) {
                                        Some(sample) => *sample = pixel[channel],
                                        None => {
                                            stored = false;
                                            break;
                                        }
                                    }
                                }
                                if stored {
                                    match expected_alpha.get_mut(index) {
                                        Some(sample) => *sample = alpha,
                                        None => stored = false,
                                    }
                                }
                            }
                            assert_eq!(
                                scatter_composition_pixel(
                                    &mut colors,
                                    &mut alphas,
                                    index,
                                    pixel,
                                    alpha,
                                ),
                                stored
                            );
                            assert_eq!(colors, expected_colors);
                            assert_eq!(alphas, expected_alpha);
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 864);
    }

    #[test]
    fn missing_late_planar_sample_preserves_current_pixel_and_work_order() {
        for missing_source in [false, true] {
            let mut source = OwnedLayer {
                width: 2,
                height: 1,
                color_space: ConvertedColorSpace::Srgb,
                colors: vec![vec![101, 201], vec![102, 202], vec![103, 203]],
                alpha: None,
            };
            let mut colors = vec![vec![0, 17], vec![0, 23], vec![0, 29]];
            if missing_source {
                source.colors[2].truncate(1);
            } else {
                colors[2].truncate(1);
            }
            let mut expected = colors.clone();
            for (channel, samples) in expected.iter_mut().enumerate() {
                samples[0] = 101 + u16::try_from(channel).unwrap();
            }
            let instruction = JpxInstructionInfo {
                layer_index: 0,
                target_x: 0,
                target_y: 0,
                target_width: 2,
                target_height: 1,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                persistent: true,
                next_reuse: 0,
            };
            let mut alpha = vec![0, 31];
            let mut work = 0;
            assert_eq!(
                composite_placed_layer(
                    2,
                    1,
                    ConvertedColorSpace::Srgb,
                    &source,
                    instruction,
                    &mut colors,
                    &mut alpha,
                    &mut work,
                    &ResourceLimits::default(),
                ),
                Err(invalid_layer())
            );
            assert_eq!(colors, expected);
            assert_eq!(alpha, [u16::MAX, 31]);
            assert_eq!(work, 4);
        }
    }

    #[test]
    fn complete_pixel_stage_matches_independent_rational_rounding() {
        let maximum = u128::from(u16::MAX);
        let nearest = |n: u128, d: u128| {
            if d == 0 {
                0
            } else {
                u16::try_from(n / d + u128::from(2 * (n % d) >= d)).unwrap()
            }
        };
        for channels in 0..=4 {
            for premultiplied in [false, true] {
                for source_alpha in [0, 1, 32_767, 32_768, u16::MAX] {
                    for destination_alpha in [0, 1, 32_767, 32_768, u16::MAX] {
                        for source in [[0, source_alpha / 2, source_alpha], [u16::MAX, 1, 32_768]] {
                            for destination in [[0, 1, u16::MAX], [u16::MAX, 32_768, 0]] {
                                let actual = crate::refinement::source_over_pixel(
                                    source,
                                    destination,
                                    source_alpha,
                                    destination_alpha,
                                    premultiplied,
                                    channels,
                                );
                                if channels > 3
                                    || (premultiplied
                                        && source[..channels]
                                            .iter()
                                            .any(|value| *value > source_alpha))
                                {
                                    assert_eq!(actual, None);
                                    continue;
                                }
                                let a = u128::from(source_alpha);
                                let d = u128::from(destination_alpha);
                                let alpha_numerator = a * maximum + d * (maximum - a);
                                let mut expected = destination;
                                for i in 0..channels {
                                    let s = u128::from(source[i]);
                                    let c = u128::from(destination[i]);
                                    let source_term = if premultiplied {
                                        s * maximum * maximum
                                    } else {
                                        s * a * maximum
                                    };
                                    expected[i] = nearest(
                                        source_term + c * d * (maximum - a),
                                        alpha_numerator,
                                    );
                                }
                                assert_eq!(
                                    actual,
                                    Some((expected, nearest(alpha_numerator, maximum)))
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_late_channel_rejects_only_the_current_pixel_before_any_store() {
        let source = OwnedLayer {
            width: 2,
            height: 1,
            color_space: ConvertedColorSpace::Srgb,
            colors: vec![vec![32_768, 32_768], vec![0, 1], vec![16_384, 32_769]],
            alpha: Some((vec![32_768, 32_768], true)),
        };
        let instruction = JpxInstructionInfo {
            layer_index: 0,
            target_x: 0,
            target_y: 0,
            target_width: 2,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            persistent: true,
            next_reuse: 0,
        };
        let mut colors = vec![vec![0, 17], vec![0, 23], vec![0, 29]];
        let mut alpha = vec![0, 31];
        let mut work = 0;
        assert_eq!(
            composite_placed_layer(
                2,
                1,
                ConvertedColorSpace::Srgb,
                &source,
                instruction,
                &mut colors,
                &mut alpha,
                &mut work,
                &ResourceLimits::default()
            ),
            Err(invalid_layer())
        );
        assert_eq!(colors, [vec![u16::MAX, 17], vec![0, 23], vec![32_768, 29]]);
        assert_eq!(alpha, [32_768, 31]);
        assert_eq!(work, 4);
    }

    #[test]
    fn composition_mapped_offsets_match_wide_floor_division() {
        let check = |position: u32, source: u32, target: u32| {
            let mapped = crate::refinement::composition_mapped_offset(position, source, target);
            let numerator = u128::from(position) * u128::from(source);
            let denominator = u128::from(target);
            assert_eq!(u128::from(mapped), numerator / denominator);
            assert!(u128::from(mapped) * denominator <= numerator);
            assert!(numerator < (u128::from(mapped) + 1) * denominator);
            if position < target && source > 0 {
                assert!(mapped < u64::from(source));
            }
        };
        for source in 0..=16 {
            for target in 1..=16 {
                for position in 0..=target + 1 {
                    check(position, source, target);
                }
            }
        }
        for position in [0, 1, 2, 65_535, 65_536, u32::MAX - 1, u32::MAX] {
            for source in [0, 1, 2, 65_535, 65_536, u32::MAX - 1, u32::MAX] {
                for target in [1, 2, 65_535, 65_536, u32::MAX] {
                    check(position, source, target);
                }
            }
        }
    }

    #[test]
    fn composition_pixel_indices_match_wide_row_major_arithmetic() {
        for width in [0, 1, 2, 65_535, 65_536, u32::MAX - 1, u32::MAX] {
            for x in [0, 1, 2, 65_535, 65_536, u32::MAX - 1, u32::MAX] {
                for y in [0, 1, 2, 65_535, 65_536, u32::MAX - 1, u32::MAX] {
                    let wide = u128::from(y) * u128::from(width) + u128::from(x);
                    assert!(wide <= u128::from(u64::MAX));
                    let expected = usize::try_from(wide).ok();
                    assert_eq!(
                        crate::refinement::composition_pixel_index(width, x, y),
                        expected
                    );
                    let actual = index_of(width, x, y);
                    assert_eq!(actual.as_ref().ok().copied(), expected);
                    if expected.is_none() {
                        assert_eq!(actual, Err(arithmetic_overflow()));
                    }
                }
            }
        }
        for width in 1..=16 {
            for height in 1..=16 {
                for y in 0..height {
                    for x in 0..width {
                        let index = index_of(width, x, y).unwrap();
                        assert!((index as u128) < u128::from(width) * u128::from(height));
                        assert_eq!(
                            u32::try_from(index / usize::try_from(width).unwrap()).unwrap(),
                            y
                        );
                        assert_eq!(
                            u32::try_from(index % usize::try_from(width).unwrap()).unwrap(),
                            x
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn placed_composition_scaled_crop_and_clipping_match_coordinate_oracle() {
        let source_values: Vec<u16> = (0..20).map(|i| i * 1999 + 31).collect();
        let source = OwnedLayer {
            width: 5,
            height: 4,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![source_values.clone()],
            alpha: None,
        };
        let base = JpxInstructionInfo {
            layer_index: 0,
            target_x: 4,
            target_y: 3,
            target_width: 6,
            target_height: 4,
            source_x: 1,
            source_y: 1,
            source_width: 3,
            source_height: 2,
            persistent: true,
            next_reuse: 0,
        };
        for (instruction, crop) in [
            (base, [1_u32, 1, 3, 2]),
            (
                JpxInstructionInfo {
                    source_x: 4,
                    source_y: 3,
                    source_width: 99,
                    source_height: 99,
                    target_width: u32::MAX,
                    target_height: u32::MAX,
                    ..base
                },
                [4, 3, 1, 1],
            ),
            (
                JpxInstructionInfo {
                    source_y: 0,
                    source_width: 0,
                    source_height: 0,
                    target_x: 0,
                    target_y: 0,
                    target_width: 2,
                    target_height: 2,
                    ..base
                },
                [1, 0, 4, 4],
            ),
            (
                JpxInstructionInfo {
                    source_x: 0,
                    source_y: 0,
                    source_width: 0,
                    source_height: 0,
                    target_x: 1,
                    target_y: 1,
                    target_width: 5,
                    target_height: 4,
                    ..base
                },
                [0, 0, 5, 4],
            ),
        ] {
            let before: Vec<Vec<u16>> = (0..3)
                .map(|channel| (0..42).map(|i| 10_000 + channel * 1000 + i).collect())
                .collect();
            let before_alpha: Vec<u16> = (0..42).map(|i| 3 + i * 7).collect();
            let mut colors = before.clone();
            let mut alpha = before_alpha.clone();
            let mut work = 7;
            composite_placed_layer(
                7,
                6,
                ConvertedColorSpace::Srgb,
                &source,
                instruction,
                &mut colors,
                &mut alpha,
                &mut work,
                &ResourceLimits::default(),
            )
            .unwrap();
            let mut copied = 0;
            for y in 0..6_u32 {
                for x in 0..7_u32 {
                    let index = usize::try_from(y * 7 + x).unwrap();
                    let covered = x >= instruction.target_x
                        && y >= instruction.target_y
                        && x - instruction.target_x < instruction.target_width
                        && y - instruction.target_y < instruction.target_height;
                    if covered {
                        let sx = u128::from(crop[0])
                            + u128::from(x - instruction.target_x) * u128::from(crop[2])
                                / u128::from(instruction.target_width);
                        let sy = u128::from(crop[1])
                            + u128::from(y - instruction.target_y) * u128::from(crop[3])
                                / u128::from(instruction.target_height);
                        let sample = source_values[usize::try_from(sy * 5 + sx).unwrap()];
                        for samples in &colors {
                            assert_eq!(samples[index], sample);
                        }
                        assert_eq!(alpha[index], u16::MAX);
                        copied += 1;
                    } else {
                        for (samples, previous) in colors.iter().zip(&before) {
                            assert_eq!(samples[index], previous[index]);
                        }
                        assert_eq!(alpha[index], before_alpha[index]);
                    }
                }
            }
            assert_eq!(work, 7 + copied * 4);
        }
    }

    #[test]
    fn placed_layer_zero_width_clip_does_not_traverse_unvisited_rows() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let source = OwnedLayer {
                width: 1,
                height: 1,
                color_space: ConvertedColorSpace::Grayscale,
                colors: vec![],
                alpha: Some((vec![], true)),
            };
            let instruction = JpxInstructionInfo {
                layer_index: 0,
                target_x: 1,
                target_y: 0,
                target_width: 1,
                target_height: u32::MAX,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                persistent: true,
                next_reuse: 0,
            };
            let mut colors = vec![vec![17], vec![], vec![23, 29], vec![31]];
            let mut alpha = vec![];
            let mut work = u64::MAX;
            let result = composite_placed_layer(
                1,
                u32::MAX,
                ConvertedColorSpace::Srgb,
                &source,
                instruction,
                &mut colors,
                &mut alpha,
                &mut work,
                &ResourceLimits::default()
                    .with_maximum_work_units(0)
                    .unwrap(),
            );
            sender.send((result, colors, alpha, work)).unwrap();
        });
        let (result, colors, alpha, work) = receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("an empty clipped rectangle must not traverse unvisited rows");
        worker.join().unwrap();
        assert_eq!(result, Ok(()));
        assert_eq!(colors, [vec![17], vec![], vec![23, 29], vec![31]]);
        assert!(alpha.is_empty());
        assert_eq!(work, u64::MAX);
    }

    #[test]
    fn placed_layer_work_rejections_preserve_post_store_order() {
        let source = OwnedLayer {
            width: 1,
            height: 1,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![vec![2_345]],
            alpha: None,
        };
        let instruction = JpxInstructionInfo {
            layer_index: 0,
            target_x: 0,
            target_y: 0,
            target_width: 1,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            persistent: true,
            next_reuse: 0,
        };
        for (initial, limit, expected, final_work) in [
            (u64::MAX, 7, Err(arithmetic_overflow()), u64::MAX),
            (5, 6, Err(DecodeError::limit(Resource::WorkUnits, 6, 7)), 7),
            (5, 7, Ok(()), 7),
        ] {
            let mut colors = vec![vec![111]];
            let mut alpha = vec![0];
            let mut work = initial;
            assert_eq!(
                composite_placed_layer(
                    1,
                    1,
                    ConvertedColorSpace::Grayscale,
                    &source,
                    instruction,
                    &mut colors,
                    &mut alpha,
                    &mut work,
                    &ResourceLimits::default()
                        .with_maximum_work_units(limit)
                        .unwrap(),
                ),
                expected
            );
            assert_eq!(colors, [vec![2_345]]);
            assert_eq!(alpha, [u16::MAX]);
            assert_eq!(work, final_work);
        }
    }

    #[test]
    fn placed_layer_exact_error_precedence_matches_independent_replay() {
        let mut cases = 0;
        for initial_work in [0, 9, u64::MAX - 4, u64::MAX - 3, u64::MAX] {
            for missing_at in 0..=6 {
                for missing_alpha in [false, true] {
                    for limit in [
                        0, 3, 4, 8, 9, 12, 13, 16, 17, 20, 21, 24, 25, 28, 29, 32, 33, 37,
                    ] {
                        let mut source = OwnedLayer {
                            width: 3,
                            height: 2,
                            color_space: ConvertedColorSpace::Srgb,
                            colors: (0..3)
                                .map(|channel| {
                                    (0..6)
                                        .map(|i| u16::try_from((channel + 1) * 100 + i).unwrap())
                                        .collect::<Vec<_>>()
                                })
                                .collect(),
                            alpha: if missing_alpha {
                                Some((vec![u16::MAX; missing_at], false))
                            } else {
                                None
                            },
                        };
                        if !missing_alpha {
                            source.colors[2].truncate(missing_at);
                        }
                        let instruction = JpxInstructionInfo {
                            layer_index: 0,
                            target_x: 1,
                            target_y: 1,
                            target_width: 3,
                            target_height: 2,
                            source_x: 0,
                            source_y: 0,
                            source_width: 0,
                            source_height: 0,
                            persistent: true,
                            next_reuse: 0,
                        };
                        let mut colors = vec![vec![31; 20], vec![32; 20], vec![33; 20]];
                        let mut alpha = vec![17; 20];
                        let mut expected_colors = colors.clone();
                        let mut expected_alpha = alpha.clone();
                        let mut expected_work = initial_work;
                        let mut expected_result = Ok(());
                        for rank in 0..6 {
                            if rank == missing_at {
                                expected_result = Err(DecodeError::MalformedInput {
                                    offset: 0,
                                    kind: MalformedKind::InvalidBoxContents,
                                });
                                break;
                            }
                            let index = (rank / 3 + 1) * 5 + rank % 3 + 1;
                            for (channel, samples) in expected_colors.iter_mut().enumerate() {
                                samples[index] = source.colors[channel][rank];
                            }
                            expected_alpha[index] = u16::MAX;
                            let wide_work = u128::from(expected_work) + 4;
                            if wide_work > u128::from(u64::MAX) {
                                expected_result = Err(DecodeError::MalformedInput {
                                    offset: 0,
                                    kind: MalformedKind::ArithmeticOverflow,
                                });
                                break;
                            }
                            expected_work = u64::try_from(wide_work).unwrap();
                            if expected_work > limit {
                                expected_result = Err(DecodeError::LimitExceeded {
                                    resource: Resource::WorkUnits,
                                    limit,
                                    actual: expected_work,
                                });
                                break;
                            }
                        }
                        let mut work = initial_work;
                        assert_eq!(
                            composite_placed_layer(
                                5,
                                4,
                                ConvertedColorSpace::Srgb,
                                &source,
                                instruction,
                                &mut colors,
                                &mut alpha,
                                &mut work,
                                &ResourceLimits::default()
                                    .with_maximum_work_units(limit)
                                    .unwrap(),
                            ),
                            expected_result
                        );
                        assert_eq!(colors, expected_colors);
                        assert_eq!(alpha, expected_alpha);
                        assert_eq!(work, expected_work);
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 1_260);
    }

    #[test]
    fn placed_layer_failures_preserve_multirow_raster_prefixes() {
        let mut cases = Vec::new();
        for count in 1..=6_usize {
            let work = 9 + u64::try_from(count).unwrap() * 4;
            cases.push((
                count,
                9,
                work - 1,
                false,
                Err(DecodeError::limit(Resource::WorkUnits, work - 1, work)),
                work,
            ));
        }
        cases.push((4, 9, 1_000, true, Err(invalid_layer()), 25));
        cases.push((
            1,
            u64::MAX - 3,
            1_000,
            false,
            Err(arithmetic_overflow()),
            u64::MAX - 3,
        ));
        cases.push((6, 9, 33, false, Ok(()), 33));
        for (stored, initial_work, limit, missing_sample, expected_result, expected_work) in cases {
            let mut source = OwnedLayer {
                width: 3,
                height: 2,
                color_space: ConvertedColorSpace::Srgb,
                colors: (0..3)
                    .map(|channel| {
                        (0..6)
                            .map(|i| u16::try_from((channel + 1) * 100 + i).unwrap())
                            .collect::<Vec<_>>()
                    })
                    .collect(),
                alpha: None,
            };
            if missing_sample {
                source.colors[2].truncate(4);
            }
            let instruction = JpxInstructionInfo {
                layer_index: 0,
                target_x: 1,
                target_y: 1,
                target_width: 3,
                target_height: 2,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                persistent: true,
                next_reuse: 0,
            };
            let mut colors = vec![vec![31; 20], vec![32; 20], vec![33; 20]];
            let mut alpha = vec![17; 20];
            let mut expected_colors = colors.clone();
            let mut expected_alpha = alpha.clone();
            for rank in 0..stored {
                let destination = (rank / 3 + 1) * 5 + rank % 3 + 1;
                for (channel, samples) in expected_colors.iter_mut().enumerate() {
                    samples[destination] = source.colors[channel][rank];
                }
                expected_alpha[destination] = u16::MAX;
            }
            let mut work = initial_work;
            assert_eq!(
                composite_placed_layer(
                    5,
                    4,
                    ConvertedColorSpace::Srgb,
                    &source,
                    instruction,
                    &mut colors,
                    &mut alpha,
                    &mut work,
                    &ResourceLimits::default()
                        .with_maximum_work_units(limit)
                        .unwrap(),
                ),
                expected_result
            );
            assert_eq!(colors, expected_colors);
            assert_eq!(alpha, expected_alpha);
            assert_eq!(work, expected_work);
        }
    }

    #[test]
    fn placed_composition_preserves_empty_zero_target_and_off_canvas_behavior() {
        let source = OwnedLayer {
            width: 1,
            height: 1,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![vec![123]],
            alpha: None,
        };
        let base = JpxInstructionInfo {
            layer_index: 0,
            target_x: 0,
            target_y: 0,
            target_width: 1,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            persistent: true,
            next_reuse: 0,
        };
        for (instruction, expected) in [
            (
                JpxInstructionInfo {
                    source_x: 1,
                    target_width: 0,
                    target_height: 0,
                    ..base
                },
                Ok(()),
            ),
            (
                JpxInstructionInfo {
                    target_width: 0,
                    ..base
                },
                Err(invalid_layer()),
            ),
            (
                JpxInstructionInfo {
                    target_height: 0,
                    ..base
                },
                Err(invalid_layer()),
            ),
            (
                JpxInstructionInfo {
                    target_x: u32::MAX,
                    ..base
                },
                Ok(()),
            ),
            (
                JpxInstructionInfo {
                    target_y: u32::MAX,
                    ..base
                },
                Ok(()),
            ),
        ] {
            let mut colors = vec![vec![17]];
            let mut alpha = vec![23];
            let mut work = 29;
            assert_eq!(
                composite_placed_layer(
                    1,
                    1,
                    ConvertedColorSpace::Grayscale,
                    &source,
                    instruction,
                    &mut colors,
                    &mut alpha,
                    &mut work,
                    &ResourceLimits::default()
                ),
                expected
            );
            assert_eq!(colors, [vec![17]]);
            assert_eq!(alpha, [23]);
            assert_eq!(work, 29);
        }
    }

    #[test]
    fn composition_store_has_exact_single_slot_effect() {
        for length in 0..=8 {
            let before: Vec<u16> = (0..length)
                .map(|i| u16::try_from(i).unwrap() * 4093)
                .collect();
            for index in (0..length + 2).chain(std::iter::once(usize::MAX)) {
                for value in [0, 1, 255, 32_768, u16::MAX] {
                    let mut samples = before.clone();
                    let stored =
                        crate::refinement::try_store_composition_sample(&mut samples, index, value);
                    let mut expected = before.clone();
                    if index < length {
                        expected[index] = value;
                    }
                    assert_eq!(stored, index < length);
                    assert_eq!(samples.len(), before.len());
                    assert_eq!(samples, expected);
                }
            }
        }
    }

    #[test]
    fn composition_store_respects_subslice_offsets_and_repeated_writes() {
        let mut samples = vec![111, 222, 3, 4, 5, 666, 777];
        assert!(crate::refinement::try_store_composition_sample(
            &mut samples[2..5],
            0,
            u16::MAX
        ));
        assert!(crate::refinement::try_store_composition_sample(
            &mut samples[2..5],
            2,
            0
        ));
        assert!(!crate::refinement::try_store_composition_sample(
            &mut samples[2..5],
            3,
            999
        ));
        assert!(!crate::refinement::try_store_composition_sample(
            &mut samples[2..5],
            usize::MAX,
            999
        ));
        assert_eq!(samples, [111, 222, u16::MAX, 4, 0, 666, 777]);
        assert!(crate::refinement::try_store_composition_sample(
            &mut samples[2..5],
            0,
            17
        ));
        assert_eq!(samples, [111, 222, 17, 4, 0, 666, 777]);
    }

    #[test]
    fn placed_source_over_samples_match_wide_rational_rounding_and_preserve_guards() {
        let maximum = u128::from(u16::MAX);
        let nearest = |numerator: u128, denominator: u128| {
            if denominator == 0 {
                0
            } else {
                u16::try_from(
                    numerator / denominator
                        + u128::from(2 * (numerator % denominator) >= denominator),
                )
                .unwrap()
            }
        };
        let instruction = JpxInstructionInfo {
            layer_index: 0,
            target_x: 1,
            target_y: 0,
            target_width: 5,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            persistent: true,
            next_reuse: 0,
        };
        for premultiplied in [false, true] {
            let samples = if premultiplied {
                vec![0, 1, 16_384, 12_345, 0]
            } else {
                vec![u16::MAX, 1, 32_767, 12_345, 0]
            };
            let source_alpha = vec![0, 1, 32_768, u16::MAX, 32_767];
            let mut source = OwnedLayer {
                width: 5,
                height: 1,
                color_space: ConvertedColorSpace::Grayscale,
                colors: vec![samples.clone()],
                alpha: Some((source_alpha.clone(), premultiplied)),
            };
            let before_colors = vec![
                vec![7, u16::MAX, 3, 32_768, 34_567, 60_000, 11],
                vec![13, 1, u16::MAX, 123, 45_678, 2, 19],
                vec![17, 32_767, 1, 54_321, 0, u16::MAX, 23],
            ];
            let before_alpha = vec![17, 0, u16::MAX, 32_767, 0, 12_345, 19];
            let mut colors = before_colors.clone();
            let mut alpha = before_alpha.clone();
            let mut work = 0;
            composite_placed_layer(
                7,
                1,
                ConvertedColorSpace::Srgb,
                &source,
                instruction,
                &mut colors,
                &mut alpha,
                &mut work,
                &ResourceLimits::default(),
            )
            .unwrap();
            assert_eq!(work, 20);
            for index in 0..7 {
                if index == 0 || index == 6 {
                    assert_eq!(alpha[index], before_alpha[index]);
                    for channel in 0..3 {
                        assert_eq!(colors[channel][index], before_colors[channel][index]);
                    }
                    continue;
                }
                let a = u128::from(source_alpha[index - 1]);
                let d = u128::from(before_alpha[index]);
                let alpha_numerator = a * maximum + d * (maximum - a);
                assert_eq!(alpha[index], nearest(alpha_numerator, maximum));
                for channel in 0..3 {
                    let s = u128::from(samples[index - 1]);
                    let c = u128::from(before_colors[channel][index]);
                    let source_term = if premultiplied {
                        s * maximum * maximum
                    } else {
                        s * a * maximum
                    };
                    assert_eq!(
                        colors[channel][index],
                        nearest(source_term + c * d * (maximum - a), alpha_numerator)
                    );
                }
            }
            if premultiplied {
                source.colors[0][0] = 1;
                let before_invalid_colors = colors.clone();
                let before_invalid_alpha = alpha.clone();
                assert_eq!(
                    composite_placed_layer(
                        7,
                        1,
                        ConvertedColorSpace::Srgb,
                        &source,
                        instruction,
                        &mut colors,
                        &mut alpha,
                        &mut work,
                        &ResourceLimits::default(),
                    ),
                    Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents)),
                );
                assert_eq!(colors, before_invalid_colors);
                assert_eq!(alpha, before_invalid_alpha);
                assert_eq!(work, 20);
            }
        }
    }

    #[test]
    fn source_over_alpha_bounds_cover_every_source_alpha() {
        let maximum = u64::from(u16::MAX);
        for source_alpha in 0..=u16::MAX {
            for destination_alpha in [0, 1, 2, 32_767, 32_768, 65_534, u16::MAX] {
                let numerator = source_over_alpha_numerator(source_alpha, destination_alpha);
                let expected = u128::from(source_alpha) * u128::from(u16::MAX)
                    + u128::from(destination_alpha) * u128::from(u16::MAX - source_alpha);
                assert_eq!(u128::from(numerator), expected);
                assert!(numerator <= maximum * maximum);
                assert_eq!(numerator == 0, source_alpha == 0 && destination_alpha == 0);
                assert!(source_over_rounded_ratio(numerator, maximum) <= u128::from(u16::MAX));
            }
        }
    }

    #[test]
    fn source_over_color_bounds_and_representations_match_wide_arithmetic() {
        let values = [
            0,
            1,
            2,
            127,
            128,
            255,
            256,
            32_767,
            32_768,
            65_534,
            u16::MAX,
        ];
        let maximum = u128::from(u16::MAX);
        for source in values {
            for destination in values {
                for source_alpha in values {
                    for destination_alpha in values {
                        let alpha = source_over_alpha_numerator(source_alpha, destination_alpha);
                        for premultiplied in [false, true] {
                            let numerator = source_over_color_numerator(
                                source,
                                destination,
                                source_alpha,
                                destination_alpha,
                                premultiplied,
                            );
                            let source_term = if premultiplied {
                                u128::from(source) * maximum * maximum
                            } else {
                                u128::from(source) * u128::from(source_alpha) * maximum
                            };
                            let expected = source_term
                                + u128::from(destination)
                                    * u128::from(destination_alpha)
                                    * u128::from(u16::MAX - source_alpha);
                            assert_eq!(u128::from(numerator), expected);
                            if !premultiplied || source <= source_alpha {
                                assert!(expected <= maximum * u128::from(alpha));
                                let output = source_over_rounded_ratio(numerator, alpha);
                                assert!(output <= maximum);
                                if source_alpha == u16::MAX {
                                    assert_eq!(output, u128::from(source));
                                }
                                if source_alpha == 0 {
                                    assert_eq!(
                                        output,
                                        if destination_alpha == 0 {
                                            0
                                        } else {
                                            u128::from(destination)
                                        }
                                    );
                                }
                            }
                        }
                        if source == 0 || source == u16::MAX {
                            let premultiplied_source = if source == 0 { 0 } else { source_alpha };
                            assert_eq!(
                                source_over_color_numerator(
                                    source,
                                    destination,
                                    source_alpha,
                                    destination_alpha,
                                    false,
                                ),
                                source_over_color_numerator(
                                    premultiplied_source,
                                    destination,
                                    source_alpha,
                                    destination_alpha,
                                    true,
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn source_over_rounding_handles_ties_zero_and_full_width_inputs() {
        let values = [0, 1, 2, 3, 4, u64::MAX / 2, u64::MAX - 1, u64::MAX];
        for numerator in values {
            for denominator in values {
                let actual = source_over_rounded_ratio(numerator, denominator);
                let expected = if denominator == 0 {
                    0
                } else {
                    let n = u128::from(numerator);
                    let d = u128::from(denominator);
                    n / d + u128::from(2 * (n % d) >= d)
                };
                assert_eq!(actual, expected);
                if denominator > 0 {
                    let n = u128::from(numerator);
                    let d = u128::from(denominator);
                    assert!(2 * d * actual <= 2 * n + d);
                    assert!(2 * n < 2 * d * actual + d);
                }
            }
        }
        assert_eq!(source_over_rounded_ratio(1, 2), 1);
        assert_eq!(source_over_rounded_ratio(3, 2), 2);
        assert_eq!(source_over_rounded_ratio(1, 3), 0);
        assert_eq!(source_over_rounded_ratio(2, 3), 1);
    }

    #[test]
    fn single_layer_output_preserves_fractional_premultiplied_representation() {
        let layer = OwnedLayer {
            width: 2,
            height: 1,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![vec![1, 12_345]],
            alpha: Some((vec![2, 32_768], true)),
        };
        let image = layer.into_image().unwrap();
        assert_eq!(image.channel(0).unwrap().samples(), &[1, 12_345]);
        assert_eq!(image.channel(1).unwrap().samples(), &[2, 32_768]);
        assert_eq!(
            image.channel(1).unwrap().channel_type(),
            ConvertedChannelType::Alpha {
                premultiplied: true,
                association: crate::image::ChannelAssociation::WholeImage,
            }
        );
    }

    #[test]
    fn source_over_is_deterministic_and_crops_to_canvas() {
        let black = crate::color::from_composited(
            2,
            1,
            ConvertedColorSpace::Grayscale,
            vec![vec![0, 0]],
            vec![u16::MAX, u16::MAX],
        )
        .unwrap();
        let white = crate::color::from_composited(
            2,
            1,
            ConvertedColorSpace::Grayscale,
            vec![vec![u16::MAX, 1]],
            vec![32_768, u16::MAX],
        )
        .unwrap();
        let black_channels = layer_channels(&black).unwrap();
        let white_channels = layer_channels(&white).unwrap();
        let mut colors = vec![vec![0_u16; 2]];
        let mut alpha = vec![0_u16; 2];
        let limits = ResourceLimits::default();
        let mut work = 0_u64;
        composite_layer(
            2,
            1,
            0,
            0,
            &black_channels,
            &mut colors,
            &mut alpha,
            &mut work,
            &limits,
        )
        .unwrap();
        composite_layer(
            2,
            1,
            1,
            0,
            &white_channels,
            &mut colors,
            &mut alpha,
            &mut work,
            &limits,
        )
        .unwrap();
        assert_eq!(colors[0], [0, 32_768]);
        assert_eq!(alpha, [u16::MAX, u16::MAX]);

        let tight = ResourceLimits::default()
            .with_maximum_work_units(1)
            .unwrap();
        let mut bounded_colors = vec![vec![0_u16; 2]];
        let mut bounded_alpha = vec![0_u16; 2];
        assert!(matches!(
            composite_layer(
                2,
                1,
                0,
                0,
                &black_channels,
                &mut bounded_colors,
                &mut bounded_alpha,
                &mut 0,
                &tight,
            ),
            Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                ..
            })
        ));
    }

    #[test]
    fn rejects_premultiplied_sample_above_alpha() {
        let source = LayerChannels {
            width: 1,
            height: 1,
            colors: vec![&[2_u16]],
            alpha: Some((vec![1_u16], true)),
        };
        let mut colors = vec![vec![0_u16]];
        let mut alpha = vec![0_u16];
        assert!(matches!(
            composite_layer(
                1,
                1,
                0,
                0,
                &source,
                &mut colors,
                &mut alpha,
                &mut 0,
                &ResourceLimits::default(),
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));
    }

    #[test]
    fn timeline_matches_independent_flat_schedule_and_grouping_model() {
        fn reference(
            composition: &Composition,
            layers: usize,
            maximum_frames: u32,
        ) -> Result<Vec<JpxFrameInfo>, DecodeError> {
            let limit = (maximum_frames as usize).checked_mul(layers.max(1)).ok_or(
                DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                },
            )?;
            let program: Vec<_> = composition
                .instruction_sets
                .iter()
                .flat_map(|set| {
                    (0..=set.repeat).flat_map(move |_| set.instructions.iter().copied())
                })
                .collect();
            let mut scheduled = std::collections::BTreeMap::new();
            let mut unused = 0..layers;
            let mut frames: Vec<JpxFrameInfo> = Vec::new();
            let mut time = 0;
            for (index, instruction) in program.into_iter().enumerate() {
                if index >= limit {
                    return Err(DecodeError::LimitExceeded {
                        resource: Resource::WorkUnits,
                        limit: limit as u64,
                        actual: index as u64 + 1,
                    });
                }
                let layer = match scheduled.get(&index).copied().or_else(|| unused.next()) {
                    Some(layer) => layer,
                    None => break,
                };
                let open = frames
                    .last()
                    .is_some_and(|frame| frame.duration_milliseconds == 0);
                if !open {
                    if frames.len() == maximum_frames as usize {
                        return Err(DecodeError::LimitExceeded {
                            resource: Resource::Frames,
                            limit: maximum_frames as u64,
                            actual: frames.len() as u64 + 1,
                        });
                    }
                    frames.push(JpxFrameInfo {
                        index: frames.len(),
                        start_milliseconds: time,
                        duration_milliseconds: 0,
                        persistent: true,
                        instructions: Vec::new(),
                    });
                }
                let frame = frames.last_mut().unwrap();
                frame.instructions.push(JpxInstructionInfo {
                    layer_index: layer,
                    target_x: instruction.target_x,
                    target_y: instruction.target_y,
                    target_width: instruction.target_width,
                    target_height: instruction.target_height,
                    source_x: instruction.source_x,
                    source_y: instruction.source_y,
                    source_width: instruction.source_width,
                    source_height: instruction.source_height,
                    persistent: instruction.persistent,
                    next_reuse: instruction.next_reuse,
                });
                frame.duration_milliseconds = instruction.life;
                frame.persistent = instruction.persistent;
                time += u64::from(instruction.life);
                if instruction.next_reuse != 0 {
                    let target = index + instruction.next_reuse as usize;
                    if target >= limit {
                        return Err(DecodeError::LimitExceeded {
                            resource: Resource::WorkUnits,
                            limit: limit as u64,
                            actual: target as u64 + 1,
                        });
                    }
                    if scheduled.get(&target).is_some_and(|&prior| prior != layer) {
                        return Err(DecodeError::MalformedInput {
                            offset: 0,
                            kind: MalformedKind::InvalidBoxContents,
                        });
                    }
                    scheduled.insert(target, layer);
                }
            }
            Ok(frames)
        }
        let make = |life, persistent, next_reuse, offset| CompositionInstruction {
            target_x: offset,
            target_y: offset + 1,
            target_width: offset + 2,
            target_height: offset + 3,
            source_x: offset + 4,
            source_y: offset + 5,
            source_width: offset + 6,
            source_height: offset + 7,
            life,
            persistent,
            next_reuse,
        };
        for layers in 0..4 {
            for maximum_frames in 0..5 {
                for repeat in 0..3 {
                    for first_life in [0, 1, u32::MAX] {
                        for second_life in [0, 7] {
                            for first_reuse in 0..4 {
                                for second_reuse in 0..4 {
                                    for persistent in [false, true] {
                                        let composition = Composition {
                                            width: 1,
                                            height: 1,
                                            loop_count: 2,
                                            has_unknown_boxes: true,
                                            instruction_sets: vec![
                                                CompositionInstructionSet {
                                                    flags: 7,
                                                    repeat,
                                                    tick: 11,
                                                    instructions: vec![
                                                        make(
                                                            first_life,
                                                            persistent,
                                                            first_reuse,
                                                            10,
                                                        ),
                                                        make(
                                                            second_life,
                                                            !persistent,
                                                            second_reuse,
                                                            20,
                                                        ),
                                                    ],
                                                },
                                                CompositionInstructionSet {
                                                    flags: 0,
                                                    repeat: 1,
                                                    tick: 99,
                                                    instructions: vec![],
                                                },
                                                CompositionInstructionSet {
                                                    flags: 7,
                                                    repeat: 0,
                                                    tick: 1,
                                                    instructions: vec![make(9, false, 0, 30)],
                                                },
                                            ],
                                        };
                                        assert_eq!(
                                            build_timeline(&composition, layers, maximum_frames),
                                            reference(&composition, layers, maximum_frames),
                                            "layers={layers}, frames={maximum_frames}, repeat={repeat}, life={first_life}/{second_life}, reuse={first_reuse}/{second_reuse}, persistent={persistent}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let empty = Composition {
            width: 0,
            height: 0,
            loop_count: 0,
            instruction_sets: vec![],
            has_unknown_boxes: false,
        };
        assert_eq!(build_timeline(&empty, 1, 0).unwrap(), []);
        assert_eq!(
            build_timeline(&empty, usize::MAX, 2).err(),
            Some(arithmetic_overflow())
        );
    }

    #[test]
    fn timeline_checked_errors_precede_later_reuse_and_layer_exhaustion() {
        let make = |life, next_reuse| CompositionInstruction {
            target_x: 0,
            target_y: 0,
            target_width: 1,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            life,
            persistent: false,
            next_reuse,
        };
        let composition = |instructions| Composition {
            width: 1,
            height: 1,
            loop_count: 1,
            instruction_sets: vec![CompositionInstructionSet {
                flags: 7,
                repeat: 0,
                tick: 1,
                instructions,
            }],
            has_unknown_boxes: false,
        };
        assert_eq!(
            build_timeline(&composition(vec![make(0, 0)]), 0, 0).err(),
            Some(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: 0,
                actual: 1
            })
        );
        assert_eq!(
            build_timeline(&composition(vec![make(0, 0)]), 0, 1).unwrap(),
            []
        );
        assert_eq!(
            build_timeline(&composition(vec![make(1, 0), make(1, u32::MAX)]), 2, 1).err(),
            Some(DecodeError::LimitExceeded {
                resource: Resource::Frames,
                limit: 1,
                actual: 2
            })
        );
        assert_eq!(
            build_timeline(&composition(vec![make(1, 2), make(1, 1)]), 2, 3).err(),
            Some(invalid_layer())
        );
        let mut frames = Vec::new();
        let mut time = u64::MAX;
        assert_eq!(
            append_timeline_frame(&mut frames, &mut time, test_unit_instruction(0, 1), 1, 1).err(),
            Some(arithmetic_overflow())
        );
        let mut state = TimelineState {
            frames: Vec::new(),
            assignments: Vec::new(),
            next_layer: 0,
            instruction_index: 0,
            milliseconds: 0,
        };
        assert_eq!(
            apply_timeline_instruction(&mut state, make(1, 0), 0, 0, 0).err(),
            Some(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: 0,
                actual: 1
            })
        );
    }

    #[test]
    fn timeline_expands_repetition_reuse_and_timing_deterministically() {
        let instruction = CompositionInstruction {
            target_x: 2,
            target_y: 3,
            target_width: 4,
            target_height: 5,
            life: 25,
            persistent: true,
            next_reuse: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
        };
        let composition = Composition {
            width: 8,
            height: 8,
            loop_count: 1,
            instruction_sets: vec![CompositionInstructionSet {
                flags: 7,
                repeat: 1,
                tick: 1,
                instructions: vec![instruction],
            }],
            has_unknown_boxes: false,
        };
        let frames = build_timeline(&composition, 1, 4).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].duration_milliseconds(), 25);
        assert_eq!(frames[0].start_milliseconds(), 0);
        assert_eq!(frames[1].start_milliseconds(), 25);
        assert_eq!(frames[0].instructions()[0].layer_index(), 0);
        assert!(frames[0].instructions()[0].persistent());
        assert_eq!(frames[0].instructions()[0].next_reuse(), 1);
        assert_eq!(frames[1].instructions()[0].layer_index(), 0);
        assert_eq!(frames[1].instructions()[0].target_x(), 2);
    }

    #[test]
    fn composed_normalized_frames_match_independent_indexed_supply_and_rational_model() {
        let limits = ResourceLimits::default();
        for mask in 0_u8..16 {
            let composition = Composition {
                width: 2,
                height: 1,
                loop_count: 0,
                has_unknown_boxes: false,
                instruction_sets: vec![CompositionInstructionSet {
                    flags: 7,
                    repeat: 0,
                    tick: 1,
                    instructions: (0..4)
                        .map(|index| CompositionInstruction {
                            target_x: index % 2,
                            target_y: 0,
                            target_width: 1,
                            target_height: 1,
                            source_x: 0,
                            source_y: 0,
                            source_width: 0,
                            source_height: 0,
                            life: index + 1,
                            persistent: mask & (1 << index) != 0,
                            next_reuse: if index == 0 {
                                2
                            } else if index == 2 {
                                1
                            } else {
                                0
                            },
                        })
                        .collect(),
                }],
            };
            let frames = build_timeline(&composition, 2, 4).unwrap();
            for request in [0, 1, 2, 3, 4, 7, 2, 6, 0] {
                for rgb_first in [false, true] {
                    let make_gray = |sample, alpha, premultiplied| {
                        (
                            0,
                            Some(Ok(OwnedLayer {
                                width: 1,
                                height: 1,
                                color_space: ConvertedColorSpace::Grayscale,
                                colors: vec![vec![sample]],
                                alpha: Some((vec![alpha], premultiplied)),
                            })),
                        )
                    };
                    let rgb = (
                        1,
                        Some(Ok(OwnedLayer {
                            width: 1,
                            height: 1,
                            color_space: ConvertedColorSpace::Srgb,
                            colors: vec![vec![60_000], vec![3], vec![32_768]],
                            alpha: None,
                        })),
                    );
                    let mut bank = vec![
                        make_gray(12_345, 32_768, false),
                        (
                            usize::MAX,
                            Some(Err(DecodeError::FrameIndexOutOfRange {
                                index: 99,
                                frames: 1,
                            })),
                        ),
                        (0, None),
                        make_gray(8_000, 16_384, true),
                        make_gray(60_000, u16::MAX, false),
                    ];
                    bank.insert(if rgb_first { 0 } else { 3 }, rgb);
                    let image = render_normalized_frame(
                        2,
                        1,
                        &frames,
                        0,
                        request,
                        bank,
                        limits.maximum_decoded_bytes(),
                        &limits,
                    )
                    .unwrap();
                    let current = request % 4;
                    let chosen: Vec<usize> = (0..=current)
                        .filter(|&index| index == current || mask & (1 << index) != 0)
                        .collect();
                    let rgb = chosen.contains(&1);
                    let channels = if rgb { 3 } else { 1 };
                    let mut colors = vec![vec![0_u16; 2]; channels];
                    let mut alpha = vec![0_u16; 2];
                    let mut gray_occurrence = 0;
                    let maximum = u128::from(u16::MAX);
                    let nearest = |n: u128, d: u128| -> u16 {
                        if d == 0 {
                            0
                        } else {
                            (n / d + u128::from(n % d * 2 >= d)) as u16
                        }
                    };
                    for index in chosen {
                        let pixel = index % 2;
                        let (samples, a, premultiplied) = if index == 1 {
                            ([60_000_u128, 3, 32_768], maximum, false)
                        } else {
                            let (sample, a, flag) = [
                                (12_345, 32_768, false),
                                (8_000, 16_384, true),
                                (60_000, 65_535, false),
                            ][gray_occurrence];
                            gray_occurrence += 1;
                            ([sample; 3], a, flag)
                        };
                        let d = u128::from(alpha[pixel]);
                        let denominator = a * maximum + d * (maximum - a);
                        for channel in 0..channels {
                            let source = if premultiplied {
                                samples[channel] * maximum * maximum
                            } else {
                                samples[channel] * a * maximum
                            };
                            colors[channel][pixel] = nearest(
                                source + u128::from(colors[channel][pixel]) * d * (maximum - a),
                                denominator,
                            );
                        }
                        alpha[pixel] = nearest(denominator, maximum);
                    }
                    assert_eq!(
                        (
                            image.width(),
                            image.height(),
                            image.precision(),
                            image.channels().len()
                        ),
                        (2, 1, 16, channels + 1)
                    );
                    assert_eq!(
                        image.color_space(),
                        if rgb {
                            ConvertedColorSpace::Srgb
                        } else {
                            ConvertedColorSpace::Grayscale
                        }
                    );
                    for (channel, samples) in colors.iter().enumerate() {
                        assert_eq!(image.channel(channel as u16).unwrap().samples(), samples);
                        assert_eq!(
                            image.channel(channel as u16).unwrap().channel_type(),
                            if rgb {
                                [
                                    ConvertedChannelType::Red,
                                    ConvertedChannelType::Green,
                                    ConvertedChannelType::Blue,
                                ][channel]
                            } else {
                                ConvertedChannelType::Gray
                            }
                        );
                    }
                    assert_eq!(image.channel(channels as u16).unwrap().samples(), alpha);
                    assert_eq!(
                        image.channel(channels as u16).unwrap().channel_type(),
                        ConvertedChannelType::Alpha {
                            premultiplied: false,
                            association: ChannelAssociation::WholeImage,
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn composed_normalized_frames_preserve_selection_supply_and_render_error_precedence() {
        let limits = ResourceLimits::default()
            .with_maximum_work_units(0)
            .unwrap();
        let frame = |index, reference, persistent| JpxFrameInfo {
            index,
            start_milliseconds: index as u64,
            duration_milliseconds: 1,
            persistent,
            instructions: vec![test_unit_instruction(reference, 1)],
        };
        let frames = [frame(0, 7, true), frame(1, 9, false)];
        let first_error = DecodeError::FrameIndexOutOfRange {
            index: 123,
            frames: 2,
        };
        let second_error = DecodeError::LayerIndexOutOfRange {
            index: 456,
            layers: 3,
        };
        let invalid = || {
            (
                7,
                Some(Ok(OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Grayscale,
                    colors: vec![vec![]],
                    alpha: None,
                })),
            )
        };
        for reversed in [false, true] {
            let mut bank = vec![(9, Some(Err(second_error))), (7, Some(Err(first_error)))];
            if reversed {
                bank.reverse();
            }
            assert_eq!(
                render_normalized_frame(1, 1, &frames, 1, 1, bank, 0, &limits).err(),
                Some(first_error)
            );
        }
        assert_eq!(
            render_normalized_frame(
                1,
                1,
                &frames,
                1,
                1,
                vec![(9, Some(Err(second_error))), invalid()],
                0,
                &limits
            )
            .err(),
            Some(second_error)
        );
        assert_eq!(
            render_normalized_frame(1, 1, &frames, 1, 1, vec![invalid()], 0, &limits).err(),
            Some(invalid_layer())
        );
        assert_eq!(
            render_normalized_frame(
                1,
                1,
                &frames,
                1,
                2,
                vec![(7, Some(Err(first_error)))],
                0,
                &limits
            )
            .err(),
            Some(DecodeError::FrameIndexOutOfRange {
                index: 2,
                frames: 2
            })
        );
        assert_eq!(
            render_normalized_frame(
                1,
                1,
                &[],
                0,
                0,
                vec![(7, Some(Err(first_error)))],
                0,
                &limits
            )
            .err(),
            Some(DecodeError::FrameIndexOutOfRange {
                index: 0,
                frames: 0
            })
        );
        let repeated = [JpxFrameInfo {
            index: 0,
            start_milliseconds: 0,
            duration_milliseconds: 1,
            persistent: false,
            instructions: vec![test_unit_instruction(7, 1), test_unit_instruction(7, 1)],
        }];
        assert_eq!(
            render_normalized_frame(
                1,
                1,
                &repeated,
                0,
                0,
                vec![
                    (7, None),
                    invalid(),
                    (7, Some(Err(second_error))),
                    (7, Some(Err(first_error)))
                ],
                0,
                &limits
            )
            .err(),
            Some(second_error)
        );
        let valid = || {
            (
                7,
                Some(Ok(OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Grayscale,
                    colors: vec![vec![7]],
                    alpha: None,
                })),
            )
        };
        assert_eq!(
            render_normalized_frame(1, 1, &frames[..1], 0, 0, vec![valid()], 7, &limits).err(),
            Some(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 7,
                actual: 8
            })
        );
        assert_eq!(
            render_normalized_frame(1, 1, &frames[..1], 0, 0, vec![valid()], 8, &limits).err(),
            Some(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: 0,
                actual: 3
            })
        );
        assert_eq!(
            render_normalized_frame(
                1,
                1,
                &frames[..1],
                0,
                0,
                vec![invalid()],
                8,
                &ResourceLimits::default()
            )
            .err(),
            Some(invalid_layer())
        );
    }

    #[test]
    fn normalized_frame_selection_matches_ordered_sequence_model() {
        for mask in 0_u8..16 {
            let frames: Vec<_> = (0..4)
                .map(|index| JpxFrameInfo {
                    index: 99 - index,
                    start_milliseconds: index as u64 * 11,
                    duration_milliseconds: 11,
                    persistent: mask & (1 << index) != 0,
                    instructions: (0..index)
                        .map(|local| {
                            let mut instruction = test_unit_instruction((index + local) % 3, 1);
                            instruction.target_x = (index * 10 + local) as u32;
                            instruction.persistent = local % 2 == 0;
                            instruction.next_reuse = local as u32;
                            instruction
                        })
                        .collect(),
                })
                .collect();
            for loops in [0, 1, 3] {
                for requested in (0..14).chain([usize::MAX]) {
                    let selected = select_frame_instructions(&frames, loops, requested);
                    if loops != 0 && requested >= 4 * usize::from(loops) {
                        assert_eq!(
                            selected.err(),
                            Some(DecodeError::FrameIndexOutOfRange {
                                index: requested,
                                frames: 4 * usize::from(loops),
                            })
                        );
                        continue;
                    }
                    let current = requested % 4;
                    let selected = selected.unwrap();
                    let expected: Vec<_> = (0..=current)
                        .filter(|&index| index == current || mask & (1 << index) != 0)
                        .flat_map(|index| {
                            (0..index).map(move |local| {
                                (
                                    (index * 10 + local) as u32,
                                    (index + local) % 3,
                                    local % 2 == 0,
                                    local as u32,
                                )
                            })
                        })
                        .collect();
                    assert_eq!(
                        selected
                            .iter()
                            .map(|instruction| (
                                instruction.target_x,
                                instruction.layer_index,
                                instruction.persistent,
                                instruction.next_reuse
                            ))
                            .collect::<Vec<_>>(),
                        expected
                    );
                }
            }
        }
        for requested in [0, 1, usize::MAX] {
            assert_eq!(
                select_frame_instructions(&[], 0, requested).err(),
                Some(DecodeError::FrameIndexOutOfRange {
                    index: requested,
                    frames: 0
                })
            );
        }
    }

    #[test]
    fn normalized_multiframe_reuse_persistence_and_reset_match_rational_images() {
        let mut instructions = Vec::new();
        for (index, (life, persistent, next_reuse)) in
            [(10, true, 2), (20, false, 0), (30, true, 1), (40, false, 0)]
                .into_iter()
                .enumerate()
        {
            instructions.push(CompositionInstruction {
                target_x: u32::from(index == 3),
                target_y: 0,
                target_width: 1,
                target_height: 1,
                life,
                persistent,
                next_reuse,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
            });
        }
        let composition = Composition {
            width: 2,
            height: 1,
            loop_count: 0,
            has_unknown_boxes: false,
            instruction_sets: vec![CompositionInstructionSet {
                flags: 7,
                repeat: 0,
                tick: 1,
                instructions,
            }],
        };
        let frames = build_timeline(&composition, 2, 4).unwrap();
        assert_eq!(
            frames
                .iter()
                .map(|frame| (
                    frame.index,
                    frame.start_milliseconds,
                    frame.duration_milliseconds,
                    frame.persistent,
                    frame.instructions[0].layer_index
                ))
                .collect::<Vec<_>>(),
            [
                (0, 0, 10, true, 0),
                (1, 10, 20, false, 1),
                (2, 30, 30, true, 0),
                (3, 60, 40, false, 0)
            ]
        );
        let limits = ResourceLimits::default();
        for requested in [0, 1, 2, 3, 4, 7, 6, 2, 0, usize::MAX] {
            let selected =
                select_frame_instructions(&frames, composition.loop_count, requested).unwrap();
            let expected_indices: &[usize] = match requested % 4 {
                0 => &[0],
                1 => &[0, 1],
                2 => &[0, 2],
                _ => &[0, 2, 3],
            };
            assert_eq!(
                selected
                    .iter()
                    .map(|instruction| (instruction.layer_index, instruction.target_x))
                    .collect::<Vec<_>>(),
                expected_indices
                    .iter()
                    .map(|&index| (usize::from(index == 1), u32::from(index == 3)))
                    .collect::<Vec<_>>()
            );
            let layers: Vec<_> = selected
                .into_iter()
                .map(|instruction| {
                    let layer = if instruction.layer_index == 0 {
                        OwnedLayer {
                            width: 1,
                            height: 1,
                            color_space: ConvertedColorSpace::Grayscale,
                            colors: vec![vec![12_345]],
                            alpha: Some((vec![32_768], false)),
                        }
                    } else {
                        OwnedLayer {
                            width: 1,
                            height: 1,
                            color_space: ConvertedColorSpace::Srgb,
                            colors: vec![vec![u16::MAX], vec![0], vec![1]],
                            alpha: None,
                        }
                    };
                    (layer, instruction)
                })
                .collect();
            let image =
                render_decoded_layers(2, 1, &layers, limits.maximum_decoded_bytes(), &limits)
                    .unwrap();
            let color_count = if requested % 4 == 1 { 3 } else { 1 };
            assert_eq!(
                (
                    image.width(),
                    image.height(),
                    image.precision(),
                    image.channels().len()
                ),
                (2, 1, 16, color_count + 1)
            );
            assert_eq!(
                image.color_space(),
                if color_count == 3 {
                    ConvertedColorSpace::Srgb
                } else {
                    ConvertedColorSpace::Grayscale
                }
            );
            let mut expected_colors = vec![vec![0_u16; 2]; color_count];
            let mut expected_alpha = vec![0_u16; 2];
            let maximum = u128::from(u16::MAX);
            let nearest = |n: u128, d: u128| -> u16 {
                if d == 0 {
                    0
                } else {
                    (n / d + u128::from(n % d * 2 >= d)) as u16
                }
            };
            for &index in expected_indices {
                let pixel = usize::from(index == 3);
                let source_alpha = if index == 1 { maximum } else { 32_768 };
                let destination_alpha = u128::from(expected_alpha[pixel]);
                let denominator =
                    source_alpha * maximum + destination_alpha * (maximum - source_alpha);
                for (channel, samples) in expected_colors.iter_mut().enumerate() {
                    let source = if index == 1 {
                        [maximum, 0, 1][channel]
                    } else {
                        12_345
                    };
                    samples[pixel] = nearest(
                        source * source_alpha * maximum
                            + u128::from(samples[pixel])
                                * destination_alpha
                                * (maximum - source_alpha),
                        denominator,
                    );
                }
                expected_alpha[pixel] = nearest(denominator, maximum);
            }
            for (channel, samples) in expected_colors.iter().enumerate() {
                assert_eq!(image.channel(channel as u16).unwrap().samples(), samples);
            }
            assert_eq!(
                image.channel(color_count as u16).unwrap().samples(),
                expected_alpha
            );
            assert_eq!(
                image.channel(color_count as u16).unwrap().channel_type(),
                ConvertedChannelType::Alpha {
                    premultiplied: false,
                    association: ChannelAssociation::WholeImage,
                }
            );
        }
        assert_eq!(
            select_frame_instructions(&frames, 2, 8).err(),
            Some(DecodeError::FrameIndexOutOfRange {
                index: 8,
                frames: 8
            })
        );
    }

    #[test]
    fn normalized_layer_sequence_matches_independent_rational_stack() {
        let maximum = u128::from(u16::MAX);
        let nearest = |n: u128, d: u128| -> u16 {
            if d == 0 {
                0
            } else {
                (n / d + u128::from(n % d * 2 >= d)) as u16
            }
        };
        for order in [[0, 1, 2], [2, 1, 0], [1, 0, 2], [0, 2, 1]] {
            for premultiplied in [false, true] {
                let mut layers = Vec::new();
                let mut expected = [vec![0_u16; 2], vec![0_u16; 2], vec![0_u16; 2]];
                let mut expected_alpha = vec![0_u16; 2];
                for index in order {
                    let alpha = [[32_768, 0], [12_345, u16::MAX], [54_321, 1]][index];
                    let straight = [[60_000, 7], [1, 30_000], [45_000, 60_000]][index];
                    let gray = index != 1;
                    let plane_count = if gray { 1 } else { 3 };
                    let colors: Vec<Vec<u16>> = (0..plane_count)
                        .map(|channel| {
                            (0..2)
                                .map(|pixel| {
                                    let sample = straight[pixel] / (channel + 1) as u16;
                                    if premultiplied {
                                        nearest(
                                            u128::from(sample) * u128::from(alpha[pixel]),
                                            maximum,
                                        )
                                    } else {
                                        sample
                                    }
                                })
                                .collect()
                        })
                        .collect();
                    for pixel in 0..2 {
                        let a = u128::from(alpha[pixel]);
                        let d = u128::from(expected_alpha[pixel]);
                        let denominator = a * maximum + d * (maximum - a);
                        for channel in 0..3 {
                            let s = u128::from(colors[if gray { 0 } else { channel }][pixel]);
                            let old = u128::from(expected[channel][pixel]);
                            expected[channel][pixel] = nearest(
                                if premultiplied {
                                    s * maximum * maximum
                                } else {
                                    s * a * maximum
                                } + old * d * (maximum - a),
                                denominator,
                            );
                        }
                        expected_alpha[pixel] = nearest(denominator, maximum);
                    }
                    layers.push((
                        OwnedLayer {
                            width: 2,
                            height: 1,
                            color_space: if gray {
                                ConvertedColorSpace::Grayscale
                            } else {
                                ConvertedColorSpace::Srgb
                            },
                            colors,
                            alpha: Some((alpha.to_vec(), premultiplied)),
                        },
                        test_unit_instruction(index, 2),
                    ));
                }
                let limits = ResourceLimits::default();
                let image =
                    render_decoded_layers(2, 1, &layers, limits.maximum_decoded_bytes(), &limits)
                        .unwrap();
                assert_eq!(
                    (
                        image.width(),
                        image.height(),
                        image.precision(),
                        image.color_space()
                    ),
                    (2, 1, 16, ConvertedColorSpace::Srgb)
                );
                assert_eq!(image.channels().len(), 4);
                for (channel, expected_samples) in expected.iter().enumerate() {
                    assert_eq!(
                        image.channel(channel as u16).unwrap().samples(),
                        expected_samples
                    );
                    assert_eq!(
                        image.channel(channel as u16).unwrap().channel_type(),
                        [
                            ConvertedChannelType::Red,
                            ConvertedChannelType::Green,
                            ConvertedChannelType::Blue
                        ][channel]
                    );
                }
                assert_eq!(image.channel(3).unwrap().samples(), expected_alpha);
                assert_eq!(
                    image.channel(3).unwrap().channel_type(),
                    ConvertedChannelType::Alpha {
                        premultiplied: false,
                        association: ChannelAssociation::WholeImage,
                    }
                );
            }
        }
    }

    fn test_unit_instruction(layer_index: usize, width: u32) -> JpxInstructionInfo {
        JpxInstructionInfo {
            layer_index,
            target_x: 0,
            target_y: 0,
            target_width: width,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            persistent: true,
            next_reuse: 0,
        }
    }

    #[test]
    fn normalized_layer_sequence_preserves_resource_and_first_error_precedence() {
        let gray = |samples: Vec<u16>| OwnedLayer {
            width: 1,
            height: 1,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![samples],
            alpha: None,
        };
        let limits = ResourceLimits::default();
        let layers = [
            (gray(vec![7]), test_unit_instruction(0, 1)),
            (gray(vec![9]), test_unit_instruction(1, 1)),
        ];
        for (cap, actual) in [(0, 8), (7, 8), (8, 12), (11, 12)] {
            assert_eq!(
                render_decoded_layers(1, 1, &layers, cap, &limits).err(),
                Some(DecodeError::LimitExceeded {
                    resource: Resource::DecodedBytes,
                    limit: cap,
                    actual
                })
            );
        }
        for (cap, actual) in [(0, 3), (2, 3), (3, 5), (4, 5)] {
            let tight = limits.with_maximum_work_units(cap).unwrap();
            assert_eq!(
                render_decoded_layers(1, 1, &layers, 12, &tight).err(),
                Some(DecodeError::LimitExceeded {
                    resource: Resource::WorkUnits,
                    limit: cap,
                    actual
                })
            );
        }
        let image = render_decoded_layers(
            1,
            1,
            &layers,
            12,
            &limits.with_maximum_work_units(5).unwrap(),
        )
        .unwrap();
        assert_eq!(image.channel(0).unwrap().samples(), [9]);
        let invalid = [
            (gray(vec![]), test_unit_instruction(0, 1)),
            (gray(vec![9]), test_unit_instruction(1, 1)),
        ];
        assert_eq!(
            render_decoded_layers(1, 1, &invalid, 7, &limits).err(),
            Some(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 7,
                actual: 8
            })
        );
        assert_eq!(
            render_decoded_layers(1, 1, &invalid, 8, &limits).err(),
            Some(invalid_layer())
        );
        let mut invalid_target = test_unit_instruction(0, 0);
        invalid_target.source_width = 0;
        let invalid_first = [
            (gray(vec![7]), invalid_target),
            (gray(vec![]), test_unit_instruction(1, 1)),
        ];
        assert_eq!(
            render_decoded_layers(1, 1, &invalid_first, 12, &limits).err(),
            Some(invalid_layer())
        );
        assert_eq!(
            render_decoded_layers(0, 0, &[], 0, &limits).err(),
            Some(invalid_layer())
        );
        let huge = [(
            OwnedLayer {
                width: u32::MAX,
                height: u32::MAX,
                color_space: ConvertedColorSpace::Srgb,
                colors: vec![vec![], vec![], vec![]],
                alpha: None,
            },
            test_unit_instruction(0, 1),
        )];
        assert_eq!(
            render_decoded_layers(1, 1, &huge, u64::MAX, &limits).err(),
            Some(arithmetic_overflow())
        );
    }

    #[test]
    fn normalized_layer_sequence_selects_space_globally_and_preserves_empty_canvas_metadata() {
        let mut hidden = test_unit_instruction(1, 1);
        hidden.target_x = 10;
        let layers = [
            (
                OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Grayscale,
                    colors: vec![vec![12_345]],
                    alpha: None,
                },
                test_unit_instruction(0, 1),
            ),
            (
                OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Srgb,
                    colors: vec![vec![], vec![], vec![]],
                    alpha: None,
                },
                hidden,
            ),
        ];
        let limits = ResourceLimits::default();
        let image = render_decoded_layers(1, 1, &layers, 20, &limits).unwrap();
        assert_eq!(image.color_space(), ConvertedColorSpace::Srgb);
        for channel in 0..3 {
            assert_eq!(image.channel(channel).unwrap().samples(), [12_345]);
        }
        assert_eq!(image.channel(3).unwrap().samples(), [u16::MAX]);
        let image = render_decoded_layers(0, 0, &layers, 12, &limits).unwrap();
        assert_eq!(
            (
                image.width(),
                image.height(),
                image.precision(),
                image.channels().len()
            ),
            (0, 0, 16, 4)
        );
        assert!(
            image
                .channels()
                .iter()
                .all(|channel| channel.samples().is_empty())
        );
    }

    #[test]
    fn scaled_registration_and_mixed_color_spaces_are_bounded() {
        let gray = OwnedLayer {
            width: 1,
            height: 1,
            color_space: ConvertedColorSpace::Grayscale,
            colors: vec![vec![12_345]],
            alpha: None,
        };
        let registered = register_layer(
            &gray,
            &jp2::CodestreamRegistration {
                codestream_index: 0,
                horizontal_subsampling: 2,
                vertical_subsampling: 1,
                horizontal_offset: 0,
                vertical_offset: 0,
            },
            1,
            1,
        )
        .unwrap();
        assert_eq!(registered.width, 2);
        assert_eq!(registered.colors[0], [12_345, 12_345]);
        let red = OwnedLayer {
            width: 1,
            height: 1,
            color_space: ConvertedColorSpace::Srgb,
            colors: vec![vec![u16::MAX], vec![0], vec![0]],
            alpha: None,
        };
        let placement = |layer_index| JpxInstructionInfo {
            layer_index,
            target_x: 0,
            target_y: 0,
            target_width: 1,
            target_height: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            persistent: true,
            next_reuse: 0,
        };
        let rendered = render_decoded_layers(
            1,
            1,
            &[(gray, placement(0)), (red, placement(1))],
            ResourceLimits::default().maximum_decoded_bytes(),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(rendered.color_space(), ConvertedColorSpace::Srgb);
        assert_eq!(rendered.channel(0).unwrap().samples(), &[u16::MAX]);
    }
}
