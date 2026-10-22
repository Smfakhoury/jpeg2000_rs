use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits,
    codestream::{ProgressionChange, ProgressionChanges, ProgressionOrder, TilePartPayload},
    tier1::{SubbandOrientation, Tier1CodeBlockDecoder, Tier1CodingStyle},
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION: usize = 164;
const SOP_MARKER: [u8; 2] = [0xff, 0x91];
const EPH_MARKER: [u8; 2] = [0xff, 0x92];

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PacketBitReader<'a> {
    input: &'a [u8],
    byte_index: usize,
    current_byte: u8,
    bits_remaining: u8,
    previous_byte_was_ff: bool,
    bits_read: u64,
    maximum_work_units: u64,
    maximum_header_bytes: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn packet_bit_reader_view(reader: &PacketBitReader<'_>) -> crate::specs::Part1PacketReaderState {
        crate::specs::Part1PacketReaderState {
            input: reader.input@, byte_index: reader.byte_index, current_byte: reader.current_byte,
            bits_remaining: reader.bits_remaining, previous_byte_was_ff: reader.previous_byte_was_ff,
            bits_read: reader.bits_read, maximum_work_units: reader.maximum_work_units,
            maximum_header_bytes: reader.maximum_header_bytes,
        }
    }

    pub open spec fn packet_reader_outcome(result: Result<PacketBitReader<'_>, DecodeError>)
        -> Result<crate::specs::Part1PacketReaderState, DecodeError>
    {
        match result { Ok(reader) => Ok(packet_bit_reader_view(&reader)), Err(error) => Err(error) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_reader_error(offset, kind),
))]
pub(crate) fn packet_reader_error(offset: usize, kind: MalformedKind) -> DecodeError {
    DecodeError::MalformedInput {
        offset: offset as u64,
        kind,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_reader_limit(resource, limit, actual),
))]
fn packet_reader_limit(resource: Resource, limit: u64, actual: u64) -> DecodeError {
    DecodeError::LimitExceeded {
        resource,
        limit,
        actual,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures packet_bit_reader_view(&result) == crate::specs::part1_packet_reader_prefix(input@, header_limit, work_limit),
))]
pub(crate) fn packet_reader_prefix_from_input(
    input: &[u8],
    header_limit: u64,
    work_limit: u64,
) -> PacketBitReader<'_> {
    PacketBitReader {
        input,
        byte_index: 0,
        current_byte: 0,
        bits_remaining: 0,
        previous_byte_was_ff: false,
        bits_read: 0,
        maximum_work_units: work_limit,
        maximum_header_bytes: header_limit,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures packet_reader_outcome(result) == crate::specs::part1_packet_reader_initial(input@, header_limit, work_limit),
))]
pub(crate) fn packet_reader_from_input(
    input: &[u8],
    header_limit: u64,
    work_limit: u64,
) -> Result<PacketBitReader<'_>, DecodeError> {
    let input_length = input.len() as u64;
    if input_length > header_limit {
        return Err(packet_reader_limit(
            Resource::PacketHeaderBytes,
            header_limit,
            input_length,
        ));
    }
    Ok(packet_reader_prefix_from_input(
        input,
        header_limit,
        work_limit,
    ))
}

impl<'a> PacketBitReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        packet_reader_from_input(
            input,
            limits.maximum_packet_header_bytes(),
            limits.maximum_work_units(),
        )
    }

    fn new_prefix(input: &'a [u8], limits: &ResourceLimits) -> Self {
        packet_reader_prefix_from_input(
            input,
            limits.maximum_packet_header_bytes(),
            limits.maximum_work_units(),
        )
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packet_bit_reader_view(self).bits_read,))]
    pub const fn bits_read(&self) -> u64 {
        self.bits_read
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packet_bit_reader_view(self).byte_index,))]
    pub const fn bytes_consumed(&self) -> usize {
        self.byte_index
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (packet_bit_reader_view(self).bits_remaining == 0),))]
    pub const fn is_byte_aligned(&self) -> bool {
        self.bits_remaining == 0
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures packet_bit_reader_view(final(self)) == (crate::specs::Part1PacketReaderState {
            bits_remaining: 0, ..packet_bit_reader_view(old(self))
        }),
    ))]
    pub fn align_to_byte(&mut self) {
        self.bits_remaining = 0;
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, packet_bit_reader_view(final(self))) == crate::specs::part1_packet_reader_read(packet_bit_reader_view(old(self))),
    ))]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_reader_read); }
        if self.bits_remaining == 0 {
            self.load_byte()?;
        }
        if self.bits_remaining == 0 || self.bits_remaining > 8 {
            return Err(packet_reader_error(
                self.byte_index,
                MalformedKind::InvalidBitStuffing,
            ));
        }
        self.bits_remaining = match self.bits_remaining.checked_sub(1) {
            Some(count) => count,
            None => {
                return Err(packet_reader_error(
                    self.byte_index,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        let bit = crate::refinement::packet_bit(self.current_byte, self.bits_remaining);
        self.bits_read = match self.bits_read.checked_add(1) {
            Some(count) => count,
            None => {
                return Err(packet_reader_error(
                    self.byte_index,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        if self.bits_read > self.maximum_work_units {
            return Err(packet_reader_limit(
                Resource::WorkUnits,
                self.maximum_work_units,
                self.bits_read,
            ));
        }
        Ok(bit)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, packet_bit_reader_view(final(self))) == crate::specs::part1_packet_reader_read_bits(packet_bit_reader_view(old(self)), count),
    ))]
    pub fn read_bits(&mut self, count: u8) -> Result<u32, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = packet_bit_reader_view(self); }
        if count > 32 {
            return Err(packet_reader_error(
                self.byte_index,
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut value = 0_u32;
        let mut read = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant read <= count,
                    count <= 32, initial == packet_bit_reader_view(old(self)),
                    crate::specs::part1_packet_reader_read_bits(initial, count)
                        == crate::specs::part1_packet_reader_word(packet_bit_reader_view(self), (count - read) as nat, value),
                decreases count - read,
            )
        )]
        while read < count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_packet_reader_word); }
            value <<= 1;
            #[cfg(verus_keep_ghost)]
            proof! { assert(value | 0u32 == value) by(bit_vector); }
            if self.read_bit()? {
                value |= 1;
            }
            read = match read.checked_add(1) {
                Some(count) => count,
                None => {
                    return Err(packet_reader_error(
                        self.byte_index,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_reader_word); }
        Ok(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, packet_bit_reader_view(final(self))) == crate::specs::part1_packet_reader_load(packet_bit_reader_view(old(self))),
    ))]
    fn load_byte(&mut self) -> Result<(), DecodeError> {
        if self.byte_index as u64 >= self.maximum_header_bytes {
            let actual = match (self.byte_index as u64).checked_add(1) {
                Some(actual) => actual,
                None => {
                    return Err(packet_reader_error(
                        self.byte_index,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            return Err(packet_reader_limit(
                Resource::PacketHeaderBytes,
                self.maximum_header_bytes,
                actual,
            ));
        }
        if self.byte_index >= self.input.len() {
            return Err(packet_reader_error(
                self.byte_index,
                MalformedKind::Truncated,
            ));
        }
        let byte = self.input[self.byte_index];
        if self.previous_byte_was_ff && byte & 0x80 != 0 {
            return Err(packet_reader_error(
                self.byte_index,
                MalformedKind::InvalidBitStuffing,
            ));
        }
        self.byte_index = match self.byte_index.checked_add(1) {
            Some(index) => index,
            None => {
                return Err(packet_reader_error(
                    self.byte_index,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        self.current_byte = byte;
        self.bits_remaining = crate::refinement::packet_data_bits(self.previous_byte_was_ff);
        self.previous_byte_was_ff = byte == 0xff;
        Ok(())
    }
}

#[cfg_attr(not(verus_keep_ghost), derive(Clone, Debug, Eq, PartialEq))]
pub struct PacketGrid {
    layers: u16,
    components: u16,
    resolutions: u8,
    component_resolutions: Vec<u8>,
    maximum_precincts_per_resolution: u32,
    precinct_counts: Vec<u32>,
    precinct_offsets: Vec<usize>,
    candidate_count: u64,
    packet_count: u64,
}

impl PacketGrid {
    pub fn total_precinct_count(&self) -> Result<u64, DecodeError> {
        self.packet_count
            .checked_div(u64::from(self.layers))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
    }

    pub fn new(
        layers: u16,
        components: u16,
        resolutions: u8,
        precinct_counts: Vec<u32>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        Self::new_with_component_resolutions(
            layers,
            components,
            vec![resolutions; usize::from(components)],
            precinct_counts,
            limits,
        )
    }

    pub(crate) fn new_with_component_resolutions(
        layers: u16,
        components: u16,
        component_resolutions: Vec<u8>,
        precinct_counts: Vec<u32>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        Self::new_with_selected_resolutions(
            layers,
            components,
            component_resolutions,
            precinct_counts,
            None,
            limits,
        )
    }

    pub(crate) fn new_with_selected_resolutions(
        layers: u16,
        components: u16,
        component_resolutions: Vec<u8>,
        precinct_counts: Vec<u32>,
        selected_resolution_counts: Option<&[u8]>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        let mut resolutions = 0_u8;
        let mut component_index = 0_usize;
        let mut has_zero_resolution = false;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= component_resolutions.len(),
                decreases component_resolutions.len() - component_index,
            )
        )]
        while component_index < component_resolutions.len() {
            let component_resolution = *component_resolutions
                .get(component_index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
            if component_resolution == 0 {
                has_zero_resolution = true;
            }
            resolutions = resolutions.max(component_resolution);
            component_index = component_index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        if layers == 0 || components == 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        if resolutions == 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        if component_resolutions.len() != usize::from(components) || has_zero_resolution {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        if layers > limits.maximum_layers() {
            return Err(DecodeError::limit(
                Resource::Layers,
                u64::from(limits.maximum_layers()),
                u64::from(layers),
            ));
        }
        if components > limits.maximum_components() {
            return Err(DecodeError::limit(
                Resource::Components,
                u64::from(limits.maximum_components()),
                u64::from(components),
            ));
        }
        if resolutions > limits.maximum_resolutions_per_component() {
            return Err(DecodeError::limit(
                Resource::Resolutions,
                u64::from(limits.maximum_resolutions_per_component()),
                u64::from(resolutions),
            ));
        }
        let expected_counts = usize::from(components)
            .checked_mul(usize::from(resolutions))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if precinct_counts.len() != expected_counts {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }

        let mut total_precincts = 0_u64;
        let mut maximum_precincts_per_resolution = 0_u32;
        let mut precinct_offsets = Vec::with_capacity(
            expected_counts
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        let mut index = 0_usize;
        let mut count_component = 0_usize;
        let mut count_resolution = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= precinct_counts.len(),
                decreases precinct_counts.len() - index,
            )
        )]
        while index < precinct_counts.len() {
            precinct_offsets.push(
                usize::try_from(total_precincts)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            );
            let precincts = precinct_counts[index];
            let valid_resolution =
                count_resolution
                    < usize::from(*component_resolutions.get(count_component).ok_or_else(
                        || DecodeError::malformed(0, MalformedKind::InvalidCodingStyle),
                    )?);
            if !valid_resolution && precincts != 0 {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
            }
            total_precincts = total_precincts
                .checked_add(u64::from(precincts))
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if precincts > maximum_precincts_per_resolution {
                maximum_precincts_per_resolution = precincts;
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            count_resolution = count_resolution
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if count_resolution == usize::from(resolutions) {
                count_resolution = 0;
                count_component = count_component
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            }
        }
        precinct_offsets.push(
            usize::try_from(total_precincts)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        let mut selected_precincts = total_precincts;
        if let Some(selected_counts) = selected_resolution_counts {
            if selected_counts.len() != usize::from(components) {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
            }
            selected_precincts = 0;
            let mut selected_component = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant selected_component <= selected_counts.len(),
                    decreases selected_counts.len() - selected_component,
                )
            )]
            while selected_component < selected_counts.len() {
                let selected_count = *selected_counts
                    .get(selected_component)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
                let available = *component_resolutions
                    .get(selected_component)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
                if selected_count == 0 || selected_count > available {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
                }
                let mut selected_resolution = 0_u8;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant selected_resolution <= selected_count,
                        decreases selected_count - selected_resolution,
                    )
                )]
                while selected_resolution < selected_count {
                    let flat = selected_component
                        .checked_mul(usize::from(resolutions))
                        .and_then(|value| value.checked_add(usize::from(selected_resolution)))
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?;
                    selected_precincts = selected_precincts
                        .checked_add(u64::from(*precinct_counts.get(flat).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)
                        })?))
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?;
                    selected_resolution = selected_resolution.checked_add(1).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                }
                selected_component = selected_component
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            }
        }
        if selected_precincts > limits.maximum_precincts() {
            return Err(DecodeError::limit(
                Resource::Precincts,
                limits.maximum_precincts(),
                selected_precincts,
            ));
        }
        let packet_count = total_precincts
            .checked_mul(u64::from(layers))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let selected_packet_count = selected_precincts
            .checked_mul(u64::from(layers))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if selected_packet_count > limits.maximum_packets() {
            return Err(DecodeError::limit(
                Resource::Packets,
                limits.maximum_packets(),
                selected_packet_count,
            ));
        }
        let candidate_count = u64::from(layers)
            .checked_mul(u64::from(components))
            .and_then(|value| value.checked_mul(u64::from(resolutions)))
            .and_then(|value| value.checked_mul(u64::from(maximum_precincts_per_resolution)))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if candidate_count > limits.maximum_work_units() {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                limits.maximum_work_units(),
                candidate_count,
            ));
        }
        Ok(Self {
            layers,
            components,
            resolutions,
            component_resolutions,
            maximum_precincts_per_resolution,
            precinct_counts,
            precinct_offsets,
            candidate_count,
            packet_count,
        })
    }

    pub const fn packet_count(&self) -> u64 {
        self.packet_count
    }

    pub(crate) fn selected_packet_count(
        &self,
        resolution_counts: &[u8],
    ) -> Result<u64, DecodeError> {
        if resolution_counts.len() != usize::from(self.components) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        let mut precincts = 0_u64;
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= self.components,
                decreases self.components - component,
            )
        )]
        while component < self.components {
            let count = *resolution_counts
                .get(usize::from(component))
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
            if count == 0
                || count
                    > *self
                        .component_resolutions
                        .get(usize::from(component))
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)
                        })?
            {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
            }
            let mut resolution = 0_u8;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant resolution <= count,
                    decreases count - resolution,
                )
            )]
            while resolution < count {
                precincts = precincts
                    .checked_add(u64::from(self.precinct_count(component, resolution)?))
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                resolution = resolution
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            }
            component = component
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        precincts
            .checked_mul(u64::from(self.layers))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
    }

    pub fn precinct_count(&self, component: u16, resolution: u8) -> Result<u32, DecodeError> {
        if component >= self.components || resolution >= self.resolutions {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        let component_resolution_count = *self
            .component_resolutions
            .get(usize::from(component))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
        if resolution >= component_resolution_count {
            return Ok(0);
        }
        let index = usize::from(component)
            .checked_mul(usize::from(self.resolutions))
            .and_then(|value| value.checked_add(usize::from(resolution)))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if index >= self.precinct_counts.len() {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        Ok(self.precinct_counts[index])
    }

    pub fn precinct_index(
        &self,
        component: u16,
        resolution: u8,
        precinct: u32,
    ) -> Result<usize, DecodeError> {
        let count = self.precinct_count(component, resolution)?;
        if precinct >= count {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }

        let component_resolution = usize::from(component)
            .checked_mul(usize::from(self.resolutions))
            .and_then(|value| value.checked_add(usize::from(resolution)))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let start = *self
            .precinct_offsets
            .get(component_resolution)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
        start
            .checked_add(
                usize::try_from(precinct)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            )
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
    }
}

pub(crate) fn decode_jpip_precinct_packet(
    template: &TilePartPayload<'_>,
    data: &[u8],
    expected_sop_sequence: u16,
    layer: u16,
    precinct: &mut PacketPrecinctDecoder,
    limits: &ResourceLimits,
) -> Result<usize, DecodeError> {
    let states = precinct.states.clone();
    let contexts = precinct.contexts.clone();
    let subbands = precinct.subbands.clone();
    let result = decode_jpip_precinct_packet_inner(
        template,
        data,
        expected_sop_sequence,
        layer,
        precinct,
        limits,
    );
    if result.is_err() {
        precinct.states = states;
        precinct.contexts = contexts;
        precinct.subbands = subbands;
    }
    result
}

fn decode_jpip_precinct_packet_inner(
    template: &TilePartPayload<'_>,
    data: &[u8],
    expected_sop_sequence: u16,
    layer: u16,
    precinct: &mut PacketPrecinctDecoder,
    limits: &ResourceLimits,
) -> Result<usize, DecodeError> {
    let mut context_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant context_index <= precinct.contexts.len(),
            decreases precinct.contexts.len() - context_index,
        )
    )]
    while context_index < precinct.contexts.len() {
        let context = precinct
            .contexts
            .get_mut(context_index)
            .ok_or_else(invalid_packet_header)?;
        context.layer = layer;
        context_index = context_index
            .checked_add(1)
            .ok_or_else(packet_arithmetic_overflow)?;
    }
    let inline = TilePartPayload {
        tile_index: template.tile_index,
        tile_part_index: 0,
        tile_part_count: 1,
        coding_style: template.coding_style,
        quantization: template.quantization,
        component_coding_styles: template.component_coding_styles,
        component_quantizations: template.component_quantizations.clone(),
        roi_shifts: template.roi_shifts,
        progression_changes: template.progression_changes.clone(),
        multiple_component_transform: template.multiple_component_transform.clone(),
        part2_extensions: template.part2_extensions.clone(),
        packet_lengths: crate::codestream::PacketLengths::none(),
        packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
        has_functional_header_marker: template.has_functional_header_marker,
        has_roi_marker: template.has_roi_marker,
        data,
    };
    let (decoded, consumed, packed) = decode_precinct_packet(
        &inline,
        0,
        &[],
        0,
        None,
        expected_sop_sequence,
        precinct,
        false,
        limits,
    )?;
    if packed != 0
        || consumed
            != usize::try_from(decoded.bytes_consumed)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
    {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidPacketLength,
        ));
    }
    Ok(consumed)
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, Hash, PartialEq))]
pub struct PacketCoordinate {
    pub layer: u16,
    pub resolution: u8,
    pub component: u16,
    pub precinct: u32,
}

pub struct PacketIterator<'a> {
    grid: &'a PacketGrid,
    order: ProgressionOrder,
    candidate_index: u64,
    emitted: u64,
}

impl<'a> PacketIterator<'a> {
    pub const fn new(grid: &'a PacketGrid, order: ProgressionOrder) -> Self {
        Self {
            grid,
            order,
            candidate_index: 0,
            emitted: 0,
        }
    }

    pub const fn emitted(&self) -> u64 {
        self.emitted
    }

    pub fn next_packet(&mut self) -> Result<Option<PacketCoordinate>, DecodeError> {
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                decreases self.grid.candidate_count as int - self.candidate_index as int,
            )
        )]
        while self.candidate_index < self.grid.candidate_count {
            let candidate = self.candidate_index;
            self.candidate_index = self
                .candidate_index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            let coordinate = self.decode_candidate(candidate)?;
            if coordinate.precinct
                < self
                    .grid
                    .precinct_count(coordinate.component, coordinate.resolution)?
            {
                self.emitted = self
                    .emitted
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                if self.emitted > self.grid.packet_count {
                    return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
                }
                return Ok(Some(coordinate));
            }
        }
        Ok(None)
    }

    fn decode_candidate(&self, candidate: u64) -> Result<PacketCoordinate, DecodeError> {
        let layers = u64::from(self.grid.layers);
        let resolutions = u64::from(self.grid.resolutions);
        let components = u64::from(self.grid.components);
        let precincts = u64::from(self.grid.maximum_precincts_per_resolution);
        let (layer, resolution, component, precinct) = match self.order {
            ProgressionOrder::Lrcp => {
                let values = unravel(candidate, [layers, resolutions, components, precincts])?;
                (values[0], values[1], values[2], values[3])
            }
            ProgressionOrder::Rlcp => {
                let values = unravel(candidate, [resolutions, layers, components, precincts])?;
                (values[1], values[0], values[2], values[3])
            }
            ProgressionOrder::Rpcl => {
                let values = unravel(candidate, [resolutions, precincts, components, layers])?;
                (values[3], values[0], values[2], values[1])
            }
            ProgressionOrder::Pcrl => {
                let values = unravel(candidate, [precincts, components, resolutions, layers])?;
                (values[3], values[2], values[1], values[0])
            }
            ProgressionOrder::Cprl => {
                let values = unravel(candidate, [components, precincts, resolutions, layers])?;
                (values[3], values[2], values[0], values[1])
            }
        };
        Ok(PacketCoordinate {
            layer: u16::try_from(layer)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            resolution: u8::try_from(resolution)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            component: u16::try_from(component)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            precinct: u32::try_from(precinct)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        })
    }
}

#[cfg(not(verus_keep_ghost))]
impl Iterator for PacketIterator<'_> {
    type Item = Result<PacketCoordinate, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_packet().transpose()
    }
}

pub struct ProgressionPacketIterator<'a> {
    grid: &'a PacketGrid,
    changes: &'a ProgressionChanges,
    default: Option<PacketIterator<'a>>,
    window_index: usize,
    window_candidate: u64,
    window_candidates: u64,
    emitted: u64,
}

impl<'a> ProgressionPacketIterator<'a> {
    pub fn new(
        grid: &'a PacketGrid,
        default_order: ProgressionOrder,
        changes: &'a ProgressionChanges,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        let default = if changes.is_empty() {
            Some(PacketIterator::new(grid, default_order))
        } else {
            None
        };
        let mut candidate_work = 0_u64;
        let change_count = changes.records().len();
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases change_count - index,)
        )]
        while index < change_count {
            let record = *changes.records().get(index).ok_or_else(|| {
                DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
            })?;
            candidate_work = candidate_work
                .checked_add(window_candidate_count(grid, record)?)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if candidate_work > limits.maximum_work_units() {
                return Err(DecodeError::limit(
                    Resource::WorkUnits,
                    limits.maximum_work_units(),
                    candidate_work,
                ));
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        let window_candidates = match changes.records().first() {
            Some(record) => window_candidate_count(grid, *record)?,
            None => 0,
        };
        Ok(Self {
            grid,
            changes,
            default,
            window_index: 0,
            window_candidate: 0,
            window_candidates,
            emitted: 0,
        })
    }

    pub const fn emitted(&self) -> u64 {
        self.emitted
    }

    pub fn next_packet(&mut self) -> Result<Option<PacketCoordinate>, DecodeError> {
        if let Some(default) = self.default.as_mut() {
            let coordinate = default.next_packet()?;
            self.emitted = default.emitted();
            return Ok(coordinate);
        }
        let change_count = self.changes.records().len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases change_count - self.window_index,)
        )]
        while self.window_index < change_count {
            let record = *self
                .changes
                .records()
                .get(self.window_index)
                .ok_or_else(|| {
                    DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
                })?;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(decreases self.window_candidates - self.window_candidate,)
            )]
            while self.window_candidate < self.window_candidates {
                let candidate = self.window_candidate;
                self.window_candidate = self
                    .window_candidate
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let coordinate = decode_window_candidate(self.grid, record, candidate)?;
                if coordinate.precinct
                    >= self
                        .grid
                        .precinct_count(coordinate.component, coordinate.resolution)?
                {
                    continue;
                }
                let mut prior = 0_usize;
                let mut already_emitted = false;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant prior <= self.window_index,
                        decreases self.window_index - prior,
                    )
                )]
                while prior < self.window_index {
                    let prior_record = *self.changes.records().get(prior).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
                    })?;
                    if prior_record.contains(
                        coordinate.layer,
                        coordinate.resolution,
                        coordinate.component,
                    ) {
                        already_emitted = true;
                        break;
                    }
                    prior = prior.checked_add(1).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                }
                if !crate::refinement::progression_emit_allowed(already_emitted) {
                    continue;
                }
                self.emitted = self
                    .emitted
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                if self.emitted > self.grid.packet_count {
                    return Err(DecodeError::malformed(
                        0,
                        MalformedKind::InvalidProgressionOrderChange,
                    ));
                }
                return Ok(Some(coordinate));
            }
            self.window_index = self
                .window_index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            self.window_candidate = 0;
            self.window_candidates = match self.changes.records().get(self.window_index) {
                Some(next) => window_candidate_count(self.grid, *next)?,
                None => 0,
            };
        }
        if self.emitted != self.grid.packet_count {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidProgressionOrderChange,
            ));
        }
        Ok(None)
    }
}

fn window_candidate_count(
    grid: &PacketGrid,
    record: ProgressionChange,
) -> Result<u64, DecodeError> {
    u64::from(record.layer_end)
        .checked_mul(u64::from(
            record
                .resolution_end
                .checked_sub(record.resolution_start)
                .ok_or_else(|| {
                    DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
                })?,
        ))
        .and_then(|value| {
            value.checked_mul(u64::from(
                record.component_end.saturating_sub(record.component_start),
            ))
        })
        .and_then(|value| value.checked_mul(u64::from(grid.maximum_precincts_per_resolution)))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
}

fn decode_window_candidate(
    grid: &PacketGrid,
    record: ProgressionChange,
    candidate: u64,
) -> Result<PacketCoordinate, DecodeError> {
    let layers = u64::from(record.layer_end);
    let resolutions = u64::from(
        record
            .resolution_end
            .checked_sub(record.resolution_start)
            .ok_or_else(|| {
                DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
            })?,
    );
    let components = u64::from(
        record
            .component_end
            .checked_sub(record.component_start)
            .ok_or_else(|| {
                DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
            })?,
    );
    let precincts = u64::from(grid.maximum_precincts_per_resolution);
    let (layer, resolution, component, precinct) = match record.progression_order {
        ProgressionOrder::Lrcp => {
            let values = unravel(candidate, [layers, resolutions, components, precincts])?;
            (values[0], values[1], values[2], values[3])
        }
        ProgressionOrder::Rlcp => {
            let values = unravel(candidate, [resolutions, layers, components, precincts])?;
            (values[1], values[0], values[2], values[3])
        }
        ProgressionOrder::Rpcl => {
            let values = unravel(candidate, [resolutions, precincts, components, layers])?;
            (values[3], values[0], values[2], values[1])
        }
        ProgressionOrder::Pcrl => {
            let values = unravel(candidate, [precincts, components, resolutions, layers])?;
            (values[3], values[2], values[1], values[0])
        }
        ProgressionOrder::Cprl => {
            let values = unravel(candidate, [components, precincts, resolutions, layers])?;
            (values[3], values[2], values[0], values[1])
        }
    };
    Ok(PacketCoordinate {
        layer: u16::try_from(layer)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        resolution: u8::try_from(resolution)
            .ok()
            .and_then(|value| value.checked_add(record.resolution_start))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        component: u16::try_from(component)
            .ok()
            .and_then(|value| value.checked_add(record.component_start))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        precinct: u32::try_from(precinct)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
    })
}
fn unravel(mut index: u64, dimensions: [u64; 4]) -> Result<[u64; 4], DecodeError> {
    if dimensions[0] == 0 || dimensions[1] == 0 || dimensions[2] == 0 || dimensions[3] == 0 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
    }
    let third = crate::refinement::progression_digit(index, dimensions[3]);
    index = crate::refinement::progression_advance(index, dimensions[3]);
    let second = crate::refinement::progression_digit(index, dimensions[2]);
    index = crate::refinement::progression_advance(index, dimensions[2]);
    let first = crate::refinement::progression_digit(index, dimensions[1]);
    index = crate::refinement::progression_advance(index, dimensions[1]);
    if index >= dimensions[0] {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
    }
    Ok([index, first, second, third])
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockPacketState {
    included: bool,
    zero_bitplanes: Option<u16>,
    lblock: u8,
    total_passes: u32,
    high_throughput: Option<bool>,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn code_block_packet_state_view(state: &CodeBlockPacketState) -> crate::specs::Part1CodeBlockPacketState {
        crate::specs::Part1CodeBlockPacketState {
            included: state.included, zero_bitplanes: state.zero_bitplanes, lblock: state.lblock,
            total_passes: state.total_passes, high_throughput: state.high_throughput,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures code_block_packet_state_view(&result) == crate::specs::part1_code_block_packet_initial(),
))]
pub(crate) const fn code_block_packet_state_new() -> CodeBlockPacketState {
    CodeBlockPacketState {
        included: false,
        zero_bitplanes: None,
        lblock: 3,
        total_passes: 0,
        high_throughput: None,
    }
}

impl CodeBlockPacketState {
    pub const fn new() -> Self {
        code_block_packet_state_new()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == code_block_packet_state_view(self).included,))]
    pub const fn included(&self) -> bool {
        self.included
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == code_block_packet_state_view(self).zero_bitplanes,))]
    pub const fn zero_bitplanes(&self) -> Option<u16> {
        self.zero_bitplanes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == code_block_packet_state_view(self).lblock,))]
    pub const fn lblock(&self) -> u8 {
        self.lblock
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == code_block_packet_state_view(self).total_passes,))]
    pub const fn total_passes(&self) -> u32 {
        self.total_passes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == code_block_packet_state_view(self).high_throughput,))]
    pub const fn high_throughput(&self) -> Option<bool> {
        self.high_throughput
    }
}

impl Default for CodeBlockPacketState {
    fn default() -> Self {
        code_block_packet_state_new()
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockContribution {
    pub first_inclusion: bool,
    pub zero_bitplanes: u16,
    pub new_passes: u8,
    pub data_length: u32,
    pub high_throughput: bool,
    segment_count: u8,
    segment_passes: [u8; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
    segment_lengths: [u32; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn code_block_contribution_view(contribution: &CodeBlockContribution) -> crate::specs::Part1CodeBlockContribution {
        crate::specs::Part1CodeBlockContribution {
            first_inclusion: contribution.first_inclusion,
            zero_bitplanes: contribution.zero_bitplanes,
            new_passes: contribution.new_passes,
            data_length: contribution.data_length,
            high_throughput: contribution.high_throughput,
            segment_count: contribution.segment_count,
            segment_passes: contribution.segment_passes@,
            segment_lengths: contribution.segment_lengths@,
        }
    }

    pub open spec fn code_block_contribution_option_view(contribution: Option<CodeBlockContribution>)
        -> Option<crate::specs::Part1CodeBlockContribution>
    {
        match contribution { None => None, Some(value) => Some(code_block_contribution_view(&value)) }
    }

    pub open spec fn code_block_header_outcome(result: Result<Option<CodeBlockContribution>, DecodeError>)
        -> Result<Option<crate::specs::Part1CodeBlockContribution>, DecodeError>
    {
        match result {
            Err(error) => Err(error),
            Ok(contribution) => Ok(code_block_contribution_option_view(contribution)),
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockSegmentContribution {
    pub passes: u8,
    pub data_length: u32,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockBodyDecode {
    pub segments: u8,
    pub passes: u32,
    pub bytes: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct HtBodyProgress {
    pub segment_index: u8,
    pub offset: usize,
    pub decoded_passes: u32,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_body_progress_view(progress: &HtBodyProgress) -> crate::specs::HtPacketBodyProgress {
        crate::specs::HtPacketBodyProgress {
            segment_index: progress.segment_index,
            offset: progress.offset,
            decoded_passes: progress.decoded_passes,
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PacketBodyDecode {
    pub present: bool,
    pub code_blocks: u32,
    pub contributions: u32,
    pub segments: u32,
    pub passes: u32,
    pub header_bytes: u32,
    pub body_bytes: u32,
    pub bytes_consumed: u32,
}

pub struct PacketCodeBlockExecution<'a> {
    pub states: &'a mut [CodeBlockPacketState],
    pub inclusion_tree: &'a mut TagTree,
    pub zero_bitplane_tree: &'a mut TagTree,
    pub contexts: &'a [CodeBlockPacketContext],
    pub decoders: &'a mut [Tier1CodeBlockDecoder],
}

pub struct PacketPrecinctDecoder {
    component: u16,
    resolution: u8,
    precinct: u32,
    states: Vec<CodeBlockPacketState>,
    contexts: Vec<CodeBlockPacketContext>,
    decoders: Vec<Tier1CodeBlockDecoder>,
    subbands: Vec<PacketSubbandState>,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn packet_precinct_decoders(value: &PacketPrecinctDecoder) -> Seq<Tier1CodeBlockDecoder> {
        value.decoders@
    }

    pub open spec fn packet_decoder_outcome(result: Result<&Tier1CodeBlockDecoder, DecodeError>)
        -> Result<Tier1CodeBlockDecoder, DecodeError>
    {
        match result { Ok(value) => Ok(*value), Err(error) => Err(error) }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PacketPrecinctGeometry {
    pub component: u16,
    pub resolution: u8,
    pub precinct: u32,
    pub code_blocks_across: u32,
    pub code_blocks_down: u32,
    pub maximum_zero_bitplanes: u16,
}

pub struct PacketSubbandExecution {
    pub orientation: SubbandOrientation,
    pub code_blocks_across: u32,
    pub code_blocks_down: u32,
    pub maximum_zero_bitplanes: u16,
    pub decoders: Vec<Tier1CodeBlockDecoder>,
}

#[derive(Clone)]
struct PacketSubbandState {
    orientation: SubbandOrientation,
    first_decoder: usize,
    decoder_count: usize,
    code_blocks_across: u32,
    code_blocks_down: u32,
    inclusion_tree: TagTree,
    zero_bitplane_tree: TagTree,
}

impl PacketPrecinctDecoder {
    pub fn new(
        geometry: PacketPrecinctGeometry,
        tier1_style: Tier1CodingStyle,
        decoders: Vec<Tier1CodeBlockDecoder>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        Self::new_subbands(
            geometry.component,
            geometry.resolution,
            geometry.precinct,
            tier1_style,
            vec![PacketSubbandExecution {
                orientation: SubbandOrientation::Ll,
                code_blocks_across: geometry.code_blocks_across,
                code_blocks_down: geometry.code_blocks_down,
                maximum_zero_bitplanes: geometry.maximum_zero_bitplanes,
                decoders,
            }],
            limits,
        )
    }

    /// An empty execution list represents a precinct with no nonempty subbands.
    pub fn new_subbands(
        component: u16,
        resolution: u8,
        precinct: u32,
        tier1_style: Tier1CodingStyle,
        mut executions: Vec<PacketSubbandExecution>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        if executions.len() > 3 {
            return Err(invalid_packet_header());
        }
        let mut states = Vec::new();
        let mut contexts = Vec::new();
        let mut decoders = Vec::new();
        let mut subbands = Vec::with_capacity(executions.len());
        let mut execution_index = 0_usize;
        let mut total_code_blocks = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant execution_index <= executions.len(),
                decreases executions.len() - execution_index,
            )
        )]
        while execution_index < executions.len() {
            let execution = executions
                .get_mut(execution_index)
                .ok_or_else(invalid_packet_header)?;
            if execution.code_blocks_across == 0
                || execution.code_blocks_down == 0
                || execution.maximum_zero_bitplanes == 0
                || execution.maximum_zero_bitplanes > crate::tier1::MAX_TIER1_BITPLANES
            {
                return Err(invalid_packet_header());
            }
            let count = u64::from(execution.code_blocks_across)
                .checked_mul(u64::from(execution.code_blocks_down))
                .ok_or_else(packet_arithmetic_overflow)?;
            total_code_blocks = total_code_blocks
                .checked_add(count)
                .ok_or_else(packet_arithmetic_overflow)?;
            if total_code_blocks > limits.maximum_code_blocks() {
                return Err(DecodeError::limit(
                    Resource::CodeBlocks,
                    limits.maximum_code_blocks(),
                    total_code_blocks,
                ));
            }
            if total_code_blocks > limits.maximum_work_units() {
                return Err(DecodeError::limit(
                    Resource::WorkUnits,
                    limits.maximum_work_units(),
                    total_code_blocks,
                ));
            }
            let count = usize::try_from(count).map_err(|_| packet_arithmetic_overflow())?;
            if execution.decoders.len() != count {
                return Err(invalid_packet_header());
            }
            let first_decoder = decoders.len();
            let mut local_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant local_index <= count,
                    decreases count - local_index,
                )
            )]
            while local_index < count {
                let linear_index =
                    u32::try_from(local_index).map_err(|_| packet_arithmetic_overflow())?;
                states.push(CodeBlockPacketState::new());
                contexts.push(CodeBlockPacketContext {
                    leaf_x: linear_index
                        .checked_rem(execution.code_blocks_across)
                        .ok_or_else(packet_arithmetic_overflow)?,
                    leaf_y: linear_index
                        .checked_div(execution.code_blocks_across)
                        .ok_or_else(packet_arithmetic_overflow)?,
                    layer: 0,
                    maximum_zero_bitplanes: execution.maximum_zero_bitplanes,
                    tier1_style,
                });
                local_index = local_index
                    .checked_add(1)
                    .ok_or_else(packet_arithmetic_overflow)?;
            }
            decoders.append(&mut execution.decoders);
            subbands.push(PacketSubbandState {
                orientation: execution.orientation,
                first_decoder,
                decoder_count: count,
                code_blocks_across: execution.code_blocks_across,
                code_blocks_down: execution.code_blocks_down,
                inclusion_tree: TagTree::new(
                    execution.code_blocks_across,
                    execution.code_blocks_down,
                    limits,
                )?,
                zero_bitplane_tree: TagTree::new(
                    execution.code_blocks_across,
                    execution.code_blocks_down,
                    limits,
                )?,
            });
            execution_index = execution_index
                .checked_add(1)
                .ok_or_else(packet_arithmetic_overflow)?;
        }
        Ok(Self {
            component,
            resolution,
            precinct,
            states,
            contexts,
            decoders,
            subbands,
        })
    }

    pub const fn coordinate(&self) -> (u16, u8, u32) {
        (self.component, self.resolution, self.precinct)
    }

    pub fn states(&self) -> &[CodeBlockPacketState] {
        &self.states
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result@ == packet_precinct_decoders(self),
    ))]
    pub fn decoders(&self) -> &[Tier1CodeBlockDecoder] {
        &self.decoders
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result as nat == crate::specs::part1_packet_decoder_count(packet_precinct_decoders(self)),
    ))]
    pub const fn decoder_count(&self) -> usize {
        self.decoders.len()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packet_decoder_outcome(result) == crate::specs::part1_packet_precinct_decoder(packet_precinct_decoders(self), index),
    ))]
    pub fn decoder(&self, index: usize) -> Result<&Tier1CodeBlockDecoder, DecodeError> {
        self.decoders.get(index).ok_or_else(invalid_packet_header)
    }

    pub const fn subband_count(&self) -> usize {
        self.subbands.len()
    }

    pub fn subband_orientation(&self, index: usize) -> Result<SubbandOrientation, DecodeError> {
        self.subbands
            .get(index)
            .map(|subband| subband.orientation)
            .ok_or_else(invalid_packet_header)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TilePacketDecode {
    pub tile_index: u16,
    pub tile_parts: u32,
    pub packets: u64,
    pub present_packets: u64,
    pub contributions: u64,
    pub segments: u64,
    pub passes: u64,
    pub packet_header_bytes: u64,
    pub packet_body_bytes: u64,
    pub payload_bytes: u64,
}

pub fn decode_progression_tile_packets(
    tile_parts: &[TilePartPayload<'_>],
    grid: &PacketGrid,
    precincts: &mut [PacketPrecinctDecoder],
    limits: &ResourceLimits,
) -> Result<TilePacketDecode, DecodeError> {
    decode_progression_tile_packets_selected(tile_parts, grid, precincts, None, limits)
}

pub(crate) fn decode_progression_tile_packets_selected(
    tile_parts: &[TilePartPayload<'_>],
    grid: &PacketGrid,
    precincts: &mut [PacketPrecinctDecoder],
    maximum_resolution_counts: Option<&[u8]>,
    limits: &ResourceLimits,
) -> Result<TilePacketDecode, DecodeError> {
    if tile_parts.is_empty() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidTilePart));
    }
    if tile_parts.len() as u64 > u64::from(limits.maximum_tile_parts()) {
        return Err(DecodeError::limit(
            Resource::TileParts,
            u64::from(limits.maximum_tile_parts()),
            tile_parts.len() as u64,
        ));
    }
    let first = tile_parts
        .first()
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
    let mut maximum_resolution_levels = 0_u8;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= grid.components,
            decreases grid.components - component,
        )
    )]
    while component < grid.components {
        let resolution_levels = first
            .component_coding_styles
            .style(component)?
            .decomposition_levels
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        maximum_resolution_levels = maximum_resolution_levels.max(resolution_levels);
        component = component
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    if first.coding_style.layers != grid.layers || maximum_resolution_levels != grid.resolutions {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
    }
    if let Some(counts) = maximum_resolution_counts {
        if counts.len() != usize::from(grid.components) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= counts.len(),
                decreases counts.len() - index,
            )
        )]
        while index < counts.len() {
            let count = *counts
                .get(index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
            let available = *grid
                .component_resolutions
                .get(index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
            if count == 0 || count > available {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
    }
    let tile_index = first.tile_index;
    let mut payload_bytes = 0_u64;
    let mut packed_payload_bytes = 0_u64;
    let mut declared_packets = 0_u64;
    let mut all_packet_lengths_present = true;
    let mut packed_headers = Vec::with_capacity(tile_parts.len());
    let mut tile_part_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant tile_part_index <= tile_parts.len(),
            decreases tile_parts.len() - tile_part_index,
        )
    )]
    while tile_part_index < tile_parts.len() {
        let tile_part = tile_parts
            .get(tile_part_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        let expected_index = u8::try_from(tile_part_index)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        if tile_part.tile_index != tile_index
            || tile_part.tile_part_index != expected_index
            || (tile_part.tile_part_count != 0
                && usize::from(tile_part.tile_part_count) != tile_parts.len())
        {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTilePart));
        }
        payload_bytes = payload_bytes
            .checked_add(tile_part.data.len() as u64)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let reassembled = tile_part.packed_packet_headers.reassemble(limits)?;
        packed_payload_bytes = packed_payload_bytes
            .checked_add(reassembled.len() as u64)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if packed_payload_bytes > limits.maximum_packet_header_bytes() {
            return Err(DecodeError::limit(
                Resource::PacketHeaderBytes,
                limits.maximum_packet_header_bytes(),
                packed_payload_bytes,
            ));
        }
        packed_headers.push(reassembled);
        if tile_part.packet_lengths.is_present() {
            declared_packets = declared_packets
                .checked_add(tile_part.packet_lengths.packet_count())
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        } else {
            all_packet_lengths_present = false;
        }
        tile_part_index = tile_part_index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    if all_packet_lengths_present && declared_packets != grid.packet_count {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidPacketLength,
        ));
    }

    let precinct_count = usize::try_from(
        grid.packet_count
            .checked_div(u64::from(grid.layers))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
    )
    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    if precincts.len() != precinct_count {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let mut precinct_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant precinct_index <= precincts.len(),
            decreases precincts.len() - precinct_index,
        )
    )]
    while precinct_index < precincts.len() {
        let precinct = precincts
            .get(precinct_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidPacketHeader))?;
        let expected =
            grid.precinct_index(precinct.component, precinct.resolution, precinct.precinct)?;
        if expected != precinct_index {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidPacketHeader,
            ));
        }
        precinct_index = precinct_index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }

    let mut iterator = ProgressionPacketIterator::new(
        grid,
        first.coding_style.progression_order,
        &first.progression_changes,
        limits,
    )?;
    let mut current_tile_part = 0_usize;
    let mut length_tile_part = 0_usize;
    let mut length_iterator = first.packet_lengths.iter();
    let mut offset = 0_usize;
    let mut packed_offset = 0_usize;
    let mut packets = 0_u64;
    let mut present_packets = 0_u64;
    let mut contributions = 0_u64;
    let mut segments = 0_u64;
    let mut passes = 0_u64;
    let mut packet_header_bytes = 0_u64;
    let mut packet_body_bytes = 0_u64;
    let seen_length = usize::try_from(grid.packet_count)
        .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let mut seen = vec![false; seen_length];
    let mut remaining_packets = limits.maximum_work_units();
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases remaining_packets,)
    )]
    'packet: while let Some(coordinate) = iterator.next_packet()? {
        if remaining_packets == 0 {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidProgressionOrderChange,
            ));
        }
        remaining_packets -= 1;
        let previous_tile_part = current_tile_part;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases tile_parts.len() - current_tile_part,)
        )]
        while current_tile_part < tile_parts.len() {
            let current = tile_parts
                .get(current_tile_part)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
            let current_packed = packed_headers
                .get(current_tile_part)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
            if offset < current.data.len() || packed_offset < current_packed.len() {
                break;
            }
            if offset != current.data.len() || packed_offset != current_packed.len() {
                return Err(DecodeError::malformed(
                    offset as u64,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
            if current.packet_lengths.is_present()
                && length_tile_part == current_tile_part
                && length_iterator.next_length()?.is_some()
            {
                return Err(DecodeError::malformed(
                    offset as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
            current_tile_part = current_tile_part
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            offset = 0;
            packed_offset = 0;
        }
        if current_tile_part != previous_tile_part {
            let Some(next) = tile_parts.get(current_tile_part) else {
                break;
            };
            iterator = ProgressionPacketIterator::new(
                grid,
                next.coding_style.progression_order,
                &next.progression_changes,
                limits,
            )?;
            continue 'packet;
        }
        let tile_part = tile_parts
            .get(current_tile_part)
            .ok_or_else(|| DecodeError::malformed(payload_bytes, MalformedKind::Truncated))?;
        if length_tile_part != current_tile_part {
            length_iterator = tile_part.packet_lengths.iter();
            length_tile_part = current_tile_part;
        }
        let expected_packet_length = if tile_part.packet_lengths.is_present() {
            Some(length_iterator.next_length()?.ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketLength)
            })?)
        } else {
            None
        };
        let execution_index = grid.precinct_index(
            coordinate.component,
            coordinate.resolution,
            coordinate.precinct,
        )?;
        let seen_index = execution_index
            .checked_mul(usize::from(grid.layers))
            .and_then(|value| value.checked_add(usize::from(coordinate.layer)))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let seen_slot = seen.get_mut(seen_index).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
        })?;
        if !crate::refinement::progression_emit_allowed(*seen_slot) {
            continue;
        }
        let precinct = precincts.get_mut(execution_index).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
        })?;
        let mut context_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant context_index <= precinct.contexts.len(),
                decreases precinct.contexts.len() - context_index,
            )
        )]
        while context_index < precinct.contexts.len() {
            let context = precinct
                .contexts
                .get_mut(context_index)
                .ok_or_else(invalid_packet_header)?;
            context.layer = coordinate.layer;
            context_index = context_index
                .checked_add(1)
                .ok_or_else(packet_arithmetic_overflow)?;
        }
        let packed = packed_headers
            .get(current_tile_part)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        let decode_tier1 = match maximum_resolution_counts {
            Some(counts) => {
                coordinate.resolution
                    < *counts
                        .get(usize::from(coordinate.component))
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)
                        })?
            }
            None => true,
        };
        let (decoded, tile_bytes_consumed, packed_bytes_consumed) = decode_precinct_packet(
            tile_part,
            offset,
            packed,
            packed_offset,
            expected_packet_length,
            u16::try_from(packets & u64::from(u16::MAX))
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            precinct,
            decode_tier1,
            limits,
        )?;
        if decoded.bytes_consumed == 0 {
            return Err(DecodeError::malformed(
                offset as u64,
                MalformedKind::InvalidPacketHeader,
            ));
        }
        match expected_packet_length {
            Some(expected) if decoded.bytes_consumed != expected => {
                return Err(DecodeError::malformed(
                    offset as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
            _ => {}
        }
        offset = offset.checked_add(tile_bytes_consumed).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
        packed_offset = packed_offset
            .checked_add(packed_bytes_consumed)
            .ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
            })?;
        packets = packets
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        *seen_slot = true;
        present_packets = present_packets
            .checked_add(u64::from(decoded.present))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        contributions = contributions
            .checked_add(u64::from(decoded.contributions))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        segments = segments
            .checked_add(u64::from(decoded.segments))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        passes = passes
            .checked_add(u64::from(decoded.passes))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        packet_header_bytes = packet_header_bytes
            .checked_add(u64::from(decoded.header_bytes))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        packet_body_bytes = packet_body_bytes
            .checked_add(u64::from(decoded.body_bytes))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases tile_parts.len() - current_tile_part,)
    )]
    while current_tile_part < tile_parts.len() {
        let current = tile_parts
            .get(current_tile_part)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        let current_packed = packed_headers
            .get(current_tile_part)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        if offset != current.data.len() || packed_offset != current_packed.len() {
            return Err(DecodeError::malformed(
                offset as u64,
                MalformedKind::TrailingData,
            ));
        }
        if current.packet_lengths.is_present()
            && length_tile_part == current_tile_part
            && length_iterator.next_length()?.is_some()
        {
            return Err(DecodeError::malformed(
                offset as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        current_tile_part = current_tile_part
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        offset = 0;
        packed_offset = 0;
    }
    if packets != grid.packet_count {
        return Err(DecodeError::malformed(
            payload_bytes,
            MalformedKind::Truncated,
        ));
    }
    Ok(TilePacketDecode {
        tile_index,
        tile_parts: u32::try_from(tile_parts.len())
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        packets,
        present_packets,
        contributions,
        segments,
        passes,
        packet_header_bytes,
        packet_body_bytes,
        payload_bytes,
    })
}

#[derive(Clone, Copy)]
struct PacketBodyPreflight {
    index: usize,
    end: usize,
}

#[derive(Clone, Copy)]
struct PacketBodyExecutionProgress {
    index: usize,
    offset: usize,
    segments: u32,
    passes: u32,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn packet_body_contributions_view(values: &[Option<CodeBlockContribution>])
        -> Seq<Option<crate::specs::Part1CodeBlockContribution>>
    {
        values@.map_values(|value: Option<CodeBlockContribution>| code_block_contribution_option_view(value))
    }

    pub closed spec fn packet_body_preflight_view(value: &PacketBodyPreflight)
        -> crate::specs::Part1PacketBodyPreflight
    {
        crate::specs::Part1PacketBodyPreflight { index: value.index, end: value.end }
    }

    pub closed spec fn packet_body_execution_view(
        decoders: &[Tier1CodeBlockDecoder], progress: &PacketBodyExecutionProgress,
    ) -> crate::specs::Part1PacketBodyExecution {
        crate::specs::Part1PacketBodyExecution {
            decoders: decoders@.map_values(|decoder: Tier1CodeBlockDecoder| crate::tier1::tier1_decoder_state(&decoder)),
            index: progress.index, offset: progress.offset, segments: progress.segments, passes: progress.passes,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, packet_body_preflight_view(final(progress))) == crate::specs::part1_packet_body_preflight(
        packet_body_preflight_view(old(progress)), packet_body_contributions_view(contributions), data@, error_offset),
))]
fn preflight_packet_body_ranges(
    contributions: &[Option<CodeBlockContribution>],
    data: &[u8],
    progress: &mut PacketBodyPreflight,
    error_offset: usize,
) -> Result<(), DecodeError> {
    let slots = contributions.len();
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_body_preflight); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            slots == contributions.len(),
            crate::specs::part1_packet_body_preflight(packet_body_preflight_view(old(progress)),
                packet_body_contributions_view(contributions), data@, error_offset)
            == crate::specs::part1_packet_body_preflight(packet_body_preflight_view(progress),
                packet_body_contributions_view(contributions), data@, error_offset),
        decreases slots as int - progress.index as int,
    ))]
    while progress.index < slots {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_body_preflight); }
        if let Some(contribution) = contributions[progress.index] {
            progress.end = progress
                .end
                .checked_add(
                    usize::try_from(contribution.data_length)
                        .map_err(|_| packet_arithmetic_overflow())?,
                )
                .ok_or_else(packet_arithmetic_overflow)?;
            if progress.end > data.len() {
                return Err(DecodeError::MalformedInput {
                    offset: error_offset as u64,
                    kind: MalformedKind::Truncated,
                });
            }
        }
        progress.index = progress
            .index
            .checked_add(1)
            .ok_or_else(packet_arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_body_preflight); }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, packet_body_execution_view(final(decoders), final(progress)))
        == crate::specs::part1_packet_body_execute(packet_body_execution_view(old(decoders), old(progress)),
            packet_body_contributions_view(contributions), data@, error_offset, decode_tier1,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
))]
fn execute_packet_body_contributions(
    decoders: &mut [Tier1CodeBlockDecoder],
    contributions: &[Option<CodeBlockContribution>],
    data: &[u8],
    progress: &mut PacketBodyExecutionProgress,
    error_offset: usize,
    decode_tier1: bool,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let slots = contributions.len();
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_body_execute); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            slots == contributions.len(),
            crate::specs::part1_packet_body_execute(packet_body_execution_view(old(decoders), old(progress)),
                packet_body_contributions_view(contributions), data@, error_offset, decode_tier1,
                crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
                crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits))
            == crate::specs::part1_packet_body_execute(packet_body_execution_view(decoders, progress),
                packet_body_contributions_view(contributions), data@, error_offset, decode_tier1,
                crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
                crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
        decreases slots as int - progress.index as int,
    ))]
    while progress.index < slots {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_body_execute); }
        if let Some(contribution) = contributions[progress.index] {
            let body_end = progress
                .offset
                .checked_add(
                    usize::try_from(contribution.data_length)
                        .map_err(|_| packet_arithmetic_overflow())?,
                )
                .ok_or_else(packet_arithmetic_overflow)?;
            let body = data
                .get(progress.offset..body_end)
                .ok_or(DecodeError::MalformedInput {
                    offset: error_offset as u64,
                    kind: MalformedKind::Truncated,
                })?;
            if decode_tier1 {
                #[cfg(verus_keep_ghost)]
                proof_decl! {
                    let ghost prior_decoders = decoders@;
                    let ghost prior_states = packet_body_execution_view(decoders, progress).decoders;
                    let ghost selected_index = progress.index;
                }
                let decoded_result = {
                    let decoder = decoders
                        .get_mut(progress.index)
                        .ok_or_else(invalid_packet_header)?;
                    decode_code_block_contribution_body(decoder, &contribution, body, limits)
                };
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(decoders@.len() == prior_decoders.len());
                    assert forall |i: int| 0 <= i < decoders@.len() && i != selected_index as int implies
                        decoders@[i] == prior_decoders[i] by {}
                    assert(packet_body_execution_view(decoders, progress).decoders =~=
                        prior_states.update(selected_index as int,
                            crate::tier1::tier1_decoder_state(&decoders@[selected_index as int])));
                }
                let decoded = decoded_result?;
                progress.segments = progress
                    .segments
                    .checked_add(u32::from(decoded.segments))
                    .ok_or_else(packet_arithmetic_overflow)?;
                progress.passes = progress
                    .passes
                    .checked_add(decoded.passes)
                    .ok_or_else(packet_arithmetic_overflow)?;
            }
            progress.offset = body_end;
        }
        progress.index = progress
            .index
            .checked_add(1)
            .ok_or_else(packet_arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_body_execute); }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decode_precinct_packet(
    tile_part: &TilePartPayload<'_>,
    offset: usize,
    packed_headers: &[u8],
    packed_offset: usize,
    expected_packet_length: Option<u32>,
    expected_sop_sequence: u16,
    precinct: &mut PacketPrecinctDecoder,
    decode_tier1: bool,
    limits: &ResourceLimits,
) -> Result<(PacketBodyDecode, usize, usize), DecodeError> {
    let component = precinct.component;
    let PacketPrecinctDecoder {
        states,
        contexts,
        decoders,
        subbands,
        ..
    } = precinct;
    if states.len() != contexts.len()
        || states.len() != decoders.len()
        || states.is_empty() != subbands.is_empty()
    {
        return Err(invalid_packet_header());
    }
    let code_blocks = states.len() as u64;
    if code_blocks > limits.maximum_code_blocks() {
        return Err(DecodeError::limit(
            Resource::CodeBlocks,
            limits.maximum_code_blocks(),
            code_blocks,
        ));
    }
    if code_blocks > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            code_blocks,
        ));
    }
    let data = tile_part
        .data
        .get(offset..)
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketLength))?;
    let tier1_style = tile_part
        .component_coding_styles
        .style(component)?
        .tier1_coding_style()?;
    let mut tile_offset = 0_usize;
    let mut packed_bytes_consumed = 0_usize;
    if tile_part.coding_style.sop_markers {
        let marker = data
            .get(0..2)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        let marker_length = data
            .get(2..4)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        if !crate::refinement::sop_valid(marker[0], marker[1], marker_length[0], marker_length[1]) {
            return Err(invalid_packet_header());
        }
        let sequence = data
            .get(4..6)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        let sequence_number = crate::refinement::sop_sequence(sequence[0], sequence[1]);
        if sequence_number != expected_sop_sequence {
            return Err(invalid_packet_header());
        }
        tile_offset = 6;
    }
    let packed = tile_part.packed_packet_headers.is_present();
    let packet_data = if packed {
        packed_headers
            .get(packed_offset..)
            .ok_or_else(|| DecodeError::malformed(packed_offset as u64, MalformedKind::Truncated))?
    } else {
        data.get(tile_offset..)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?
    };
    let mut reader = PacketBitReader::new_prefix(packet_data, limits);
    let packet_present = reader.read_bit()?;
    let mut contributions = vec![None; states.len()];
    let mut contribution_count = 0_u32;
    if packet_present && !contexts.is_empty() {
        let layer = contexts.first().ok_or_else(invalid_packet_header)?.layer;
        let subband_count = subbands.len();
        let mut subband_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant subband_index <= subband_count,
                decreases subband_count - subband_index,
            )
        )]
        while subband_index < subband_count {
            let subband = subbands
                .get_mut(subband_index)
                .ok_or_else(invalid_packet_header)?;
            let expected_count = usize::try_from(
                u64::from(subband.code_blocks_across)
                    .checked_mul(u64::from(subband.code_blocks_down))
                    .ok_or_else(packet_arithmetic_overflow)?,
            )
            .map_err(|_| packet_arithmetic_overflow())?;
            if expected_count != subband.decoder_count
                || subband.inclusion_tree.leaf_count()? != expected_count as u64
                || subband.zero_bitplane_tree.leaf_count()? != expected_count as u64
            {
                return Err(invalid_packet_header());
            }
            let decoder_count = subband.decoder_count;
            let mut local_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant local_index <= decoder_count,
                    decreases decoder_count - local_index,
                )
            )]
            while local_index < decoder_count {
                let block_index = subband
                    .first_decoder
                    .checked_add(local_index)
                    .ok_or_else(packet_arithmetic_overflow)?;
                let context = contexts
                    .get(block_index)
                    .ok_or_else(invalid_packet_header)?;
                let linear_index =
                    u32::try_from(local_index).map_err(|_| packet_arithmetic_overflow())?;
                let expected_x = linear_index
                    .checked_rem(subband.code_blocks_across)
                    .ok_or_else(packet_arithmetic_overflow)?;
                let expected_y = linear_index
                    .checked_div(subband.code_blocks_across)
                    .ok_or_else(packet_arithmetic_overflow)?;
                if context.layer != layer
                    || context.leaf_x != expected_x
                    || context.leaf_y != expected_y
                    || !tier1_styles_equal(context.tier1_style, tier1_style)
                {
                    return Err(invalid_packet_header());
                }
                let state = states
                    .get_mut(block_index)
                    .ok_or_else(invalid_packet_header)?;
                let contribution = decode_code_block_contribution(
                    state,
                    &mut subband.inclusion_tree,
                    &mut subband.zero_bitplane_tree,
                    *context,
                    &mut reader,
                )?;
                if contribution.is_some() {
                    contribution_count = contribution_count
                        .checked_add(1)
                        .ok_or_else(packet_arithmetic_overflow)?;
                }
                let slot = contributions
                    .get_mut(block_index)
                    .ok_or_else(invalid_packet_header)?;
                *slot = contribution;
                local_index = local_index
                    .checked_add(1)
                    .ok_or_else(packet_arithmetic_overflow)?;
            }
            subband_index = subband_index
                .checked_add(1)
                .ok_or_else(packet_arithmetic_overflow)?;
        }
    }
    reader.align_to_byte();
    let header_bytes = reader.bytes_consumed();
    if packed {
        packed_bytes_consumed = header_bytes;
    } else {
        tile_offset = tile_offset
            .checked_add(header_bytes)
            .ok_or_else(packet_arithmetic_overflow)?;
    }
    if tile_part.coding_style.eph_markers {
        let marker_start = header_bytes;
        let marker_end = marker_start
            .checked_add(2)
            .ok_or_else(packet_arithmetic_overflow)?;
        let marker = packet_data
            .get(marker_start..marker_end)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        if !crate::refinement::eph_valid(marker[0], marker[1]) {
            return Err(invalid_packet_header());
        }
        if packed {
            packed_bytes_consumed = marker_end;
        } else {
            tile_offset = tile_offset
                .checked_add(2)
                .ok_or_else(packet_arithmetic_overflow)?;
        }
    }
    let body_start = tile_offset;
    let mut preflight = PacketBodyPreflight {
        index: 0,
        end: tile_offset,
    };
    preflight_packet_body_ranges(&contributions, data, &mut preflight, offset)?;
    tile_offset = preflight.end;
    let mut execution = PacketBodyExecutionProgress {
        index: 0,
        offset: body_start,
        segments: 0,
        passes: 0,
    };
    execute_packet_body_contributions(
        decoders,
        &contributions,
        data,
        &mut execution,
        offset,
        decode_tier1,
        limits,
    )?;
    let body_bytes = tile_offset
        .checked_sub(body_start)
        .ok_or_else(packet_arithmetic_overflow)?;
    if tile_offset > usize::MAX - packed_bytes_consumed {
        return Err(packet_arithmetic_overflow());
    }
    let logical_bytes = crate::refinement::packet_logical_bytes(tile_offset, packed_bytes_consumed);
    let packet_length_matches = match expected_packet_length {
        Some(expected) => crate::refinement::packet_length_matches(
            logical_bytes,
            usize::try_from(expected).map_err(|_| packet_arithmetic_overflow())?,
        ),
        None => true,
    };
    if !packet_length_matches {
        return Err(DecodeError::malformed(
            offset as u64,
            MalformedKind::InvalidPacketLength,
        ));
    }
    Ok((
        PacketBodyDecode {
            present: packet_present,
            code_blocks: u32::try_from(code_blocks).map_err(|_| packet_arithmetic_overflow())?,
            contributions: contribution_count,
            segments: execution.segments,
            passes: execution.passes,
            header_bytes: u32::try_from(header_bytes).map_err(|_| packet_arithmetic_overflow())?,
            body_bytes: u32::try_from(body_bytes).map_err(|_| packet_arithmetic_overflow())?,
            bytes_consumed: u32::try_from(logical_bytes)
                .map_err(|_| packet_arithmetic_overflow())?,
        },
        tile_offset,
        packed_bytes_consumed,
    ))
}

pub fn decode_multi_code_block_packet(
    tile_part: &TilePartPayload<'_>,
    offset: usize,
    execution: PacketCodeBlockExecution<'_>,
    limits: &ResourceLimits,
) -> Result<PacketBodyDecode, DecodeError> {
    let states = execution.states;
    let inclusion_tree = execution.inclusion_tree;
    let zero_bitplane_tree = execution.zero_bitplane_tree;
    let contexts = execution.contexts;
    let decoders = execution.decoders;
    if states.is_empty() || states.len() != contexts.len() || states.len() != decoders.len() {
        return Err(DecodeError::malformed(
            offset as u64,
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let code_blocks = states.len() as u64;
    if code_blocks > limits.maximum_code_blocks() {
        return Err(DecodeError::limit(
            Resource::CodeBlocks,
            limits.maximum_code_blocks(),
            code_blocks,
        ));
    }
    if code_blocks > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            code_blocks,
        ));
    }
    let inclusion_leaves = inclusion_tree
        .levels
        .first()
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::InvalidTagTree))?;
    let zero_bitplane_leaves = zero_bitplane_tree
        .levels
        .first()
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::InvalidTagTree))?;
    let inclusion_width = inclusion_leaves.width;
    let inclusion_height = inclusion_leaves.height;
    if inclusion_width == 0 || inclusion_height == 0 {
        return Err(DecodeError::malformed(
            offset as u64,
            MalformedKind::InvalidTagTree,
        ));
    }
    if inclusion_tree.leaf_count()? != code_blocks
        || zero_bitplane_tree.leaf_count()? != code_blocks
        || inclusion_width != zero_bitplane_leaves.width
        || inclusion_height != zero_bitplane_leaves.height
    {
        return Err(DecodeError::malformed(
            offset as u64,
            MalformedKind::InvalidTagTree,
        ));
    }
    let data = tile_part
        .data
        .get(offset..)
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
    let tier1_style = tile_part.coding_style.tier1_coding_style()?;
    let layer = contexts
        .first()
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader))?
        .layer;
    let block_count = contexts.len();
    let mut context_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant context_index <= block_count,
            decreases block_count - context_index,
        )
    )]
    while context_index < block_count {
        let context = contexts.get(context_index).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
        })?;
        let linear_index = u32::try_from(context_index).map_err(|_| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
        let expected_x = linear_index.checked_rem(inclusion_width).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
        let expected_y = linear_index.checked_div(inclusion_width).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
        if context.layer != layer
            || context.leaf_x != expected_x
            || context.leaf_y != expected_y
            || !tier1_styles_equal(context.tier1_style, tier1_style)
        {
            return Err(DecodeError::malformed(
                offset as u64,
                MalformedKind::InvalidPacketHeader,
            ));
        }
        context_index = context_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
    }
    let mut packet_offset = 0_usize;
    if tile_part.coding_style.sop_markers {
        let marker = data
            .get(0..2)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        let marker_length = data
            .get(2..4)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        if marker[0] != SOP_MARKER[0]
            || marker[1] != SOP_MARKER[1]
            || marker_length[0] != 0
            || marker_length[1] != 4
        {
            return Err(DecodeError::malformed(
                offset as u64,
                MalformedKind::InvalidPacketHeader,
            ));
        }
        data.get(4..6)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        packet_offset = 6;
    }
    let packet_data = data
        .get(packet_offset..)
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
    let mut reader = PacketBitReader::new_prefix(packet_data, limits);
    let packet_present = reader.read_bit()?;
    let mut contributions = vec![None; states.len()];
    let mut contribution_count = 0_u32;
    if packet_present {
        let mut block_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant block_index <= block_count,
                block_count == contributions.len(),
                decreases block_count - block_index,
            )
        )]
        while block_index < block_count {
            let context = contexts.get(block_index).ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
            })?;
            let state = states.get_mut(block_index).ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
            })?;
            let contribution = decode_code_block_contribution(
                state,
                inclusion_tree,
                zero_bitplane_tree,
                *context,
                &mut reader,
            )?;
            if contribution.is_some() {
                contribution_count = contribution_count.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
                })?;
            }
            let slot = contributions.get_mut(block_index).ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
            })?;
            *slot = contribution;
            block_index = block_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
            })?;
        }
    }
    reader.align_to_byte();
    let header_bytes = reader.bytes_consumed();
    packet_offset = packet_offset
        .checked_add(header_bytes)
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow))?;
    if tile_part.coding_style.eph_markers {
        let marker_end = packet_offset.checked_add(2).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
        let marker = data
            .get(packet_offset..marker_end)
            .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
        if marker[0] != EPH_MARKER[0] || marker[1] != EPH_MARKER[1] {
            return Err(DecodeError::malformed(
                offset as u64,
                MalformedKind::InvalidPacketHeader,
            ));
        }
        packet_offset = marker_end;
    }

    let body_start = packet_offset;
    let mut block_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant block_index <= block_count,
            block_count == contributions.len(),
            decreases block_count - block_index,
        )
    )]
    while block_index < block_count {
        if let Some(contribution) = contributions[block_index] {
            packet_offset = packet_offset
                .checked_add(usize::try_from(contribution.data_length).map_err(|_| {
                    DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
                })?)
                .ok_or_else(|| {
                    DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
                })?;
            if packet_offset > data.len() {
                return Err(DecodeError::malformed(
                    offset as u64,
                    MalformedKind::Truncated,
                ));
            }
        }
        block_index = block_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
    }

    let mut body_offset = body_start;
    let mut decoded_segments = 0_u32;
    let mut decoded_passes = 0_u32;
    block_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant block_index <= block_count,
            block_count == contributions.len(),
            decreases block_count - block_index,
        )
    )]
    while block_index < block_count {
        if let Some(contribution) = contributions[block_index] {
            let body_end = body_offset
                .checked_add(usize::try_from(contribution.data_length).map_err(|_| {
                    DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
                })?)
                .ok_or_else(|| {
                    DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
                })?;
            let body = data
                .get(body_offset..body_end)
                .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::Truncated))?;
            let decoder = decoders.get_mut(block_index).ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::InvalidPacketHeader)
            })?;
            let decoded =
                decode_code_block_contribution_body(decoder, &contribution, body, limits)?;
            decoded_segments = decoded_segments
                .checked_add(u32::from(decoded.segments))
                .ok_or_else(|| {
                    DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
                })?;
            decoded_passes = decoded_passes.checked_add(decoded.passes).ok_or_else(|| {
                DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
            })?;
            body_offset = body_end;
        }
        block_index = block_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
    }
    let body_bytes = packet_offset
        .checked_sub(body_start)
        .ok_or_else(|| DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow))?;
    Ok(PacketBodyDecode {
        present: packet_present,
        code_blocks: u32::try_from(code_blocks).map_err(|_| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?,
        contributions: contribution_count,
        segments: decoded_segments,
        passes: decoded_passes,
        header_bytes: u32::try_from(header_bytes).map_err(|_| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?,
        body_bytes: u32::try_from(body_bytes).map_err(|_| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?,
        bytes_consumed: u32::try_from(packet_offset).map_err(|_| {
            DecodeError::malformed(offset as u64, MalformedKind::ArithmeticOverflow)
        })?,
    })
}

pub fn decode_single_code_block_packet(
    payload: &[u8],
    state: &mut CodeBlockPacketState,
    inclusion_tree: &mut TagTree,
    zero_bitplane_tree: &mut TagTree,
    context: CodeBlockPacketContext,
    decoder: &mut Tier1CodeBlockDecoder,
    limits: &ResourceLimits,
) -> Result<Option<CodeBlockBodyDecode>, DecodeError> {
    let mut reader = PacketBitReader::new(payload, limits)?;
    if !reader.read_bit()? {
        reader.align_to_byte();
        if reader.bytes_consumed() != payload.len() {
            return Err(DecodeError::malformed(
                reader.bytes_consumed() as u64,
                MalformedKind::TrailingData,
            ));
        }
        return Ok(None);
    }
    let contribution = decode_code_block_contribution(
        state,
        inclusion_tree,
        zero_bitplane_tree,
        context,
        &mut reader,
    )?
    .ok_or_else(|| {
        DecodeError::malformed(
            reader.bytes_consumed() as u64,
            MalformedKind::InvalidPacketHeader,
        )
    })?;
    reader.align_to_byte();
    let body_offset = reader.bytes_consumed();
    let body = payload
        .get(body_offset..)
        .ok_or_else(|| DecodeError::malformed(body_offset as u64, MalformedKind::Truncated))?;
    Ok(Some(decode_code_block_contribution_body(
        decoder,
        &contribution,
        body,
        limits,
    )?))
}

fn tier1_styles_equal(left: Tier1CodingStyle, right: Tier1CodingStyle) -> bool {
    matches!(
        (left.block_coding, right.block_coding),
        (
            crate::tier1::Tier1BlockCoding::Legacy,
            crate::tier1::Tier1BlockCoding::Legacy
        ) | (
            crate::tier1::Tier1BlockCoding::HighThroughput,
            crate::tier1::Tier1BlockCoding::HighThroughput
        ) | (
            crate::tier1::Tier1BlockCoding::Mixed,
            crate::tier1::Tier1BlockCoding::Mixed
        )
    ) && left.selective_arithmetic_coding_bypass == right.selective_arithmetic_coding_bypass
        && left.reset_contexts == right.reset_contexts
        && left.terminate_all_passes == right.terminate_all_passes
        && left.vertical_causal_context == right.vertical_causal_context
        && left.predictable_termination == right.predictable_termination
        && left.segmentation_symbols == right.segmentation_symbols
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_header_error(),
))]
fn invalid_packet_header() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidPacketHeader,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::ht_reader_error(MalformedKind::ArithmeticOverflow),
))]
fn packet_arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::ht_reader_error(MalformedKind::InvalidCodeBlock),
))]
fn packet_invalid_code_block() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidCodeBlock,
    }
}

impl CodeBlockContribution {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == code_block_contribution_view(self).segment_count,
    ))]
    pub const fn segment_count(&self) -> u8 {
        self.segment_count
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_code_block_segment(code_block_contribution_view(self), index),
    ))]
    pub fn segment(&self, index: u8) -> Result<CodeBlockSegmentContribution, DecodeError> {
        if index >= self.segment_count {
            return Err(invalid_packet_header());
        }
        let index = usize::from(index);
        if index >= self.segment_passes.len() || index >= self.segment_lengths.len() {
            return Err(invalid_packet_header());
        }
        Ok(CodeBlockSegmentContribution {
            passes: self.segment_passes[index],
            data_length: self.segment_lengths[index],
        })
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::part1_code_block_body_transaction(
            crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
        code_block_contribution_view(contribution).high_throughput ==>
        (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::ht_packet_body(
            crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_work_limit(limits)),
        !code_block_contribution_view(contribution).high_throughput ==>
        (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::part1_legacy_code_block_transaction(
            crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
))]
pub fn decode_code_block_contribution_body(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
    limits: &ResourceLimits,
) -> Result<CodeBlockBodyDecode, DecodeError> {
    let mut staged = decoder.clone();
    let decoded =
        decode_code_block_contribution_body_staged(&mut staged, contribution, body, limits)?;
    *decoder = staged;
    Ok(decoded)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::part1_code_block_body_prepare(
        crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@),
))]
fn prepare_code_block_contribution_body(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
) -> Result<(), DecodeError> {
    let declared_length = match usize::try_from(contribution.data_length) {
        Ok(length) => length,
        Err(_) => return Err(packet_arithmetic_overflow()),
    };
    if !crate::refinement::exact_consumption(body.len(), declared_length) {
        return Err(packet_invalid_code_block());
    }
    if !decoder.accepts_high_throughput_route(contribution.high_throughput) {
        return Err(packet_invalid_code_block());
    }
    decoder.set_zero_bitplanes(contribution.zero_bitplanes)?;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::part1_code_block_body_staged(
            crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
        code_block_contribution_view(contribution).high_throughput ==>
        (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::ht_packet_body_staged(
            crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_work_limit(limits)),
        !code_block_contribution_view(contribution).high_throughput ==>
        (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::part1_legacy_code_block_staged(
            crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
))]
fn decode_code_block_contribution_body_staged(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
    limits: &ResourceLimits,
) -> Result<CodeBlockBodyDecode, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_code_block_staged); }
    prepare_code_block_contribution_body(decoder, contribution, body)?;
    if contribution.high_throughput {
        return decode_ht_contribution_segments(decoder, contribution, body, limits);
    }
    decode_legacy_code_block_contribution_body(decoder, contribution, body, limits)
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn legacy_packet_progress_view(
        decoder: &Tier1CodeBlockDecoder, segment_index: u8, offset: usize, decoded_passes: u32,
    ) -> crate::specs::Part1LegacyPacketProgress {
        crate::specs::Part1LegacyPacketProgress {
            decoder: crate::tier1::tier1_decoder_state(decoder), segment_index, offset, decoded_passes,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::part1_legacy_code_block_body(
        crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
        crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
        crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
))]
fn decode_legacy_code_block_contribution_body(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
    limits: &ResourceLimits,
) -> Result<CodeBlockBodyDecode, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_code_block_body); }
    let segment_count = contribution.segment_count();
    let mut segment_index = 0_u8;
    let mut offset = 0_usize;
    let mut decoded_passes = 0_u32;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost initial = legacy_packet_progress_view(decoder, 0, 0, 0);
        let ghost goal = crate::specs::part1_legacy_code_block_run(initial, code_block_contribution_view(contribution), body@,
            segment_count as nat, crate::limits::resource_code_block_pass_limit(limits),
            crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_input_byte_limit(limits),
            crate::limits::resource_work_limit(limits));
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant segment_index <= segment_count,
                segment_count == code_block_contribution_view(contribution).segment_count,
                initial == legacy_packet_progress_view(old(decoder), 0, 0, 0),
                goal == crate::specs::part1_legacy_code_block_run(initial, code_block_contribution_view(contribution), body@,
                    segment_count as nat, crate::limits::resource_code_block_pass_limit(limits),
                    crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_input_byte_limit(limits),
                    crate::limits::resource_work_limit(limits)),
                goal == crate::specs::part1_legacy_code_block_run(
                    legacy_packet_progress_view(decoder, segment_index, offset, decoded_passes),
                    code_block_contribution_view(contribution), body@, (segment_count - segment_index) as nat,
                    crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
                    crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
            decreases segment_count - segment_index,
        )
    )]
    while segment_index < segment_count {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_legacy_code_block_body);
            reveal(crate::specs::part1_legacy_code_block_run);
        }
        decode_legacy_code_block_segment(
            decoder,
            contribution,
            body,
            limits,
            &mut segment_index,
            &mut offset,
            &mut decoded_passes,
        )?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_code_block_run); }
    if !crate::refinement::exact_consumption(offset, body.len())
        || decoded_passes != u32::from(contribution.new_passes)
    {
        return Err(packet_invalid_code_block());
    }
    Ok(CodeBlockBodyDecode {
        segments: contribution.segment_count(),
        passes: decoded_passes,
        bytes: contribution.data_length,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::ht_packet_body_segments(
        crate::tier1::tier1_decoder_state(old(decoder)), code_block_contribution_view(contribution), body@,
        crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
        crate::limits::resource_work_limit(limits)),
))]
fn decode_ht_contribution_segments(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
    limits: &ResourceLimits,
) -> Result<CodeBlockBodyDecode, DecodeError> {
    let segment_count = contribution.segment_count();
    if segment_count == 0 {
        return Err(packet_invalid_code_block());
    }
    let mut progress = HtBodyProgress {
        segment_index: 0,
        offset: 0,
        decoded_passes: 0,
    };
    run_ht_contribution_segments(decoder, contribution, body, &mut progress, limits)?;
    if !crate::refinement::exact_consumption(progress.offset, body.len())
        || progress.decoded_passes != u32::from(contribution.new_passes)
    {
        return Err(packet_invalid_code_block());
    }
    Ok(CodeBlockBodyDecode {
        segments: segment_count,
        passes: progress.decoded_passes,
        bytes: contribution.data_length,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder)), ht_body_progress_view(final(progress)))
        == crate::specs::ht_packet_body_run(crate::tier1::tier1_decoder_state(old(decoder)),
            code_block_contribution_view(contribution), body@, ht_body_progress_view(old(progress)),
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_work_limit(limits)),
))]
fn run_ht_contribution_segments(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
    progress: &mut HtBodyProgress,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let segment_count = contribution.segment_count();
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::ht_packet_body_run); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                segment_count == code_block_contribution_view(contribution).segment_count,
                crate::specs::ht_packet_body_run(crate::tier1::tier1_decoder_state(old(decoder)),
                    code_block_contribution_view(contribution), body@, ht_body_progress_view(old(progress)),
                    crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
                    crate::limits::resource_work_limit(limits))
                    == crate::specs::ht_packet_body_run(crate::tier1::tier1_decoder_state(decoder),
                        code_block_contribution_view(contribution), body@, ht_body_progress_view(progress),
                        crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
                        crate::limits::resource_work_limit(limits)),
            decreases segment_count as int - progress.segment_index as int,
        )
    )]
    while progress.segment_index < segment_count {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_packet_body_run); }
        let segment = contribution.segment(progress.segment_index)?;
        decode_ht_segment_body(
            decoder,
            segment,
            body,
            &mut progress.offset,
            &mut progress.decoded_passes,
            limits,
        )?;
        progress.segment_index = progress
            .segment_index
            .checked_add(1)
            .ok_or_else(packet_arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::ht_packet_body_run); }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder)), *final(offset), *final(decoded_passes))
        == crate::specs::ht_packet_segment_body(crate::tier1::tier1_decoder_state(old(decoder)), segment, body@,
            *old(offset), *old(decoded_passes), crate::limits::resource_code_block_pass_limit(limits),
            crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_work_limit(limits)),
))]
fn decode_ht_segment_body(
    decoder: &mut Tier1CodeBlockDecoder,
    segment: CodeBlockSegmentContribution,
    body: &[u8],
    offset: &mut usize,
    decoded_passes: &mut u32,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    if segment.passes == 0 || segment.passes > 2 {
        return Err(packet_invalid_code_block());
    }
    let segment_length =
        usize::try_from(segment.data_length).map_err(|_| packet_arithmetic_overflow())?;
    let end = offset
        .checked_add(segment_length)
        .ok_or_else(packet_arithmetic_overflow)?;
    let segment_body = body
        .get(*offset..end)
        .ok_or_else(packet_invalid_code_block)?;
    decode_ht_segment_route(decoder, segment_body, segment.passes, limits)?;
    *decoded_passes = decoded_passes
        .checked_add(u32::from(segment.passes))
        .ok_or_else(packet_arithmetic_overflow)?;
    *offset = end;
    Ok(())
}

#[cfg(test)]
pub(crate) mod ht_route_test_support {
    use super::*;

    pub(crate) fn segment_body(
        decoder: &mut Tier1CodeBlockDecoder,
        passes: u8,
        data_length: u32,
        body: &[u8],
        offset: &mut usize,
        decoded_passes: &mut u32,
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        decode_ht_segment_body(
            decoder,
            CodeBlockSegmentContribution {
                passes,
                data_length,
            },
            body,
            offset,
            decoded_passes,
            limits,
        )
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::tier1::tier1_decoder_state(final(decoder))) == crate::specs::ht_packet_segment_route(
        crate::tier1::tier1_decoder_state(old(decoder)), body@, passes,
        crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
        crate::limits::resource_work_limit(limits)),
))]
fn decode_ht_segment_route(
    decoder: &mut Tier1CodeBlockDecoder,
    body: &[u8],
    passes: u8,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    if decoder.next_pass().is_multiple_of(3) {
        if passes != 1 {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodeBlock,
            });
        }
        if body.is_empty() {
            decoder.skip_high_throughput_segment(1, limits)?;
        } else {
            decoder.decode_high_throughput_segments(body, 1, None, 0, limits)?;
        }
    } else if decoder.ht_current_set_has_cleanup() {
        decoder.decode_high_throughput_refinement(body, passes, limits)?;
    } else {
        if !body.is_empty() {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodeBlock,
            });
        }
        decoder.skip_high_throughput_segment(passes, limits)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_packet_progress_view(final(decoder), *final(segment_index), *final(offset), *final(decoded_passes)))
        == crate::specs::part1_legacy_code_block_segment(
            legacy_packet_progress_view(old(decoder), *old(segment_index), *old(offset), *old(decoded_passes)),
            code_block_contribution_view(contribution), body@,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
        result.is_ok() ==> *old(segment_index) < code_block_contribution_view(contribution).segment_count
            && *final(segment_index) as nat == *old(segment_index) as nat + 1,
))]
fn decode_legacy_code_block_segment(
    decoder: &mut Tier1CodeBlockDecoder,
    contribution: &CodeBlockContribution,
    body: &[u8],
    limits: &ResourceLimits,
    segment_index: &mut u8,
    offset: &mut usize,
    decoded_passes: &mut u32,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_code_block_segment); }
    let segment = contribution.segment(*segment_index)?;
    let segment_length = match usize::try_from(segment.data_length) {
        Ok(length) => length,
        Err(_) => return Err(packet_arithmetic_overflow()),
    };
    let end = offset
        .checked_add(segment_length)
        .ok_or_else(packet_arithmetic_overflow)?;
    if end > body.len() {
        return Err(packet_invalid_code_block());
    }
    decoder.decode_legacy_contribution(&body[*offset..end], segment.passes, limits)?;
    *decoded_passes = decoded_passes
        .checked_add(u32::from(segment.passes))
        .ok_or_else(packet_arithmetic_overflow)?;
    *offset = end;
    *segment_index = segment_index
        .checked_add(1)
        .ok_or_else(packet_arithmetic_overflow)?;
    Ok(())
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockPacketContext {
    pub leaf_x: u32,
    pub leaf_y: u32,
    pub layer: u16,
    pub maximum_zero_bitplanes: u16,
    pub tier1_style: Tier1CodingStyle,
}

struct CodeBlockHeaderSegments {
    segment_passes: [u8; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
    segment_lengths: [u32; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
    segment_count: usize,
    accounted_passes: u32,
    data_length: u32,
    routing: Option<bool>,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn header_segments_view(segments: &CodeBlockHeaderSegments) -> crate::specs::Part1HeaderSegments {
        crate::specs::Part1HeaderSegments {
            segment_passes: segments.segment_passes@, segment_lengths: segments.segment_lengths@,
            segment_count: segments.segment_count, accounted_passes: segments.accounted_passes,
            data_length: segments.data_length, routing: segments.routing,
        }
    }

    #[verifier::inline]
    spec fn header_segments_outcome(result: Result<CodeBlockHeaderSegments, DecodeError>) -> Result<crate::specs::Part1HeaderSegments, DecodeError> {
        match result { Err(error) => Err(error), Ok(segments) => Ok(header_segments_view(&segments)) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures header_segments_view(&result) == crate::specs::part1_header_segments_initial(code_block_packet_state_view(state), style),
))]
fn header_segments_new(
    state: &CodeBlockPacketState,
    style: Tier1CodingStyle,
) -> CodeBlockHeaderSegments {
    let segment_passes = [0_u8; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
    let segment_lengths = [0_u32; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
    let segment_count = 0_usize;
    let accounted_passes = 0_u32;
    let data_length = 0_u32;
    let routing = match state.high_throughput {
        Some(selected) => Some(selected),
        None => match style.block_coding {
            crate::tier1::Tier1BlockCoding::Legacy => Some(false),
            crate::tier1::Tier1BlockCoding::HighThroughput => Some(true),
            crate::tier1::Tier1BlockCoding::Mixed if style.force_high_throughput => Some(true),
            crate::tier1::Tier1BlockCoding::Mixed => None,
        },
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(segment_passes@ =~= Seq::new(164, |i: int| 0u8));
        assert(segment_lengths@ =~= Seq::new(164, |i: int| 0u32));
    }
    CodeBlockHeaderSegments {
        segment_passes,
        segment_lengths,
        segment_count,
        accounted_passes,
        data_length,
        routing,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_packet_state_view(final(state)), header_segments_view(final(segments)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_header_segment_length(code_block_packet_state_view(old(state)), header_segments_view(old(segments)),
            packet_bit_reader_view(old(reader)), first_pass, remaining_passes, passes, length_bits),
        final(segments).segment_count == old(segments).segment_count,
        final(segments).accounted_passes == old(segments).accounted_passes,
        final(segments).data_length == old(segments).data_length,
        final(segments).segment_passes@ == old(segments).segment_passes@,
        final(segments).segment_lengths@ == old(segments).segment_lengths@,
        match result { Ok((actual_passes, _)) => actual_passes == passes || actual_passes == remaining_passes, Err(_) => true },
))]
fn read_header_segment_length(
    state: &mut CodeBlockPacketState,
    segments: &mut CodeBlockHeaderSegments,
    reader: &mut PacketBitReader<'_>,
    first_pass: u32,
    remaining_passes: u32,
    passes: u32,
    length_bits: u8,
) -> Result<(u32, u32), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_header_segment_length); }
    let segment_reader_start = *reader;
    let mut segment_length = reader.read_bits(length_bits)?;
    let mut actual_passes = passes;
    if segments.routing.is_none() {
        let legacy_length_bits = state
            .lblock
            .checked_add(floor_log2(remaining_passes))
            .ok_or_else(packet_arithmetic_overflow)?;
        if legacy_length_bits > 32 {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidPacketHeader,
            ));
        }
        let mut legacy_probe = segment_reader_start;
        let legacy_length = legacy_probe.read_bits(legacy_length_bits)?;
        let leading_zero = segment_length != 0
            && length_bits > 0
            && segment_length < (1_u32 << u32::from(length_bits - 1));
        let selects_legacy = crate::refinement::ht_mixed_selects_legacy(
            segment_length,
            legacy_length,
            first_pass.is_multiple_of(3),
            state.lblock > 3,
            leading_zero,
        );
        if selects_legacy {
            if segments.accounted_passes != 0 {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::InvalidPacketHeader,
                ));
            }
            segments.routing = Some(false);
            state.high_throughput = Some(false);
            *reader = legacy_probe;
            actual_passes = remaining_passes;
            segment_length = legacy_length;
        } else if segment_length != 0 {
            segments.routing = Some(true);
            state.high_throughput = Some(true);
        }
    }
    Ok((actual_passes, segment_length))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_packet_state_view(final(state)), tag_tree_view(final(inclusion_tree)),
        tag_tree_view(final(zero_bitplane_tree)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_header_prefix(code_block_packet_state_view(old(state)), tag_tree_view(old(inclusion_tree)),
            tag_tree_view(old(zero_bitplane_tree)), context, packet_bit_reader_view(old(reader))),
))]
fn read_header_prefix(
    state: &mut CodeBlockPacketState,
    inclusion_tree: &mut TagTree,
    zero_bitplane_tree: &mut TagTree,
    context: CodeBlockPacketContext,
    reader: &mut PacketBitReader<'_>,
) -> Result<bool, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_header_prefix);
    }
    if context.maximum_zero_bitplanes == 0
        || context.maximum_zero_bitplanes > crate::tier1::MAX_TIER1_BITPLANES
    {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let first_inclusion = !state.included;
    let included = if first_inclusion {
        let threshold = match u32::from(context.layer).checked_add(1) {
            Some(value) => value,
            None => {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        inclusion_tree
            .decode(context.leaf_x, context.leaf_y, threshold, reader)?
            .is_below_threshold
    } else {
        reader.read_bit()?
    };
    if !included {
        return Ok(false);
    }

    if first_inclusion {
        state.included = crate::refinement::packet_next_included(state.included, true);
        read_header_zero_bitplanes(state, zero_bitplane_tree, context, reader)?;
    }
    Ok(true)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (code_block_header_outcome(result), code_block_packet_state_view(final(state)), tag_tree_view(final(inclusion_tree)),
        tag_tree_view(final(zero_bitplane_tree)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_packet_header_decode(code_block_packet_state_view(old(state)), tag_tree_view(old(inclusion_tree)),
            tag_tree_view(old(zero_bitplane_tree)), context, packet_bit_reader_view(old(reader))),
        result.is_err() ==> code_block_packet_state_view(final(state)) == code_block_packet_state_view(old(state)),
))]
pub fn decode_code_block_contribution(
    state: &mut CodeBlockPacketState,
    inclusion_tree: &mut TagTree,
    zero_bitplane_tree: &mut TagTree,
    context: CodeBlockPacketContext,
    reader: &mut PacketBitReader<'_>,
) -> Result<Option<CodeBlockContribution>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_header_decode); }
    let mut next_state = *state;
    let first_inclusion = !next_state.included;
    if !read_header_prefix(
        &mut next_state,
        inclusion_tree,
        zero_bitplane_tree,
        context,
        reader,
    )? {
        return Ok(None);
    }
    let new_passes = read_new_passes(reader)?;
    read_header_lblock(&mut next_state, reader)?;
    let segments = read_header_segments(&mut next_state, new_passes, context, reader)?;
    commit_header_contribution(
        state,
        next_state,
        segments,
        first_inclusion,
        new_passes,
        reader,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (header_segments_outcome(result), code_block_packet_state_view(final(state)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_header_segments(code_block_packet_state_view(old(state)), packet_bit_reader_view(old(reader)), new_passes, context),
        match result { Ok(segments) => segments.segment_count <= 164, Err(_) => true },
))]
fn read_header_segments(
    state: &mut CodeBlockPacketState,
    new_passes: u8,
    context: CodeBlockPacketContext,
    reader: &mut PacketBitReader<'_>,
) -> Result<CodeBlockHeaderSegments, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_header_segments); }
    let mut segments = header_segments_new(state, context.tier1_style);
    let target_passes = u32::from(new_passes);
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost initial_state = code_block_packet_state_view(state);
        let ghost initial_reader = packet_bit_reader_view(reader);
        let ghost initial_segments = header_segments_view(&segments);
        let ghost goal = crate::specs::part1_header_segments_run(initial_state, initial_segments, initial_reader, new_passes, context, 165);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant segments.accounted_passes <= target_passes,
                target_passes == new_passes as u32,
                segments.segment_count <= 164,
                initial_state == code_block_packet_state_view(old(state)), initial_reader == packet_bit_reader_view(old(reader)),
                initial_segments == crate::specs::part1_header_segments_initial(initial_state, context.tier1_style),
                goal == crate::specs::part1_header_segments_run(initial_state, initial_segments, initial_reader, new_passes, context, 165),
                goal == crate::specs::part1_header_segments_run(code_block_packet_state_view(state), header_segments_view(&segments),
                    packet_bit_reader_view(reader), new_passes, context, (165 - segments.segment_count) as nat),
            decreases target_passes - segments.accounted_passes,
        )
    )]
    while segments.accounted_passes < target_passes {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_header_segments);
            reveal(crate::specs::part1_header_segments_run);
        }
        read_header_segment(state, &mut segments, new_passes, context, reader)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_header_segments_run); }
    Ok(segments)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_packet_state_view(final(state)), header_segments_view(final(segments)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_header_segment_step(code_block_packet_state_view(old(state)), header_segments_view(old(segments)),
            packet_bit_reader_view(old(reader)), new_passes, context),
        result.is_ok() ==> final(segments).segment_count == old(segments).segment_count + 1
            && old(segments).segment_count < 164
            && final(segments).accounted_passes > old(segments).accounted_passes
            && final(segments).accounted_passes <= new_passes as u32,
))]
fn read_header_segment(
    state: &mut CodeBlockPacketState,
    segments: &mut CodeBlockHeaderSegments,
    new_passes: u8,
    context: CodeBlockPacketContext,
    reader: &mut PacketBitReader<'_>,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_header_segment_step); }
    if segments.segment_count >= segments.segment_passes.len()
        || segments.segment_count >= segments.segment_lengths.len()
    {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let first_pass = match state.total_passes.checked_add(segments.accounted_passes) {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    let remaining_passes = match u32::from(new_passes).checked_sub(segments.accounted_passes) {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    let parse_as_ht = segments.routing != Some(false);
    let passes = if parse_as_ht {
        crate::refinement::ht_segment_passes(first_pass % 3, remaining_passes)
    } else {
        context
            .tier1_style
            .segment_passes(first_pass, remaining_passes)
    };
    if passes == 0 || passes > remaining_passes || passes > u32::from(u8::MAX) {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let pass_length_bits = floor_log2(passes);
    let length_bits = match state.lblock.checked_add(pass_length_bits) {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    if length_bits > 32 {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let (actual_passes, segment_length) = read_header_segment_length(
        state,
        segments,
        reader,
        first_pass,
        remaining_passes,
        passes,
        length_bits,
    )?;
    // A later layer can finish a buffered entropy segment without adding bytes.
    if segment_length == 0 && segments.routing == Some(false) && first_pass == 0 {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        ));
    }
    segments.segment_passes[segments.segment_count] = match u8::try_from(actual_passes) {
        Ok(value) => value,
        Err(_) => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    segments.segment_lengths[segments.segment_count] = segment_length;
    segments.data_length = match segments.data_length.checked_add(segment_length) {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    segments.accounted_passes = match segments.accounted_passes.checked_add(actual_passes) {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    segments.segment_count = match segments.segment_count.checked_add(1) {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::ArithmeticOverflow,
            ));
        }
    };
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (code_block_header_outcome(result), code_block_packet_state_view(final(state))) == crate::specs::part1_header_commit(
        code_block_packet_state_view(old(state)), code_block_packet_state_view(&next_state), header_segments_view(&segments),
        first_inclusion, new_passes, packet_bit_reader_view(reader)),
))]
fn commit_header_contribution(
    state: &mut CodeBlockPacketState,
    mut next_state: CodeBlockPacketState,
    segments: CodeBlockHeaderSegments,
    first_inclusion: bool,
    new_passes: u8,
    reader: &PacketBitReader<'_>,
) -> Result<Option<CodeBlockContribution>, DecodeError> {
    let CodeBlockHeaderSegments {
        segment_passes,
        segment_lengths,
        segment_count,
        data_length,
        routing,
        ..
    } = segments;
    let added_passes = u32::from(new_passes);
    if next_state.total_passes > u32::MAX - added_passes {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::ArithmeticOverflow,
        ));
    }
    next_state.total_passes =
        crate::refinement::packet_next_passes(next_state.total_passes, added_passes);
    if next_state.total_passes > crate::tier1::MAX_TIER1_PASSES {
        return Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        ));
    }
    let zero_bitplanes = match next_state.zero_bitplanes {
        Some(value) => value,
        None => {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidPacketHeader,
            ));
        }
    };
    next_state.high_throughput = routing;
    *state = next_state;
    Ok(Some(CodeBlockContribution {
        first_inclusion,
        zero_bitplanes,
        new_passes,
        data_length,
        high_throughput: routing != Some(false),
        segment_count: match u8::try_from(segment_count) {
            Ok(value) => value,
            Err(_) => {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        },
        segment_passes,
        segment_lengths,
    }))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_packet_state_view(final(state)), tag_tree_view(final(tree)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_header_zero_loop(code_block_packet_state_view(old(state)), tag_tree_view(old(tree)), context,
            packet_bit_reader_view(old(reader)), 1),
))]
fn read_header_zero_bitplanes(
    state: &mut CodeBlockPacketState,
    tree: &mut TagTree,
    context: CodeBlockPacketContext,
    reader: &mut PacketBitReader<'_>,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost original_state = code_block_packet_state_view(state);
        let ghost original_tree = tag_tree_view(tree);
        let ghost original_reader = packet_bit_reader_view(reader);
    }
    let maximum_zero_bitplanes = u32::from(context.maximum_zero_bitplanes);
    let mut threshold = 1_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant threshold >= 1,
            maximum_zero_bitplanes == context.maximum_zero_bitplanes as u32,
            original_state == code_block_packet_state_view(old(state)),
            original_tree == tag_tree_view(old(tree)), original_reader == packet_bit_reader_view(old(reader)),
            crate::specs::part1_header_zero_loop(original_state, original_tree, context, original_reader, 1)
                == crate::specs::part1_header_zero_loop(code_block_packet_state_view(state), tag_tree_view(tree), context,
                    packet_bit_reader_view(reader), threshold),
        decreases maximum_zero_bitplanes as int - threshold as int,
    ))]
    loop {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_header_zero_loop); }
        let decoded = tree.decode(context.leaf_x, context.leaf_y, threshold, reader)?;
        if decoded.is_below_threshold {
            let value = match decoded.value {
                Some(value) => value,
                None => {
                    return Err(packet_reader_error(
                        reader.bytes_consumed(),
                        MalformedKind::InvalidTagTree,
                    ));
                }
            };
            let zero_bitplanes = match u16::try_from(value) {
                Ok(value) => value,
                Err(_) => {
                    return Err(packet_reader_error(
                        reader.bytes_consumed(),
                        MalformedKind::InvalidPacketHeader,
                    ));
                }
            };
            if zero_bitplanes >= context.maximum_zero_bitplanes {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::InvalidPacketHeader,
                ));
            }
            state.zero_bitplanes = Some(zero_bitplanes);
            return Ok(());
        }
        if threshold >= maximum_zero_bitplanes {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidPacketHeader,
            ));
        }
        threshold = match threshold.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, packet_bit_reader_view(final(reader))) == crate::specs::part1_header_new_passes(packet_bit_reader_view(old(reader))),
))]
fn read_new_passes(reader: &mut PacketBitReader<'_>) -> Result<u8, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_header_new_passes); }
    if !reader.read_bit()? {
        return Ok(1);
    }
    if !reader.read_bit()? {
        return Ok(2);
    }
    let suffix = reader.read_bits(2)?;
    match suffix {
        0 => Ok(3),
        1 => Ok(4),
        2 => Ok(5),
        3 => {
            let extension = reader.read_bits(5)?;
            let passes = if extension == 31 {
                reader.read_bits(7)?.checked_add(37)
            } else {
                extension.checked_add(6)
            };
            let passes = match passes {
                Some(value) => value,
                None => {
                    return Err(packet_reader_error(
                        reader.bytes_consumed(),
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            match u8::try_from(passes) {
                Ok(value) => Ok(value),
                Err(_) => Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::ArithmeticOverflow,
                )),
            }
        }
        _ => Err(packet_reader_error(
            reader.bytes_consumed(),
            MalformedKind::InvalidPacketHeader,
        )),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_packet_state_view(final(state)), packet_bit_reader_view(final(reader)))
        == crate::specs::part1_header_lblock(code_block_packet_state_view(old(state)), packet_bit_reader_view(old(reader)), 32),
))]
fn read_header_lblock(
    state: &mut CodeBlockPacketState,
    reader: &mut PacketBitReader<'_>,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost original = code_block_packet_state_view(state);
        let ghost start_reader = packet_bit_reader_view(reader);
    }
    let mut remaining_lblock_increments = 32_u8;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant remaining_lblock_increments <= 32,
            original == code_block_packet_state_view(old(state)), start_reader == packet_bit_reader_view(old(reader)),
            crate::specs::part1_header_lblock(original, start_reader, 32)
                == crate::specs::part1_header_lblock(code_block_packet_state_view(state), packet_bit_reader_view(reader), remaining_lblock_increments as nat),
        decreases remaining_lblock_increments,
    ))]
    while reader.read_bit()? {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_header_lblock); }
        if remaining_lblock_increments == 0 {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidPacketHeader,
            ));
        }
        remaining_lblock_increments -= 1;
        if state.lblock >= 31 {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidPacketHeader,
            ));
        }
        state.lblock = crate::refinement::packet_next_lblock(state.lblock);
        if state.lblock > 31 {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidPacketHeader,
            ));
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_header_floor_log2(value),
))]
fn floor_log2(value: u32) -> u8 {
    #[cfg(verus_keep_ghost)]
    proof! { vstd::std_specs::bits::axiom_u32_leading_zeros(value); }
    if value != 0 {
        (31 - value.leading_zeros()) as u8
    } else {
        0
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct TagTreeNode {
    low: u32,
    value: u32,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct TagTreeLevel {
    offset: usize,
    width: u32,
    height: u32,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TagTree {
    levels: Vec<TagTreeLevel>,
    nodes: Vec<TagTreeNode>,
}

#[derive(Clone, Copy)]
struct TagTreePath {
    nodes: [usize; 64],
    length: usize,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn tag_node_view(node: TagTreeNode) -> crate::specs::Part1TagNode {
        crate::specs::Part1TagNode { low: node.low, value: node.value }
    }

    closed spec fn tag_level_view(level: TagTreeLevel) -> crate::specs::Part1TagLevel {
        crate::specs::Part1TagLevel { offset: level.offset, width: level.width, height: level.height }
    }

    pub closed spec fn tag_tree_view(tree: &TagTree) -> crate::specs::Part1TagTree {
        crate::specs::Part1TagTree {
            levels: tree.levels@.map_values(|level: TagTreeLevel| tag_level_view(level)),
            nodes: tree.nodes@.map_values(|node: TagTreeNode| tag_node_view(node)),
        }
    }

    pub open spec fn tag_tree_outcome(result: Result<TagTree, DecodeError>) -> Result<crate::specs::Part1TagTree, DecodeError> {
        match result { Err(error) => Err(error), Ok(tree) => Ok(tag_tree_view(&tree)) }
    }

    closed spec fn tag_path_view(path: TagTreePath) -> crate::specs::Part1TagPath {
        crate::specs::Part1TagPath { nodes: path.nodes@, length: path.length }
    }

    #[verifier::inline]
    spec fn tag_path_outcome(result: Result<TagTreePath, DecodeError>) -> Result<crate::specs::Part1TagPath, DecodeError> {
        match result { Err(error) => Err(error), Ok(path) => Ok(tag_path_view(path)) }
    }
}

impl Clone for TagTree {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures tag_tree_view(&result) == tag_tree_view(self),
    ))]
    fn clone(&self) -> Self {
        Self {
            levels: self.levels.clone(),
            nodes: self.nodes.clone(),
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TagTreeValue {
    pub value: Option<u32>,
    pub is_below_threshold: bool,
}

impl TagTree {
    pub fn new(width: u32, height: u32, limits: &ResourceLimits) -> Result<Self, DecodeError> {
        tag_tree_from_dimensions(width, height, limits.maximum_tag_tree_nodes())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_tag_leaf_count(tag_tree_view(self)),
    ))]
    pub fn leaf_count(&self) -> Result<u64, DecodeError> {
        let level = match self.levels.first() {
            Some(level) => level,
            None => return Err(packet_reader_error(0, MalformedKind::InvalidTagTree)),
        };
        match u64::from(level.width).checked_mul(u64::from(level.height)) {
            Some(count) => Ok(count),
            None => Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == tag_tree_view(self).nodes.len(),))]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures tag_path_outcome(result) == crate::specs::part1_tag_path(tag_tree_view(self), leaf_x, leaf_y, error_offset),
    ))]
    fn decode_path(
        &self,
        leaf_x: u32,
        leaf_y: u32,
        error_offset: usize,
    ) -> Result<TagTreePath, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_tag_path); }
        let leaf_level = match self.levels.first() {
            Some(level) => level,
            None => return Err(packet_reader_error(0, MalformedKind::InvalidTagTree)),
        };
        if leaf_x >= leaf_level.width || leaf_y >= leaf_level.height {
            return Err(packet_reader_error(
                error_offset,
                MalformedKind::InvalidTagTree,
            ));
        }

        let mut path = [0_usize; 64];
        let mut path_length = 0_usize;
        let mut x = leaf_x;
        let mut y = leaf_y;
        let mut level_index = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! { assert(path@ =~= Seq::new(64, |i: int| 0usize)); }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost goal = crate::specs::part1_tag_path(tag_tree_view(self), leaf_x, leaf_y, error_offset);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant level_index <= self.levels.len(),
                    path_length <= 64,
                    goal == crate::specs::part1_tag_path(tag_tree_view(self), leaf_x, leaf_y, error_offset),
                    goal == crate::specs::part1_tag_path_walk(tag_tree_view(self),
                        crate::specs::Part1TagPath { nodes: path@, length: path_length }, x, y, level_index as nat, error_offset),
                decreases self.levels.len() - level_index,
            )
        )]
        while level_index < self.levels.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_tag_path);
                reveal(crate::specs::part1_tag_path_walk);
            }
            if path_length >= path.len() {
                return Err(packet_reader_error(
                    error_offset,
                    MalformedKind::InvalidTagTree,
                ));
            }
            let level = self.levels[level_index];
            if x >= level.width || y >= level.height {
                return Err(packet_reader_error(
                    error_offset,
                    MalformedKind::InvalidTagTree,
                ));
            }
            let physical_y = match usize::try_from(y) {
                Ok(value) => value,
                Err(_) => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
            };
            let physical_width = match usize::try_from(level.width) {
                Ok(value) => value,
                Err(_) => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
            };
            let row = match physical_y.checked_mul(physical_width) {
                Some(value) => value,
                None => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
            };
            let physical_x = match usize::try_from(x) {
                Ok(value) => value,
                Err(_) => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
            };
            let relative = match row.checked_add(physical_x) {
                Some(value) => value,
                None => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
            };
            let node_index = match level.offset.checked_add(relative) {
                Some(value) => value,
                None => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
            };
            if node_index >= self.nodes.len() {
                return Err(packet_reader_error(
                    error_offset,
                    MalformedKind::InvalidTagTree,
                ));
            }
            path[path_length] = node_index;
            path_length = path_length
                .checked_add(1)
                .ok_or_else(|| packet_reader_error(0, MalformedKind::ArithmeticOverflow))?;
            x /= 2;
            y /= 2;
            level_index = level_index
                .checked_add(1)
                .ok_or_else(|| packet_reader_error(0, MalformedKind::ArithmeticOverflow))?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_tag_path_walk); }
        Ok(TagTreePath {
            nodes: path,
            length: path_length,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tag_tree_view(final(self)), packet_bit_reader_view(final(reader)))
            == crate::specs::part1_tag_reverse(tag_tree_view(old(self)), tag_path_view(path), path.length as nat,
                0, threshold, packet_bit_reader_view(old(reader))),
    ))]
    fn decode_path_nodes(
        &mut self,
        path: TagTreePath,
        threshold: u32,
        reader: &mut PacketBitReader<'_>,
    ) -> Result<TagTreeValue, DecodeError> {
        let mut inherited_low = 0_u32;
        let mut reverse_index = path.length;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost initial_tree = tag_tree_view(self);
            let ghost initial_reader = packet_bit_reader_view(reader);
            let ghost goal = crate::specs::part1_tag_reverse(initial_tree, tag_path_view(path), path.length as nat, 0, threshold, initial_reader);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant reverse_index <= path.length,
                    initial_tree == tag_tree_view(old(self)), initial_reader == packet_bit_reader_view(old(reader)),
                    goal == crate::specs::part1_tag_reverse(initial_tree, tag_path_view(path), path.length as nat, 0, threshold, initial_reader),
                    goal == crate::specs::part1_tag_reverse(tag_tree_view(self), tag_path_view(path), reverse_index as nat,
                        inherited_low, threshold, packet_bit_reader_view(reader)),
                decreases reverse_index,
            )
        )]
        while reverse_index > 0 {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_tag_reverse); }
            reverse_index = reverse_index
                .checked_sub(1)
                .ok_or_else(|| packet_reader_error(0, MalformedKind::ArithmeticOverflow))?;
            if reverse_index >= path.nodes.len() {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::InvalidTagTree,
                ));
            }
            let node_index = path.nodes[reverse_index];
            if node_index >= self.nodes.len() {
                return Err(packet_reader_error(
                    reader.bytes_consumed(),
                    MalformedKind::InvalidTagTree,
                ));
            }
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = tag_tree_view(self); }
            let decoded = decode_tag_tree_node(
                &mut self.nodes[node_index],
                inherited_low,
                threshold,
                reader,
            );
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(tag_tree_view(self).nodes =~= before.nodes.update(node_index as int, tag_node_view(self.nodes[node_index as int])));
            }
            match decoded? {
                None => {
                    return Ok(TagTreeValue {
                        value: None,
                        is_below_threshold: false,
                    });
                }
                Some(value) => inherited_low = value,
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_tag_reverse); }
        let leaf_index = path.nodes[0];
        if leaf_index >= self.nodes.len() {
            return Err(packet_reader_error(
                reader.bytes_consumed(),
                MalformedKind::InvalidTagTree,
            ));
        }
        let value = self.nodes[leaf_index].value;
        Ok(TagTreeValue {
            value: if value == u32::MAX { None } else { Some(value) },
            is_below_threshold: value < threshold,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tag_tree_view(final(self)), packet_bit_reader_view(final(reader)))
            == crate::specs::part1_tag_tree_decode(tag_tree_view(old(self)), leaf_x, leaf_y, threshold, packet_bit_reader_view(old(reader))),
    ))]
    pub fn decode(
        &mut self,
        leaf_x: u32,
        leaf_y: u32,
        threshold: u32,
        reader: &mut PacketBitReader<'_>,
    ) -> Result<TagTreeValue, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_tag_tree_decode); }
        let path = self.decode_path(leaf_x, leaf_y, reader.bytes_consumed())?;
        self.decode_path_nodes(path, threshold, reader)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, tag_node_view(*final(node)), packet_bit_reader_view(final(reader))) ==
        crate::specs::part1_tag_node_decode(tag_node_view(*old(node)), inherited, threshold, packet_bit_reader_view(old(reader))),
))]
fn decode_tag_tree_node(
    node: &mut TagTreeNode,
    inherited: u32,
    threshold: u32,
    reader: &mut PacketBitReader<'_>,
) -> Result<Option<u32>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part1_tag_node_decode(tag_node_view(*node), inherited, threshold, packet_bit_reader_view(reader)); }
    node.low = crate::refinement::tag_inherited_low(node.low, inherited);
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant goal == crate::specs::part1_tag_node_decode(tag_node_view(*old(node)), inherited, threshold, packet_bit_reader_view(old(reader))),
            goal == crate::specs::part1_tag_node_loop(tag_node_view(*node), threshold, packet_bit_reader_view(reader)),
        decreases threshold as int - node.low as int, node.value as int - node.low as int,
    ))]
    while node.low < threshold && node.low < node.value {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_tag_node_loop); }
        let bit = reader.read_bit()?;
        let next_value = crate::refinement::tag_step_value(node.low, node.value, bit);
        let next_low = if bit {
            node.low
        } else {
            crate::refinement::tag_step_low(node.low, false)
        };
        node.value = next_value;
        node.low = next_low;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_tag_node_loop); }
    Ok(if node.value >= threshold {
        None
    } else {
        Some(node.value)
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures tag_tree_outcome(result) == crate::specs::part1_tag_initial(width, height, maximum_nodes),
))]
pub(crate) fn tag_tree_from_dimensions(
    width: u32,
    height: u32,
    maximum_nodes: u64,
) -> Result<TagTree, DecodeError> {
    if width == 0 || height == 0 {
        return Err(packet_reader_error(0, MalformedKind::InvalidTagTree));
    }
    let mut levels = Vec::new();
    let mut total_nodes = 0_usize;
    let mut level_width = width;
    let mut level_height = height;
    let mut remaining_levels = 64_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(levels@.map_values(|level: TagTreeLevel| tag_level_view(level)) =~= Seq::<crate::specs::Part1TagLevel>::empty());
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant remaining_levels <= 64, width != 0, height != 0,
        invariant_except_break
            crate::specs::part1_tag_build(width, height, crate::specs::Part1TagBuild { levels: Seq::empty(), total: 0 }, 64, maximum_nodes)
                == crate::specs::part1_tag_build(level_width, level_height,
                    crate::specs::Part1TagBuild { levels: levels@.map_values(|level: TagTreeLevel| tag_level_view(level)), total: total_nodes },
                    remaining_levels as nat, maximum_nodes),
        ensures crate::specs::part1_tag_build(width, height, crate::specs::Part1TagBuild { levels: Seq::empty(), total: 0 }, 64, maximum_nodes)
            == Ok(crate::specs::Part1TagBuild {
                levels: levels@.map_values(|level: TagTreeLevel| tag_level_view(level)), total: total_nodes
            }),
        decreases remaining_levels,
    ))]
    loop {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_tag_build); }
        if remaining_levels == 0 {
            return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow));
        }
        remaining_levels -= 1;
        let level_nodes = match u64::from(level_width).checked_mul(u64::from(level_height)) {
            Some(count) => count,
            None => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
        };
        let next_total = match (total_nodes as u64).checked_add(level_nodes) {
            Some(count) => count,
            None => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
        };
        if next_total > maximum_nodes {
            return Err(packet_reader_limit(
                Resource::TagTreeNodes,
                maximum_nodes,
                next_total,
            ));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost previous_levels = levels@.map_values(|level: TagTreeLevel| tag_level_view(level)); }
        levels.push(TagTreeLevel {
            offset: total_nodes,
            width: level_width,
            height: level_height,
        });
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(levels@.map_values(|level: TagTreeLevel| tag_level_view(level)) =~= previous_levels.push(
                crate::specs::Part1TagLevel { offset: total_nodes, width: level_width, height: level_height }));
        }
        total_nodes = match usize::try_from(next_total) {
            Ok(total) => total,
            Err(_) => return Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow)),
        };
        if level_width == 1 && level_height == 1 {
            break;
        }
        level_width = ceil_div_two(level_width);
        level_height = ceil_div_two(level_height);
    }
    let nodes = vec![
        TagTreeNode {
            low: 0,
            value: u32::MAX
        };
        total_nodes
    ];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(nodes@.map_values(|node: TagTreeNode| tag_node_view(node)) =~=
            Seq::new(total_nodes as nat, |i: int| crate::specs::Part1TagNode { low: 0, value: u32::MAX }));
    }
    Ok(TagTree { levels, nodes })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == (value as nat / 2 + value as nat % 2) as u32,
))]
const fn ceil_div_two(value: u32) -> u32 {
    value / 2 + value % 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn literal_one_node_tree(low: u32, value: u32) -> TagTree {
        TagTree {
            levels: vec![TagTreeLevel {
                offset: 0,
                width: 1,
                height: 1,
            }],
            nodes: vec![TagTreeNode { low, value }],
        }
    }

    fn literal_header_context(style: Tier1CodingStyle) -> CodeBlockPacketContext {
        CodeBlockPacketContext {
            leaf_x: 0,
            leaf_y: 0,
            layer: 0,
            maximum_zero_bitplanes: 2,
            tier1_style: style,
        }
    }

    #[test]
    fn actual_full_header_literal_and_every_work_prefix_have_independent_whole_results() {
        for cap in 0_u64..=8 {
            let mut state = code_block_packet_state_new();
            let original = state;
            let mut inclusion = literal_one_node_tree(0, u32::MAX);
            let mut zero = literal_one_node_tree(0, u32::MAX);
            let mut reader = packet_reader_prefix_from_input(&[0xc2], 1, cap);
            let result = decode_code_block_contribution(
                &mut state,
                &mut inclusion,
                &mut zero,
                literal_header_context(Tier1CodingStyle::default()),
                &mut reader,
            );
            let consumed = if cap < 7 { cap + 1 } else { 7 };
            assert_eq!(
                reader,
                PacketBitReader {
                    input: &[0xc2],
                    byte_index: 1,
                    current_byte: 0xc2,
                    bits_remaining: (8 - consumed) as u8,
                    previous_byte_was_ff: false,
                    bits_read: consumed,
                    maximum_work_units: cap,
                    maximum_header_bytes: 1,
                }
            );
            assert_eq!(
                inclusion,
                literal_one_node_tree(0, if cap >= 1 { 0 } else { u32::MAX })
            );
            assert_eq!(
                zero,
                literal_one_node_tree(0, if cap >= 2 { 0 } else { u32::MAX })
            );
            if cap < 7 {
                assert_eq!(state, original);
                assert_eq!(
                    result,
                    Err(packet_reader_limit(Resource::WorkUnits, cap, cap + 1))
                );
            } else {
                assert_eq!(
                    state,
                    CodeBlockPacketState {
                        included: true,
                        zero_bitplanes: Some(0),
                        lblock: 3,
                        total_passes: 1,
                        high_throughput: Some(false),
                    }
                );
                let mut passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
                let mut lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
                passes[0] = 1;
                lengths[0] = 1;
                assert_eq!(
                    result,
                    Ok(Some(CodeBlockContribution {
                        first_inclusion: true,
                        zero_bitplanes: 0,
                        new_passes: 1,
                        data_length: 1,
                        high_throughput: false,
                        segment_count: 1,
                        segment_passes: passes,
                        segment_lengths: lengths,
                    }))
                );
            }
        }
    }

    #[test]
    fn full_header_admits_every_stored_lblock_byte_and_does_not_validate_unvisited_zero_tree() {
        for lblock in 0..=u8::MAX {
            let mut state = CodeBlockPacketState {
                included: true,
                zero_bitplanes: Some(u16::MAX),
                lblock,
                total_passes: 1,
                high_throughput: Some(false),
            };
            let original = state;
            let mut inclusion = TagTree {
                levels: vec![],
                nodes: vec![],
            };
            let mut zero = TagTree {
                levels: vec![],
                nodes: vec![],
            };
            let input = [0x80, 0, 0, 0, 0];
            let mut reader = packet_reader_prefix_from_input(&input, 5, 64);
            let mut context = literal_header_context(Tier1CodingStyle::default());
            context.maximum_zero_bitplanes = 1;
            let result = decode_code_block_contribution(
                &mut state,
                &mut inclusion,
                &mut zero,
                context,
                &mut reader,
            );
            let consumed = if lblock <= 32 {
                3 + u64::from(lblock)
            } else {
                3
            };
            let index = consumed.div_ceil(8) as usize;
            assert_eq!(
                reader,
                PacketBitReader {
                    input: &input,
                    byte_index: index,
                    current_byte: if index == 1 { 0x80 } else { 0 },
                    bits_remaining: ((8 - consumed % 8) % 8) as u8,
                    previous_byte_was_ff: false,
                    bits_read: consumed,
                    maximum_work_units: 64,
                    maximum_header_bytes: 5,
                }
            );
            assert!(
                inclusion.levels.is_empty()
                    && inclusion.nodes.is_empty()
                    && zero.levels.is_empty()
                    && zero.nodes.is_empty()
            );
            if lblock > 32 {
                assert_eq!(
                    result,
                    Err(packet_reader_error(1, MalformedKind::InvalidPacketHeader))
                );
                assert_eq!(state, original);
            } else {
                let mut passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
                passes[0] = 1;
                assert_eq!(
                    result,
                    Ok(Some(CodeBlockContribution {
                        first_inclusion: false,
                        zero_bitplanes: u16::MAX,
                        new_passes: 1,
                        data_length: 0,
                        high_throughput: false,
                        segment_count: 1,
                        segment_passes: passes,
                        segment_lengths: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
                    }))
                );
                assert_eq!(
                    state,
                    CodeBlockPacketState {
                        total_passes: 2,
                        ..original
                    }
                );
            }
        }
    }

    #[test]
    fn header_none_empty_unused_raw_trees_and_late_error_rollback_are_distinct() {
        let mut state = code_block_packet_state_new();
        let original = state;
        let mut inclusion = literal_one_node_tree(1, 1);
        let mut zero = TagTree {
            levels: vec![],
            nodes: vec![],
        };
        let mut reader = packet_reader_prefix_from_input(&[], 0, 0);
        reader.bits_remaining = u8::MAX;
        let before = reader;
        assert_eq!(
            decode_code_block_contribution(
                &mut state,
                &mut inclusion,
                &mut zero,
                literal_header_context(Tier1CodingStyle::default()),
                &mut reader
            ),
            Ok(None)
        );
        assert_eq!(state, original);
        assert_eq!(reader, before);
        assert_eq!(inclusion, literal_one_node_tree(1, 1));
        assert_eq!(
            zero,
            TagTree {
                levels: vec![],
                nodes: vec![]
            }
        );

        let mut state = CodeBlockPacketState {
            included: true,
            zero_bitplanes: Some(0),
            lblock: 0,
            total_passes: 1,
            high_throughput: Some(false),
        };
        let mut inclusion = TagTree {
            levels: vec![],
            nodes: vec![],
        };
        let mut reader = packet_reader_prefix_from_input(&[0x80], 1, 3);
        let result = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero,
            literal_header_context(Tier1CodingStyle::default()),
            &mut reader,
        )
        .unwrap()
        .unwrap();
        let mut passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        passes[0] = 1;
        assert_eq!(
            result,
            CodeBlockContribution {
                first_inclusion: false,
                zero_bitplanes: 0,
                new_passes: 1,
                data_length: 0,
                high_throughput: false,
                segment_count: 1,
                segment_passes: passes,
                segment_lengths: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
            }
        );
        assert_eq!(state.total_passes, 2);
        assert_eq!(reader.bits_read, 3);
        assert!(inclusion.levels.is_empty() && zero.levels.is_empty());
        for total in [crate::tier1::MAX_TIER1_PASSES, u32::MAX] {
            let mut state = CodeBlockPacketState {
                included: true,
                zero_bitplanes: Some(0),
                lblock: 3,
                total_passes: total,
                high_throughput: Some(false),
            };
            let before = state;
            let mut reader = packet_reader_prefix_from_input(&[0x84], 1, 64);
            assert_eq!(
                decode_code_block_contribution(
                    &mut state,
                    &mut inclusion,
                    &mut zero,
                    literal_header_context(Tier1CodingStyle::default()),
                    &mut reader
                ),
                Err(packet_reader_error(
                    1,
                    if total == u32::MAX {
                        MalformedKind::ArithmeticOverflow
                    } else {
                        MalformedKind::InvalidPacketHeader
                    }
                ))
            );
            assert_eq!(state, before);
            assert_eq!(
                (reader.bits_read, reader.byte_index, reader.bits_remaining),
                (6, 1, 2)
            );
        }
    }

    #[test]
    fn actual_mixed_header_probe_commits_only_selected_reader_and_preserves_failed_main_cursor() {
        for (lblock, input, primary_bits, expected_passes, expected_length, route, consumed) in [
            (3_u8, 0x38_u8, 3_u8, 4_u32, 7_u32, Some(false), 5_u64),
            (4, 0x1c, 4, 1, 1, Some(true), 4),
            (3, 0, 3, 1, 0, None, 3),
        ] {
            let mut state = CodeBlockPacketState {
                included: true,
                zero_bitplanes: Some(0),
                lblock,
                total_passes: 0,
                high_throughput: None,
            };
            let mut segments = header_segments_new(
                &state,
                Tier1CodingStyle::from_code_block_style(0xc0).unwrap(),
            );
            let data = [input];
            let mut reader = packet_reader_prefix_from_input(&data, 1, 64);
            assert_eq!(
                read_header_segment_length(
                    &mut state,
                    &mut segments,
                    &mut reader,
                    0,
                    4,
                    1,
                    primary_bits
                ),
                Ok((expected_passes, expected_length))
            );
            assert_eq!(state.high_throughput, route);
            assert_eq!(segments.routing, route);
            assert_eq!(
                reader,
                PacketBitReader {
                    input: &[input],
                    byte_index: 1,
                    current_byte: input,
                    bits_remaining: (8 - consumed) as u8,
                    previous_byte_was_ff: false,
                    bits_read: consumed,
                    maximum_work_units: 64,
                    maximum_header_bytes: 1,
                }
            );
        }
        let mut state = CodeBlockPacketState {
            included: true,
            zero_bitplanes: Some(0),
            lblock: 4,
            total_passes: 0,
            high_throughput: None,
        };
        let before = state;
        let mut segments = header_segments_new(
            &state,
            Tier1CodingStyle::from_code_block_style(0xc0).unwrap(),
        );
        let mut reader = packet_reader_prefix_from_input(&[0x1c], 1, 4);
        assert_eq!(
            read_header_segment_length(&mut state, &mut segments, &mut reader, 0, 4, 1, 4),
            Err(packet_reader_limit(Resource::WorkUnits, 4, 5))
        );
        assert_eq!(state, before);
        assert_eq!(segments.routing, None);
        assert_eq!(
            reader,
            PacketBitReader {
                input: &[0x1c],
                byte_index: 1,
                current_byte: 0x1c,
                bits_remaining: 4,
                previous_byte_was_ff: false,
                bits_read: 4,
                maximum_work_units: 4,
                maximum_header_bytes: 1,
            }
        );
        let mut reader = packet_reader_prefix_from_input(&[0x10], 2, 64);
        assert_eq!(
            read_header_segment_length(&mut state, &mut segments, &mut reader, 0, 164, 1, 4),
            Err(packet_reader_error(1, MalformedKind::Truncated))
        );
        assert_eq!(reader.bits_read, 4);
        assert_eq!(reader.bits_remaining, 4);
        assert_eq!(state, before);
    }

    #[test]
    fn native_header_segment_capacity_and_final_count_conversion_keep_exact_order() {
        let mut state = CodeBlockPacketState {
            included: true,
            zero_bitplanes: Some(0),
            lblock: 3,
            total_passes: 0,
            high_throughput: Some(false),
        };
        let original = state;
        for count in [164, 165, usize::MAX] {
            let mut segments = header_segments_new(&state, Tier1CodingStyle::default());
            segments.segment_count = count;
            state.total_passes = u32::MAX;
            let before = state;
            let mut reader = packet_reader_prefix_from_input(&[], 0, 0);
            reader.byte_index = 17;
            let old_reader = reader;
            assert_eq!(
                read_header_segment(
                    &mut state,
                    &mut segments,
                    u8::MAX,
                    literal_header_context(Tier1CodingStyle::default()),
                    &mut reader
                ),
                Err(packet_reader_error(17, MalformedKind::InvalidPacketHeader))
            );
            assert_eq!(state, before);
            assert_eq!(reader, old_reader);
            assert_eq!(segments.segment_count, count);
        }
        for count in [164, 165, 256] {
            state = original;
            let mut segments = header_segments_new(&state, Tier1CodingStyle::default());
            segments.segment_count = count;
            let next = state;
            let mut reader = packet_reader_prefix_from_input(&[], 0, 0);
            reader.byte_index = 17;
            let result = commit_header_contribution(&mut state, next, segments, false, 1, &reader);
            assert_eq!(
                state,
                CodeBlockPacketState {
                    total_passes: 1,
                    ..original
                }
            );
            if count <= 255 {
                assert_eq!(result.unwrap().unwrap().segment_count(), count as u8);
            } else {
                assert_eq!(
                    result,
                    Err(packet_reader_error(17, MalformedKind::ArithmeticOverflow))
                );
            }
        }
    }

    #[test]
    fn packet_reader_words_match_closed_two_byte_oracle_and_all_eight_final_fields() {
        for first in 0..=u8::MAX {
            for second in 0..=u8::MAX {
                let input = [first, second];
                for count in [0_u8, 1, 7, 8, 9, 15, 16, 32] {
                    let mut reader = packet_reader_prefix_from_input(&input, 32, 64);
                    let result = reader.read_bits(count);
                    let stuffed = first == 0xff;
                    let invalid_stuff = stuffed && second >= 0x80 && count > 8;
                    let available = if stuffed { 15_u8 } else { 16 };
                    let read = if invalid_stuff {
                        8
                    } else {
                        count.min(available)
                    };
                    let expected_result = if invalid_stuff {
                        Err(packet_reader_error(1, MalformedKind::InvalidBitStuffing))
                    } else if count > available {
                        Err(packet_reader_error(2, MalformedKind::Truncated))
                    } else if count == 0 {
                        Ok(0)
                    } else if count <= 8 {
                        Ok(u32::from(first) >> (8 - count))
                    } else if stuffed {
                        Ok((255_u32 << (count - 8)) | (u32::from(second) >> (15 - count)))
                    } else {
                        Ok(((u32::from(first) * 256) + u32::from(second)) >> (16 - count))
                    };
                    let loaded_second = read > 8;
                    assert_eq!(result, expected_result, "input={input:?}, count={count}");
                    assert_eq!(
                        reader,
                        PacketBitReader {
                            input: &input,
                            byte_index: if read == 0 {
                                0
                            } else if loaded_second {
                                2
                            } else {
                                1
                            },
                            current_byte: if read == 0 {
                                0
                            } else if loaded_second {
                                second
                            } else {
                                first
                            },
                            bits_remaining: if loaded_second {
                                available - read
                            } else if read == 0 {
                                0
                            } else {
                                8 - read
                            },
                            previous_byte_was_ff: read != 0
                                && if loaded_second {
                                    second == 0xff
                                } else {
                                    stuffed
                                },
                            bits_read: u64::from(read),
                            maximum_work_units: 64,
                            maximum_header_bytes: 32,
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn packet_reader_native_state_errors_preserve_exact_admission_and_consumption_order() {
        let mut reader = packet_reader_prefix_from_input(&[], 0, 0);
        let before = reader;
        assert_eq!(
            reader.read_bit(),
            Err(packet_reader_limit(Resource::PacketHeaderBytes, 0, 1))
        );
        assert_eq!(reader, before);
        reader.byte_index = usize::MAX;
        reader.maximum_header_bytes = u64::MAX;
        let before = reader;
        assert_eq!(
            reader.read_bit(),
            Err(packet_reader_error(
                usize::MAX,
                if usize::BITS == 64 {
                    MalformedKind::ArithmeticOverflow
                } else {
                    MalformedKind::Truncated
                }
            ))
        );
        assert_eq!(reader, before);
    }

    #[test]
    fn raw_tag_tree_single_node_matches_wide_threshold_and_budget_oracle() {
        for low in [0, 1, 3, u32::MAX - 1, u32::MAX] {
            for value in [0, 1, 4, u32::MAX - 1, u32::MAX] {
                for threshold in [0, 1, 2, 4, u32::MAX] {
                    for cap in [0_u64, 1, 2, 8] {
                        let mut tree = TagTree {
                            levels: vec![TagTreeLevel {
                                offset: 0,
                                width: 1,
                                height: 1,
                            }],
                            nodes: vec![TagTreeNode { low, value }],
                        };
                        let before = tree.clone();
                        let mut reader = packet_reader_prefix_from_input(&[0], 1, cap);
                        let needed = u64::from(threshold.min(value)).saturating_sub(u64::from(low));
                        let successful = needed.min(cap).min(8);
                        let mut expected_tree = before;
                        expected_tree.nodes[0].low = (u64::from(low) + successful) as u32;
                        let actual = tree.decode(0, 0, threshold, &mut reader);
                        let consumed = if needed > cap && cap < 8 {
                            cap + 1
                        } else {
                            successful
                        };
                        let expected = if needed > cap && cap < 8 {
                            Err(packet_reader_limit(Resource::WorkUnits, cap, cap + 1))
                        } else if needed > 8 {
                            Err(packet_reader_limit(Resource::PacketHeaderBytes, 1, 2))
                        } else {
                            Ok(TagTreeValue {
                                value: if value < threshold { Some(value) } else { None },
                                is_below_threshold: value < threshold,
                            })
                        };
                        assert_eq!(actual, expected);
                        assert_eq!(tree, expected_tree);
                        assert_eq!(
                            reader,
                            PacketBitReader {
                                input: &[0],
                                byte_index: usize::from(consumed != 0),
                                current_byte: 0,
                                bits_remaining: if consumed == 0 {
                                    0
                                } else {
                                    (8 - consumed) as u8
                                },
                                previous_byte_was_ff: false,
                                bits_read: consumed,
                                maximum_work_units: cap,
                                maximum_header_bytes: 1,
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn raw_tag_tree_duplicate_paths_and_64_65_levels_keep_real_state_and_error_order() {
        for levels in [2, 64, 65] {
            let mut tree = TagTree {
                levels: vec![
                    TagTreeLevel {
                        offset: 0,
                        width: 1,
                        height: 1
                    };
                    levels
                ],
                nodes: vec![TagTreeNode {
                    low: 0,
                    value: u32::MAX,
                }],
            };
            let before = tree.clone();
            let mut reader = packet_reader_prefix_from_input(&[0x80], 1, 1);
            let original_reader = reader;
            let result = tree.decode(0, 0, 1, &mut reader);
            if levels == 65 {
                assert_eq!(
                    result,
                    Err(packet_reader_error(0, MalformedKind::InvalidTagTree))
                );
                assert_eq!(tree, before);
                assert_eq!(reader, original_reader);
            } else {
                assert_eq!(
                    result,
                    Ok(TagTreeValue {
                        value: Some(0),
                        is_below_threshold: true
                    })
                );
                assert_eq!(tree.levels, before.levels);
                assert_eq!(tree.nodes, [TagTreeNode { low: 0, value: 0 }]);
                assert_eq!(
                    (reader.bits_read, reader.byte_index, reader.bits_remaining),
                    (1, 1, 7)
                );
            }
        }
        for offset in [0, usize::MAX] {
            let mut tree = TagTree {
                levels: vec![TagTreeLevel {
                    offset,
                    width: 2,
                    height: 2,
                }],
                nodes: vec![],
            };
            let before = tree.clone();
            let mut reader = packet_reader_prefix_from_input(&[], u64::MAX, u64::MAX);
            reader.byte_index = 17;
            let original = reader;
            assert_eq!(
                tree.decode(1, 1, u32::MAX, &mut reader),
                Err(if offset == 0 {
                    packet_reader_error(17, MalformedKind::InvalidTagTree)
                } else {
                    packet_reader_error(0, MalformedKind::ArithmeticOverflow)
                })
            );
            assert_eq!(tree, before);
            assert_eq!(reader, original);
        }
        let mut empty = TagTree {
            levels: vec![],
            nodes: vec![TagTreeNode { low: 7, value: 0 }],
        };
        let mut reader = packet_reader_prefix_from_input(&[], 0, 0);
        reader.byte_index = 17;
        assert_eq!(
            empty.leaf_count(),
            Err(packet_reader_error(0, MalformedKind::InvalidTagTree))
        );
        assert_eq!(
            empty.decode(0, 0, 1, &mut reader),
            Err(packet_reader_error(0, MalformedKind::InvalidTagTree))
        );
        assert_eq!(empty.nodes[0], TagTreeNode { low: 7, value: 0 });
    }

    #[test]
    fn tag_tree_real_constructor_matches_closed_rectangle_levels_and_cap_first_failure() {
        for width in 1_u32..=9 {
            for height in 1_u32..=9 {
                let top = 32 - (width.max(height) - 1).leading_zeros();
                for cap in [0_u64, 1, 8, 64, 200] {
                    let mut levels = Vec::new();
                    let mut total = 0_u64;
                    let mut failure = None;
                    for level in 0..=top {
                        let scale = 1_u64 << level;
                        let w = u64::from(width).div_ceil(scale);
                        let h = u64::from(height).div_ceil(scale);
                        let next = total + w * h;
                        if next > cap {
                            failure = Some(packet_reader_limit(Resource::TagTreeNodes, cap, next));
                            break;
                        }
                        levels.push(TagTreeLevel {
                            offset: total as usize,
                            width: w as u32,
                            height: h as u32,
                        });
                        total = next;
                    }
                    assert_eq!(
                        tag_tree_from_dimensions(width, height, cap),
                        match failure {
                            Some(error) => Err(error),
                            None => Ok(TagTree {
                                levels,
                                nodes: vec![
                                    TagTreeNode {
                                        low: 0,
                                        value: u32::MAX
                                    };
                                    total as usize
                                ]
                            }),
                        }
                    );
                }
            }
        }
        assert_eq!(
            tag_tree_from_dimensions(0, u32::MAX, 0),
            Err(packet_reader_error(0, MalformedKind::InvalidTagTree))
        );
        assert_eq!(
            tag_tree_from_dimensions(u32::MAX, u32::MAX, u64::MAX),
            Err(packet_reader_error(0, MalformedKind::ArithmeticOverflow))
        );
    }

    #[test]
    fn packet_state_getters_and_copy_preserve_all_native_fields_without_header_invariants() {
        assert_eq!(
            code_block_packet_state_new(),
            CodeBlockPacketState {
                included: false,
                zero_bitplanes: None,
                lblock: 3,
                total_passes: 0,
                high_throughput: None,
            }
        );
        for lblock in 0..=u8::MAX {
            for included in [false, true] {
                for zero_bitplanes in [None, Some(0), Some(u16::MAX)] {
                    for total_passes in [0, crate::tier1::MAX_TIER1_PASSES, u32::MAX] {
                        for high_throughput in [None, Some(false), Some(true)] {
                            let state = CodeBlockPacketState {
                                included,
                                zero_bitplanes,
                                lblock,
                                total_passes,
                                high_throughput,
                            };
                            let copy = state;
                            assert_eq!(
                                (
                                    copy.included(),
                                    copy.zero_bitplanes(),
                                    copy.lblock(),
                                    copy.total_passes(),
                                    copy.high_throughput()
                                ),
                                (
                                    included,
                                    zero_bitplanes,
                                    lblock,
                                    total_passes,
                                    high_throughput
                                )
                            );
                            assert_eq!(copy, state);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn packet_reader_budget_stuffing_and_full_width_words() {
        let mut reader = packet_reader_prefix_from_input(&[0x80], 1, 0);
        assert_eq!(
            reader.read_bit(),
            Err(packet_reader_limit(Resource::WorkUnits, 0, 1))
        );
        assert_eq!(
            reader,
            PacketBitReader {
                input: &[0x80],
                byte_index: 1,
                current_byte: 0x80,
                bits_remaining: 7,
                previous_byte_was_ff: false,
                bits_read: 1,
                maximum_work_units: 0,
                maximum_header_bytes: 1,
            }
        );
        reader.bits_read = u64::MAX;
        let mut expected = reader;
        expected.bits_remaining = 6;
        assert_eq!(
            reader.read_bit(),
            Err(packet_reader_error(1, MalformedKind::ArithmeticOverflow))
        );
        assert_eq!(reader, expected);
        reader.bits_remaining = u8::MAX;
        let before = reader;
        assert_eq!(reader.read_bits(0), Ok(0));
        assert_eq!(reader, before);
        for count in 33..=u8::MAX {
            assert_eq!(
                reader.read_bits(count),
                Err(packet_reader_error(1, MalformedKind::InvalidMarkerLength))
            );
            assert_eq!(reader, before);
        }
        assert_eq!(
            reader.read_bit(),
            Err(packet_reader_error(1, MalformedKind::InvalidBitStuffing))
        );
        assert_eq!(reader, before);
        assert_eq!(
            packet_reader_from_input(&[0, 0], 1, 0),
            Err(packet_reader_limit(Resource::PacketHeaderBytes, 1, 2))
        );
        let mut reader = packet_reader_prefix_from_input(&[0xff, 0x80], 2, 64);
        assert_eq!(reader.read_bit(), Ok(true));
        reader.align_to_byte();
        assert!(reader.previous_byte_was_ff);
        let before = reader;
        assert_eq!(
            reader.read_bit(),
            Err(packet_reader_error(1, MalformedKind::InvalidBitStuffing))
        );
        assert_eq!(reader, before);
        let mut reader = packet_reader_prefix_from_input(&[0xff, 0x7f, 0xff, 0x7f, 0xff], 5, 32);
        assert_eq!(reader.read_bits(32), Ok(u32::MAX));
        assert_eq!(
            (
                reader.byte_index,
                reader.bits_remaining,
                reader.bits_read,
                reader.current_byte,
                reader.previous_byte_was_ff
            ),
            (5, 6, 32, 0xff, true)
        );
    }

    fn literal_empty_ht_contribution(count: u8, passes: u8) -> CodeBlockContribution {
        CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: passes,
            data_length: 0,
            high_throughput: true,
            segment_count: count,
            segment_passes: [1; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
            segment_lengths: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
        }
    }

    fn literal_ht_decoder(bitplanes: u16) -> Tier1CodeBlockDecoder {
        Tier1CodeBlockDecoder::new(
            1,
            1,
            bitplanes,
            SubbandOrientation::Ll,
            Tier1CodingStyle::from_code_block_style(0x40).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn ht_body_zero_setting_precedes_count_error_but_outer_rolls_back_every_field() {
        let limits = ResourceLimits::default();
        let mut header = literal_empty_ht_contribution(0, 0);
        header.zero_bitplanes = 1;
        let mut staged = literal_ht_decoder(3);
        assert_eq!(staged.maximum_passes(), 7);
        assert_eq!(
            decode_code_block_contribution_body_staged(&mut staged, &header, &[], &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodeBlock
            }),
        );
        assert_eq!(staged.maximum_passes(), 4);
        assert_eq!(
            (
                staged.next_pass(),
                staged.decoded_segments(),
                staged.decoded_bytes()
            ),
            (0, 0, 0)
        );
        let mut transactional = literal_ht_decoder(3);
        let before = format!("{transactional:?}");
        assert_eq!(
            decode_code_block_contribution_body(&mut transactional, &header, &[], &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodeBlock
            }),
        );
        assert_eq!(format!("{transactional:?}"), before);
    }

    #[test]
    fn ht_body_final_pass_mismatch_keeps_staged_progress_and_rolls_back_outer_decoder() {
        let limits = ResourceLimits::default();
        let mut header = literal_empty_ht_contribution(2, 2);
        header.segment_passes[1] = 2;
        let mut staged = literal_ht_decoder(3);
        assert_eq!(
            decode_code_block_contribution_body_staged(&mut staged, &header, &[], &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodeBlock
            }),
        );
        assert_eq!(
            (
                staged.next_pass(),
                staged.decoded_segments(),
                staged.decoded_bytes()
            ),
            (3, 2, 0)
        );
        let mut transactional = literal_ht_decoder(3);
        let before = format!("{transactional:?}");
        assert!(
            decode_code_block_contribution_body(&mut transactional, &header, &[], &limits).is_err()
        );
        assert_eq!(format!("{transactional:?}"), before);
        header.new_passes = 3;
        assert_eq!(
            decode_code_block_contribution_body(&mut transactional, &header, &[], &limits),
            Ok(CodeBlockBodyDecode {
                segments: 2,
                passes: 3,
                bytes: 0
            }),
        );
        assert_eq!(
            (
                transactional.next_pass(),
                transactional.decoded_segments(),
                transactional.decoded_bytes()
            ),
            (3, 2, 0)
        );
        assert_eq!(transactional.plane().signed_coefficient(0, 0).unwrap(), 0);
    }

    #[test]
    fn ht_body_native_count_reaches_physical_header_boundary_without_count_normalization() {
        let limits = ResourceLimits::default();
        let header = literal_empty_ht_contribution(165, 0);
        let mut staged = literal_ht_decoder(56);
        assert_eq!(
            decode_code_block_contribution_body_staged(&mut staged, &header, &[], &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidPacketHeader
            }),
        );
        assert_eq!(
            (
                staged.next_pass(),
                staged.decoded_segments(),
                staged.decoded_bytes()
            ),
            (164, 164, 0)
        );
        let mut transactional = literal_ht_decoder(56);
        let before = format!("{transactional:?}");
        assert_eq!(
            decode_code_block_contribution_body(&mut transactional, &header, &[], &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidPacketHeader
            }),
        );
        assert_eq!(format!("{transactional:?}"), before);
    }

    #[test]
    fn shared_code_block_body_prepare_has_exact_native_admission_and_frame() {
        for force in [false, true] {
            for high_throughput in [false, true] {
                for zero_bitplanes in [0, u16::MAX] {
                    for wrong_length in [false, true] {
                        let mut decoder = Tier1CodeBlockDecoder::new(
                            1,
                            1,
                            1,
                            crate::tier1::SubbandOrientation::Ll,
                            Tier1CodingStyle {
                                force_high_throughput: force,
                                ..Tier1CodingStyle::default()
                            },
                        )
                        .unwrap();
                        let before = format!("{decoder:?}");
                        let contribution = CodeBlockContribution {
                            first_inclusion: false,
                            zero_bitplanes,
                            new_passes: u8::MAX,
                            data_length: 1,
                            high_throughput,
                            segment_count: u8::MAX,
                            segment_passes: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
                            segment_lengths: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
                        };
                        let body: &[u8] = if wrong_length { &[] } else { &[0xff] };
                        let accepted =
                            !wrong_length && (!force || high_throughput) && zero_bitplanes == 0;
                        assert_eq!(
                            prepare_code_block_contribution_body(&mut decoder, &contribution, body),
                            if accepted {
                                Ok(())
                            } else {
                                Err(packet_invalid_code_block())
                            }
                        );
                        assert_eq!(
                            format!("{decoder:?}"),
                            if accepted {
                                before.replacen(
                                    "zero_bitplanes: None,",
                                    "zero_bitplanes: Some(0),",
                                    1,
                                )
                            } else {
                                before
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn mixed_packet_union_routes_literal_bytes_and_keeps_empty_count_distinctions() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0xc0).unwrap();
        for high in [false, true] {
            let mut decoder =
                Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
            let mut header = one_ff_legacy_contribution();
            header.high_throughput = high;
            let body: &[u8] = if high { &[0, 0x63, 0] } else { &[0xff] };
            header.data_length = body.len() as u32;
            header.segment_lengths[0] = header.data_length;
            assert_eq!(
                decode_code_block_contribution_body(&mut decoder, &header, body, &limits),
                Ok(CodeBlockBodyDecode {
                    segments: 1,
                    passes: 1,
                    bytes: header.data_length
                }),
            );
            assert_eq!(
                (
                    decoder.next_pass(),
                    decoder.decoded_segments(),
                    decoder.decoded_bytes()
                ),
                (1, 1, body.len() as u64)
            );
            assert_eq!(
                decoder.plane().signed_coefficient(0, 0).unwrap(),
                if high { -1 } else { 0 }
            );

            let mut decoder =
                Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
            let before = format!("{decoder:?}");
            header.segment_count = 0;
            header.new_passes = 0;
            header.data_length = 0;
            let result = decode_code_block_contribution_body(&mut decoder, &header, &[], &limits);
            if high {
                assert_eq!(result, Err(packet_invalid_code_block()));
                assert_eq!(format!("{decoder:?}"), before);
            } else {
                assert_eq!(
                    result,
                    Ok(CodeBlockBodyDecode {
                        segments: 0,
                        passes: 0,
                        bytes: 0
                    })
                );
                assert_eq!(
                    (
                        decoder.next_pass(),
                        decoder.decoded_segments(),
                        decoder.decoded_bytes()
                    ),
                    (0, 0, 0)
                );
            }
        }
    }

    #[test]
    fn mixed_packet_union_preserves_each_route_late_error_and_full_outer_rollback() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0xc0).unwrap();
        for high in [false, true] {
            let mut header = one_ff_legacy_contribution();
            header.high_throughput = high;
            header.new_passes = 2;
            let body: &[u8] = if high { &[0, 0x63, 0] } else { &[0xff] };
            header.data_length = body.len() as u32;
            header.segment_lengths[0] = header.data_length;
            let mut staged =
                Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
            assert_eq!(
                decode_code_block_contribution_body_staged(&mut staged, &header, body, &limits),
                Err(packet_invalid_code_block())
            );
            assert_eq!(
                (
                    staged.next_pass(),
                    staged.decoded_segments(),
                    staged.decoded_bytes()
                ),
                (1, 1, body.len() as u64)
            );
            assert_eq!(
                staged.plane().signed_coefficient(0, 0).unwrap(),
                if high { -1 } else { 0 }
            );
            let mut decoder =
                Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
            let before = format!("{decoder:?}");
            assert_eq!(
                decode_code_block_contribution_body(&mut decoder, &header, body, &limits),
                Err(packet_invalid_code_block())
            );
            assert_eq!(format!("{decoder:?}"), before);
        }
    }

    #[test]
    fn mixed_packet_union_uses_actual_route_specific_input_budget() {
        let limits = ResourceLimits::default()
            .with_maximum_input_bytes(1)
            .unwrap();
        let style = Tier1CodingStyle::from_code_block_style(0xc0).unwrap();
        for high in [false, true] {
            let mut header = one_ff_legacy_contribution();
            header.high_throughput = high;
            let body: &[u8] = if high { &[0, 0x63, 0] } else { &[0xff, 0] };
            header.data_length = body.len() as u32;
            header.segment_lengths[0] = header.data_length;
            let mut decoder =
                Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
            let before = format!("{decoder:?}");
            let result = decode_code_block_contribution_body(&mut decoder, &header, body, &limits);
            if high {
                assert_eq!(
                    result,
                    Ok(CodeBlockBodyDecode {
                        segments: 1,
                        passes: 1,
                        bytes: 3
                    })
                );
                assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), -1);
            } else {
                assert_eq!(
                    result,
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::InputBytes,
                        limit: 1,
                        actual: 2,
                    })
                );
                assert_eq!(format!("{decoder:?}"), before);
            }
        }
    }

    fn literal_precinct_body_descriptors() -> [Option<CodeBlockContribution>; 3] {
        let mut ht = one_ff_legacy_contribution();
        ht.high_throughput = true;
        ht.data_length = 3;
        ht.segment_lengths[0] = 3;
        [Some(ht), None, Some(one_ff_legacy_contribution())]
    }

    #[test]
    fn precinct_body_preflight_reports_exact_cursor_before_any_execution() {
        let contributions = literal_precinct_body_descriptors();
        for (data, expected_end, expected_index, expected_result) in [
            (&[0, 0x63, 0, 0xff][..], 4, 3, Ok(())),
            (
                &[0, 0x63, 0][..],
                4,
                2,
                Err(DecodeError::MalformedInput {
                    offset: 71,
                    kind: MalformedKind::Truncated,
                }),
            ),
        ] {
            let mut cursor = PacketBodyPreflight { index: 0, end: 0 };
            assert_eq!(
                preflight_packet_body_ranges(&contributions, data, &mut cursor, 71),
                expected_result
            );
            assert_eq!((cursor.index, cursor.end), (expected_index, expected_end));
        }
        let mut cursor = PacketBodyPreflight {
            index: 0,
            end: usize::MAX,
        };
        assert_eq!(
            preflight_packet_body_ranges(&contributions, &[], &mut cursor, 71),
            Err(packet_arithmetic_overflow())
        );
        assert_eq!((cursor.index, cursor.end), (0, usize::MAX));
        let mut cursor = PacketBodyPreflight {
            index: usize::MAX,
            end: 19,
        };
        assert_eq!(
            preflight_packet_body_ranges(&contributions, &[], &mut cursor, 71),
            Ok(())
        );
        assert_eq!((cursor.index, cursor.end), (usize::MAX, 19));
    }

    #[test]
    fn precinct_skipped_execution_consumes_ranges_without_decoder_shape_or_counter_changes() {
        let contributions = literal_precinct_body_descriptors();
        for (data, expected_offset, expected_index, expected_result) in [
            (&[0, 0x63, 0, 0xff][..], 4, 3, Ok(())),
            (
                &[0, 0x63, 0][..],
                3,
                2,
                Err(DecodeError::MalformedInput {
                    offset: 71,
                    kind: MalformedKind::Truncated,
                }),
            ),
        ] {
            let mut progress = PacketBodyExecutionProgress {
                index: 0,
                offset: 0,
                segments: u32::MAX,
                passes: u32::MAX,
            };
            assert_eq!(
                execute_packet_body_contributions(
                    &mut [],
                    &contributions,
                    data,
                    &mut progress,
                    71,
                    false,
                    &ResourceLimits::default(),
                ),
                expected_result
            );
            assert_eq!(
                (
                    progress.index,
                    progress.offset,
                    progress.segments,
                    progress.passes
                ),
                (expected_index, expected_offset, u32::MAX, u32::MAX)
            );
        }
    }

    #[test]
    fn precinct_execution_retains_real_prior_coefficients_and_late_partial_counters() {
        let contributions = literal_precinct_body_descriptors();
        let data = [0, 0x63, 0, 0xff];
        let style = Tier1CodingStyle::from_code_block_style(0xc0).unwrap();
        let decoder =
            || Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
        let mut decoders = vec![decoder()];
        let mut progress = PacketBodyExecutionProgress {
            index: 0,
            offset: 0,
            segments: 0,
            passes: 0,
        };
        assert_eq!(
            execute_packet_body_contributions(
                &mut decoders,
                &contributions,
                &data,
                &mut progress,
                71,
                true,
                &ResourceLimits::default(),
            ),
            Err(invalid_packet_header())
        );
        assert_eq!(
            (
                progress.index,
                progress.offset,
                progress.segments,
                progress.passes
            ),
            (2, 3, 1, 1)
        );
        assert_eq!(decoders[0].plane().signed_coefficient(0, 0).unwrap(), -1);
        assert_eq!(
            (decoders[0].next_pass(), decoders[0].decoded_bytes()),
            (1, 3)
        );
        for (segments, passes, expected_segments) in [(u32::MAX, 7, u32::MAX), (9, u32::MAX, 10)] {
            let mut decoders = vec![decoder()];
            let mut progress = PacketBodyExecutionProgress {
                index: 0,
                offset: 0,
                segments,
                passes,
            };
            assert_eq!(
                execute_packet_body_contributions(
                    &mut decoders,
                    &contributions,
                    &data,
                    &mut progress,
                    71,
                    true,
                    &ResourceLimits::default(),
                ),
                Err(packet_arithmetic_overflow())
            );
            assert_eq!(
                (
                    progress.index,
                    progress.offset,
                    progress.segments,
                    progress.passes
                ),
                (0, 0, expected_segments, passes)
            );
            assert_eq!(decoders[0].plane().signed_coefficient(0, 0).unwrap(), -1);
            assert_eq!(
                (
                    decoders[0].next_pass(),
                    decoders[0].decoded_segments(),
                    decoders[0].decoded_bytes()
                ),
                (1, 1, 3)
            );
        }
    }

    fn one_ff_legacy_contribution() -> CodeBlockContribution {
        let mut contribution = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 1,
            high_throughput: false,
            segment_count: 1,
            segment_passes: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
            segment_lengths: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
        };
        contribution.segment_passes[0] = 1;
        contribution.segment_lengths[0] = 1;
        contribution
    }

    fn one_bit_legacy_decoder(force_high_throughput: bool) -> Tier1CodeBlockDecoder {
        Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle {
                force_high_throughput,
                ..Tier1CodingStyle::default()
            },
        )
        .unwrap()
    }

    fn expected_one_ff_decoder(before: &Tier1CodeBlockDecoder, header_prepared: bool) -> String {
        let mut contexts = crate::mq::MqDecoder::new(&[], &ResourceLimits::default()).unwrap();
        for index in 0..19 {
            contexts
                .set_context(
                    index,
                    match index {
                        0 => 5,
                        17 => 3,
                        18 => 46,
                        _ => 0,
                    },
                    false,
                )
                .unwrap();
        }
        let mut expected = format!("{before:?}")
            .replacen("next_pass: 0,", "next_pass: 1,", 1)
            .replacen("decoded_segments: 0,", "decoded_segments: 1,", 1)
            .replacen("decoded_bytes: 0,", "decoded_bytes: 1,", 1)
            .replacen("legacy_decode_work: 0,", "legacy_decode_work: 2,", 1)
            .replacen(
                "mq_context_state: None,",
                &format!("mq_context_state: Some({:?}),", contexts.context_state()),
                1,
            );
        if header_prepared {
            expected = expected.replacen("zero_bitplanes: None,", "zero_bitplanes: Some(0),", 1);
        }
        expected
    }

    #[test]
    fn legacy_packet_body_routes_real_mq_bytes_and_retains_late_total_error_mutations() {
        for mismatch in 0..3 {
            let mut decoder = one_bit_legacy_decoder(false);
            let expected = expected_one_ff_decoder(&decoder, false);
            let mut contribution = one_ff_legacy_contribution();
            let body: &[u8] = if mismatch == 2 { &[0xff, 0] } else { &[0xff] };
            if mismatch == 1 {
                contribution.new_passes = 2;
            }
            let result = decode_legacy_code_block_contribution_body(
                &mut decoder,
                &contribution,
                body,
                &ResourceLimits::default(),
            );
            assert_eq!(format!("{decoder:?}"), expected);
            assert_eq!(
                result,
                if mismatch == 0 {
                    Ok(CodeBlockBodyDecode {
                        segments: 1,
                        passes: 1,
                        bytes: 1,
                    })
                } else {
                    Err(packet_invalid_code_block())
                }
            );
        }
        let mut decoder = one_bit_legacy_decoder(false);
        let before = format!("{decoder:?}");
        let mut contribution = one_ff_legacy_contribution();
        contribution.segment_count = 0;
        contribution.new_passes = 0;
        contribution.data_length = u32::MAX;
        contribution.high_throughput = true;
        assert_eq!(
            decode_legacy_code_block_contribution_body(
                &mut decoder,
                &contribution,
                &[],
                &ResourceLimits::default(),
            ),
            Ok(CodeBlockBodyDecode {
                segments: 0,
                passes: 0,
                bytes: u32::MAX
            })
        );
        assert_eq!(format!("{decoder:?}"), before);
    }

    #[test]
    fn legacy_packet_segment_checks_real_range_and_counter_priority_without_input_invariants() {
        let limits = ResourceLimits::default();
        let mut contribution = one_ff_legacy_contribution();
        for offset in [0, usize::MAX] {
            let mut decoder = one_bit_legacy_decoder(false);
            let before = format!("{decoder:?}");
            let expected_decoded = expected_one_ff_decoder(&decoder, false);
            let mut index = 0;
            let mut current_offset = offset;
            let mut passes = u32::MAX;
            assert_eq!(
                decode_legacy_code_block_segment(
                    &mut decoder,
                    &contribution,
                    &[0xff],
                    &limits,
                    &mut index,
                    &mut current_offset,
                    &mut passes,
                ),
                Err(packet_arithmetic_overflow())
            );
            assert_eq!((index, current_offset, passes), (0, offset, u32::MAX));
            assert_eq!(
                format!("{decoder:?}"),
                if offset == 0 {
                    expected_decoded
                } else {
                    before
                }
            );
        }
        contribution.segment_count = u8::MAX;
        let mut decoder = one_bit_legacy_decoder(false);
        let before = format!("{decoder:?}");
        let (mut index, mut offset, mut passes) = (164, usize::MAX, u32::MAX);
        assert_eq!(
            decode_legacy_code_block_segment(
                &mut decoder,
                &contribution,
                &[],
                &limits,
                &mut index,
                &mut offset,
                &mut passes,
            ),
            Err(invalid_packet_header())
        );
        assert_eq!((index, offset, passes), (164, usize::MAX, u32::MAX));
        assert_eq!(format!("{decoder:?}"), before);
    }

    #[test]
    fn real_packet_prefix_and_outer_clone_commit_preserve_order_and_full_rollback() {
        for force in [false, true] {
            for wrong_length in [false, true] {
                let mut decoder = one_bit_legacy_decoder(force);
                let before = format!("{decoder:?}");
                let contribution = one_ff_legacy_contribution();
                let body: &[u8] = if wrong_length { &[] } else { &[0xff] };
                let expected = if !force && !wrong_length {
                    before.replacen("zero_bitplanes: None,", "zero_bitplanes: Some(0),", 1)
                } else {
                    before
                };
                assert_eq!(
                    prepare_code_block_contribution_body(&mut decoder, &contribution, body),
                    if force || wrong_length {
                        Err(packet_invalid_code_block())
                    } else {
                        Ok(())
                    }
                );
                assert_eq!(format!("{decoder:?}"), expected);
            }
        }
        for wrong_total in [false, true] {
            let mut decoder = one_bit_legacy_decoder(false);
            let before = format!("{decoder:?}");
            let decoded = expected_one_ff_decoder(&decoder, true);
            let mut contribution = one_ff_legacy_contribution();
            if wrong_total {
                contribution.new_passes = 2;
            }
            let result = decode_code_block_contribution_body(
                &mut decoder,
                &contribution,
                &[0xff],
                &ResourceLimits::default(),
            );
            assert_eq!(
                result,
                if wrong_total {
                    Err(packet_invalid_code_block())
                } else {
                    Ok(CodeBlockBodyDecode {
                        segments: 1,
                        passes: 1,
                        bytes: 1,
                    })
                }
            );
            assert_eq!(
                format!("{decoder:?}"),
                if wrong_total { before } else { decoded }
            );
        }
    }

    #[test]
    fn native_contribution_segment_lookup_covers_all_counts_indices_and_full_stored_arrays() {
        let mut contribution = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: u16::MAX,
            new_passes: u8::MAX,
            data_length: u32::MAX,
            high_throughput: true,
            segment_count: 0,
            segment_passes: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
            segment_lengths: [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION],
        };
        for index in 0..MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION {
            contribution.segment_passes[index] = u8::MAX - index as u8;
            contribution.segment_lengths[index] = u32::MAX - index as u32;
        }
        for count in 0..=u8::MAX {
            contribution.segment_count = count;
            let before = contribution;
            assert_eq!(contribution.segment_count(), count);
            for index in 0..=u8::MAX {
                let expected = if index < count
                    && usize::from(index) < MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION
                {
                    Ok(CodeBlockSegmentContribution {
                        passes: u8::MAX - index,
                        data_length: u32::MAX - u32::from(index),
                    })
                } else {
                    Err(invalid_packet_header())
                };
                assert_eq!(contribution.segment(index), expected);
                assert_eq!(contribution, before);
            }
        }
    }

    #[test]
    fn readonly_precinct_decoder_accessors_use_actual_vec_and_entire_native_indices() {
        for count in 0..=4 {
            let mut decoders = Vec::with_capacity(count + 8);
            for index in 0..count {
                decoders.push(
                    Tier1CodeBlockDecoder::new(
                        u16::try_from(index + 1).unwrap(),
                        1,
                        1,
                        SubbandOrientation::Ll,
                        Tier1CodingStyle::default(),
                    )
                    .unwrap(),
                );
            }
            let precinct = PacketPrecinctDecoder {
                component: u16::MAX,
                resolution: u8::MAX,
                precinct: u32::MAX,
                states: Vec::new(),
                contexts: Vec::new(),
                decoders,
                subbands: Vec::new(),
            };
            assert_eq!(precinct.decoder_count(), count);
            assert_eq!(precinct.decoders().len(), count);
            for index in (0..count + 9).chain([usize::MAX / 2, usize::MAX - 1, usize::MAX]) {
                match precinct.decoder(index) {
                    Ok(value) => {
                        assert!(index < count);
                        assert!(core::ptr::eq(value, &precinct.decoders()[index]));
                    }
                    Err(error) => {
                        assert!(index >= count);
                        assert_eq!(
                            error,
                            DecodeError::MalformedInput {
                                offset: 0,
                                kind: MalformedKind::InvalidPacketHeader
                            }
                        );
                    }
                }
            }
        }
    }
    use crate::codestream::{
        CodingStyle, ComponentCodingStyles, ComponentQuantizations, Quantization, WaveletTransform,
    };
    use std::collections::HashSet;

    fn coding_style(sop_markers: bool, eph_markers: bool) -> CodingStyle {
        CodingStyle {
            progression_order: ProgressionOrder::Lrcp,
            layers: 2,
            multiple_component_transform: false,
            decomposition_levels: 0,
            decomposition_structure: None,
            decomposition_directions: [crate::codestream::DecompositionDirection::TwoDimensional;
                32],
            code_block_width: 1,
            code_block_height: 1,
            code_block_style: 0,
            transform: WaveletTransform::Reversible5x3,
            precincts_defined: false,
            sop_markers,
            eph_markers,
            precinct_exponents: [(15, 15); 33],
        }
    }

    fn component_styles(style: CodingStyle) -> (ComponentCodingStyles, ComponentQuantizations) {
        (
            ComponentCodingStyles::uniform(1, style).unwrap(),
            ComponentQuantizations::uniform(1, Quantization::reversible_default(style)).unwrap(),
        )
    }

    fn pack_packet_bits(bits: &[bool]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut current = 0_u8;
        let mut used = 0_u8;
        let mut capacity = 8_u8;
        for bit in bits {
            current <<= 1;
            current |= u8::from(*bit);
            used += 1;
            if used == capacity {
                bytes.push(current);
                capacity = if current == 0xff { 7 } else { 8 };
                current = 0;
                used = 0;
            }
        }
        if used != 0 {
            current <<= capacity - used;
            bytes.push(current);
        }
        bytes
    }

    fn two_block_first_layer_header() -> Vec<u8> {
        pack_packet_bits(&[
            true, // non-empty packet
            true, true, // block 0 inclusion tree: root, leaf
            true, true,  // block 0 zero-bitplane tree: root, leaf
            false, // one coding pass
            false, // no Lblock increment
            false, true, false, // two body bytes
            true,  // block 1 inclusion leaf
            true,  // block 1 zero-bitplane leaf
            false, // one coding pass
            false, // no Lblock increment
            false, true, false, // two body bytes
        ])
    }

    #[test]
    fn decodes_every_packet_coding_pass_count_and_length_width() {
        for passes in 1_u32..=164 {
            let (mut bits, suffix, count) = match passes {
                1 => (vec![false], 0, 0),
                2 => (vec![true, false], 0, 0),
                3..=5 => (vec![true, true], passes - 3, 2),
                6..=36 => (vec![true; 4], passes - 6, 5),
                _ => (vec![true; 9], passes - 37, 7),
            };
            for bit in (0..count).rev() {
                bits.push(suffix & (1 << bit) != 0);
            }
            bits.push(true);
            let bytes = pack_packet_bits(&bits);
            let mut reader = PacketBitReader::new(&bytes, &ResourceLimits::default()).unwrap();
            assert_eq!(u32::from(read_new_passes(&mut reader).unwrap()), passes);
            assert!(
                reader.read_bit().unwrap(),
                "pass count consumed the next field"
            );
            assert_eq!(u32::from(floor_log2(passes)), 31 - passes.leading_zeros());
        }
    }

    #[test]
    fn reads_bits_most_significant_first() {
        let mut reader = PacketBitReader::new(&[0b1010_0000], &ResourceLimits::default()).unwrap();
        assert!(reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert_eq!(reader.bits_read(), 3);
        assert_eq!(reader.bytes_consumed(), 1);
    }

    #[test]
    fn enforces_stuffed_bit_after_ff() {
        let mut reader = PacketBitReader::new(&[0xff, 0x7f], &ResourceLimits::default()).unwrap();
        assert_eq!(reader.read_bits(8).unwrap(), 0xff);
        assert!(reader.read_bit().unwrap());
        assert_eq!(reader.bits_read(), 9);

        let mut invalid = PacketBitReader::new(&[0xff, 0x80], &ResourceLimits::default()).unwrap();
        assert_eq!(invalid.read_bits(8).unwrap(), 0xff);
        assert!(matches!(
            invalid.read_bit(),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBitStuffing,
                ..
            })
        ));
    }

    #[test]
    fn aligns_and_reports_truncation() {
        let mut reader = PacketBitReader::new(&[0b1000_0000], &ResourceLimits::default()).unwrap();
        assert!(reader.read_bit().unwrap());
        reader.align_to_byte();
        assert!(reader.is_byte_aligned());
        assert!(matches!(
            reader.read_bit(),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::Truncated,
                ..
            })
        ));
    }

    #[test]
    fn rejects_excessive_bit_counts_and_header_bytes() {
        let mut reader = PacketBitReader::new(&[0; 4], &ResourceLimits::default()).unwrap();
        assert!(reader.read_bits(33).is_err());
        let limits = ResourceLimits::default()
            .with_maximum_packet_header_bytes(1)
            .unwrap();
        assert!(matches!(
            PacketBitReader::new(&[0; 4], &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                ..
            })
        ));
    }

    #[test]
    fn constructs_bounded_tag_tree_topology() {
        let tree = TagTree::new(3, 2, &ResourceLimits::default()).unwrap();
        assert_eq!(tree.leaf_count().unwrap(), 6);
        assert_eq!(tree.node_count(), 9);
    }

    #[test]
    fn decodes_and_reuses_monotonic_tag_tree_state() {
        let mut tree = TagTree::new(1, 1, &ResourceLimits::default()).unwrap();
        let mut reader = PacketBitReader::new(&[0b0010_0000], &ResourceLimits::default()).unwrap();
        let value = tree.decode(0, 0, 3, &mut reader).unwrap();
        assert_eq!(value.value, Some(2));
        assert!(value.is_below_threshold);
        assert_eq!(reader.bits_read(), 3);

        let repeated = tree.decode(0, 0, 4, &mut reader).unwrap();
        assert_eq!(repeated.value, Some(2));
        assert_eq!(reader.bits_read(), 3);
    }

    #[test]
    fn rejects_invalid_tag_tree_coordinates() {
        let mut tree = TagTree::new(2, 2, &ResourceLimits::default()).unwrap();
        let mut reader = PacketBitReader::new(&[0], &ResourceLimits::default()).unwrap();
        assert!(matches!(
            tree.decode(2, 0, 1, &mut reader),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTagTree,
                ..
            })
        ));
    }

    #[test]
    fn preserves_descendants_when_ancestor_is_not_below_threshold() {
        let mut tree = TagTree::new(2, 1, &ResourceLimits::default()).unwrap();
        let mut reader = PacketBitReader::new(&[0b0110_0000], &ResourceLimits::default()).unwrap();
        let first = tree.decode(0, 0, 1, &mut reader).unwrap();
        assert!(!first.is_below_threshold);
        assert_eq!(reader.bits_read(), 1);

        let second = tree.decode(0, 0, 2, &mut reader).unwrap();
        assert_eq!(second.value, Some(1));
        assert_eq!(reader.bits_read(), 3);
    }

    #[test]
    fn all_progression_orders_emit_identical_packet_sets() {
        let grid = PacketGrid::new(2, 2, 2, vec![1, 2, 3, 1], &ResourceLimits::default()).unwrap();
        assert_eq!(grid.packet_count(), 14);
        let orders = [
            ProgressionOrder::Lrcp,
            ProgressionOrder::Rlcp,
            ProgressionOrder::Rpcl,
            ProgressionOrder::Pcrl,
            ProgressionOrder::Cprl,
        ];
        let mut baseline = None;
        for order in orders {
            let mut iterator = PacketIterator::new(&grid, order);
            let packets = iterator
                .by_ref()
                .collect::<Result<HashSet<PacketCoordinate>, _>>()
                .unwrap();
            assert_eq!(packets.len(), 14);
            assert_eq!(iterator.emitted(), 14);
            if let Some(expected) = &baseline {
                assert_eq!(&packets, expected);
            } else {
                baseline = Some(packets);
            }
        }
    }

    #[test]
    fn progression_iteration_is_order_specific_and_bounded() {
        let grid = PacketGrid::new(2, 2, 1, vec![1, 1], &ResourceLimits::default()).unwrap();
        let mut lrcp = PacketIterator::new(&grid, ProgressionOrder::Lrcp);
        assert_eq!(
            lrcp.next_packet().unwrap(),
            Some(PacketCoordinate {
                layer: 0,
                resolution: 0,
                component: 0,
                precinct: 0,
            })
        );
        assert_eq!(
            lrcp.next_packet().unwrap(),
            Some(PacketCoordinate {
                layer: 0,
                resolution: 0,
                component: 1,
                precinct: 0,
            })
        );

        let limits = ResourceLimits::default().with_maximum_packets(1).unwrap();
        assert!(matches!(
            PacketGrid::new(2, 1, 1, vec![1], &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                ..
            })
        ));
    }

    #[test]
    fn progression_change_windows_emit_each_packet_once_in_window_order() {
        let limits = ResourceLimits::default();
        let grid = PacketGrid::new(2, 2, 1, vec![1, 1], &limits).unwrap();
        let changes = ProgressionChanges::from_records(vec![
            ProgressionChange {
                resolution_start: 0,
                component_start: 0,
                layer_end: 2,
                resolution_end: 1,
                component_end: 1,
                progression_order: ProgressionOrder::Lrcp,
            },
            ProgressionChange {
                resolution_start: 0,
                component_start: 1,
                layer_end: 2,
                resolution_end: 1,
                component_end: 2,
                progression_order: ProgressionOrder::Rlcp,
            },
        ]);
        let mut iterator =
            ProgressionPacketIterator::new(&grid, ProgressionOrder::Cprl, &changes, &limits)
                .unwrap();
        let mut packets = Vec::new();
        while let Some(packet) = iterator.next_packet().unwrap() {
            packets.push(packet);
        }
        assert_eq!(
            packets,
            vec![
                PacketCoordinate {
                    layer: 0,
                    resolution: 0,
                    component: 0,
                    precinct: 0,
                },
                PacketCoordinate {
                    layer: 1,
                    resolution: 0,
                    component: 0,
                    precinct: 0,
                },
                PacketCoordinate {
                    layer: 0,
                    resolution: 0,
                    component: 1,
                    precinct: 0,
                },
                PacketCoordinate {
                    layer: 1,
                    resolution: 0,
                    component: 1,
                    precinct: 0,
                },
            ]
        );
        assert_eq!(iterator.emitted(), grid.packet_count());
    }

    #[test]
    fn overlapping_progression_windows_skip_previously_emitted_packets() {
        let limits = ResourceLimits::default();
        let grid = PacketGrid::new(2, 1, 1, vec![1], &limits).unwrap();
        let changes = ProgressionChanges::from_records(vec![
            ProgressionChange {
                resolution_start: 0,
                component_start: 0,
                layer_end: 1,
                resolution_end: 1,
                component_end: 1,
                progression_order: ProgressionOrder::Rlcp,
            },
            ProgressionChange {
                resolution_start: 0,
                component_start: 0,
                layer_end: 2,
                resolution_end: 1,
                component_end: 1,
                progression_order: ProgressionOrder::Lrcp,
            },
        ]);
        let mut iterator =
            ProgressionPacketIterator::new(&grid, ProgressionOrder::Cprl, &changes, &limits)
                .unwrap();
        let mut unique = HashSet::new();
        while let Some(packet) = iterator.next_packet().unwrap() {
            assert!(unique.insert(packet));
        }
        assert_eq!(unique.len() as u64, grid.packet_count());
    }

    #[test]
    fn decodes_code_block_packet_contributions() {
        let limits = ResourceLimits::default();
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut state = CodeBlockPacketState::new();
        let mut first_reader = PacketBitReader::new(&[0xca], &limits).unwrap();
        let first = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 8,
                tier1_style: Tier1CodingStyle::default(),
            },
            &mut first_reader,
        )
        .unwrap()
        .unwrap();
        assert!(first.first_inclusion);
        assert_eq!(first.zero_bitplanes, 0);
        assert_eq!(first.new_passes, 1);
        assert_eq!(first.data_length, 5);
        assert_eq!(first.segment_count(), 1);
        assert_eq!(
            first.segment(0).unwrap(),
            CodeBlockSegmentContribution {
                passes: 1,
                data_length: 5,
            }
        );
        assert_eq!(state.total_passes(), 1);

        let mut second_reader = PacketBitReader::new(&[0xd0, 0xc0], &limits).unwrap();
        let second = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 1,
                maximum_zero_bitplanes: 8,
                tier1_style: Tier1CodingStyle::default(),
            },
            &mut second_reader,
        )
        .unwrap()
        .unwrap();
        assert!(!second.first_inclusion);
        assert_eq!(second.new_passes, 2);
        assert_eq!(second.data_length, 3);
        assert_eq!(second.segment_count(), 1);
        assert_eq!(second.segment(0).unwrap().passes, 2);
        assert_eq!(state.total_passes(), 3);
        assert_eq!(state.lblock(), 4);

        let previous = state;
        let mut absent_reader = PacketBitReader::new(&[0], &limits).unwrap();
        assert!(
            decode_code_block_contribution(
                &mut state,
                &mut inclusion,
                &mut zero_bitplanes,
                CodeBlockPacketContext {
                    leaf_x: 0,
                    leaf_y: 0,
                    layer: 2,
                    maximum_zero_bitplanes: 8,
                    tier1_style: Tier1CodingStyle::default(),
                },
                &mut absent_reader,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(state, previous);
    }

    #[test]
    fn preserves_code_block_state_on_invalid_length() {
        let limits = ResourceLimits::default();
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut state = CodeBlockPacketState::new();
        let initial = state;
        let mut reader = PacketBitReader::new(&[0xc0], &limits).unwrap();
        assert!(matches!(
            decode_code_block_contribution(
                &mut state,
                &mut inclusion,
                &mut zero_bitplanes,
                CodeBlockPacketContext {
                    leaf_x: 0,
                    leaf_y: 0,
                    layer: 0,
                    maximum_zero_bitplanes: 8,
                    tier1_style: Tier1CodingStyle::default(),
                },
                &mut reader,
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketHeader,
                ..
            })
        ));
        assert_eq!(state, initial);
    }

    #[test]
    fn permits_zero_byte_legacy_continuation_without_accepting_an_empty_initial_segment() {
        let limits = ResourceLimits::default();
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut state = CodeBlockPacketState {
            included: true,
            zero_bitplanes: Some(0),
            lblock: 3,
            total_passes: 9,
            high_throughput: Some(false),
        };
        let mut reader = PacketBitReader::new(&[0x80], &limits).unwrap();
        let contribution = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 1,
                maximum_zero_bitplanes: 8,
                tier1_style: Tier1CodingStyle::from_code_block_style(1).unwrap(),
            },
            &mut reader,
        )
        .unwrap()
        .unwrap();
        assert!(!contribution.first_inclusion);
        assert_eq!(contribution.new_passes, 1);
        assert_eq!(contribution.data_length, 0);
        assert_eq!(contribution.segment_count(), 1);
        assert_eq!(contribution.segment(0).unwrap().data_length, 0);
        assert_eq!(state.total_passes(), 10);

        let initial = CodeBlockPacketState::new();
        state = initial;
        let mut reader = PacketBitReader::new(&[0xc0], &limits).unwrap();
        assert!(
            decode_code_block_contribution(
                &mut state,
                &mut inclusion,
                &mut zero_bitplanes,
                CodeBlockPacketContext {
                    leaf_x: 0,
                    leaf_y: 0,
                    layer: 0,
                    maximum_zero_bitplanes: 8,
                    tier1_style: Tier1CodingStyle::default(),
                },
                &mut reader,
            )
            .is_err()
        );
        assert_eq!(state, initial);
    }

    #[test]
    fn mixed_mode_routes_from_first_nonzero_segment_length() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0xc0).unwrap();

        let mut ht_inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut ht_zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut ht_state = CodeBlockPacketState::new();
        let ht_header =
            pack_packet_bits(&[true, true, false, true, false, false, true, false, false]);
        let mut ht_reader = PacketBitReader::new(&ht_header, &limits).unwrap();
        let ht = decode_code_block_contribution(
            &mut ht_state,
            &mut ht_inclusion,
            &mut ht_zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 3,
                tier1_style: style,
            },
            &mut ht_reader,
        )
        .unwrap()
        .unwrap();
        assert!(ht.high_throughput);
        assert_eq!(ht_state.high_throughput(), Some(true));
        assert_eq!(ht.data_length, 4);

        let mut legacy_inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut legacy_zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut legacy_state = CodeBlockPacketState::new();
        let legacy_header = pack_packet_bits(&[true, true, false, false, false, true, false]);
        let mut legacy_reader = PacketBitReader::new(&legacy_header, &limits).unwrap();
        let legacy = decode_code_block_contribution(
            &mut legacy_state,
            &mut legacy_inclusion,
            &mut legacy_zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 3,
                tier1_style: style,
            },
            &mut legacy_reader,
        )
        .unwrap()
        .unwrap();
        assert!(!legacy.high_throughput);
        assert_eq!(legacy_state.high_throughput(), Some(false));
        assert_eq!(legacy.data_length, 2);

        let mut extended_inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut extended_zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut extended_state = CodeBlockPacketState::new();
        let extended_header =
            pack_packet_bits(&[true, true, true, false, false, false, false, false, true]);
        let mut extended_reader = PacketBitReader::new(&extended_header, &limits).unwrap();
        let extended = decode_code_block_contribution(
            &mut extended_state,
            &mut extended_inclusion,
            &mut extended_zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 3,
                tier1_style: style,
            },
            &mut extended_reader,
        )
        .unwrap()
        .unwrap();
        assert!(!extended.high_throughput);
        assert_eq!(extended.new_passes, 2);
        assert_eq!(extended.segment(0).unwrap().passes, 2);
        assert_eq!(extended.data_length, 1);
    }

    #[test]
    fn continues_one_legacy_coding_segment_across_quality_layers() {
        let limits = ResourceLimits::default();
        let contribution = |passes: u8, bytes: u32, first_inclusion: bool| {
            let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
            let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
            segment_passes[0] = passes;
            segment_lengths[0] = bytes;
            CodeBlockContribution {
                first_inclusion,
                zero_bitplanes: 2,
                new_passes: passes,
                data_length: bytes,
                high_throughput: false,
                segment_count: 1,
                segment_passes,
                segment_lengths,
            }
        };
        let mut whole = Tier1CodeBlockDecoder::new(
            1,
            1,
            9,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let mut layered = whole.clone();
        decode_code_block_contribution_body(
            &mut whole,
            &contribution(19, 2, true),
            &[0x04, 0x4f],
            &limits,
        )
        .unwrap();
        decode_code_block_contribution_body(
            &mut layered,
            &contribution(1, 1, true),
            &[0x04],
            &limits,
        )
        .unwrap();
        let before = layered.plane().coefficients().to_vec();
        let tight = limits.with_maximum_work_units(22).unwrap();
        assert!(matches!(
            decode_code_block_contribution_body(
                &mut layered,
                &contribution(18, 1, false),
                &[0x4f],
                &tight,
            ),
            Err(DecodeError::LimitExceeded {
                resource: crate::Resource::WorkUnits,
                ..
            })
        ));
        assert_eq!(layered.plane().coefficients(), before);
        assert_eq!(layered.next_pass(), 1);
        decode_code_block_contribution_body(
            &mut layered,
            &contribution(18, 1, false),
            &[0x4f],
            &limits,
        )
        .unwrap();
        assert_eq!(layered.plane().coefficients(), whole.plane().coefficients());
        assert_eq!(layered.decoded_bytes(), 2);
        assert_eq!(layered.next_pass(), 19);
    }

    #[test]
    fn decodes_bounded_code_block_contribution_body() {
        let limits = ResourceLimits::default();
        let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        segment_passes[0] = 1;
        segment_lengths[0] = 2;
        let contribution = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 2,
            high_throughput: false,
            segment_count: 1,
            segment_passes,
            segment_lengths,
        };
        let mut decoder = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let decoded = decode_code_block_contribution_body(
            &mut decoder,
            &contribution,
            &[0x01, 0xae],
            &limits,
        )
        .unwrap();
        assert_eq!(
            decoded,
            CodeBlockBodyDecode {
                segments: 1,
                passes: 1,
                bytes: 2,
            }
        );
        assert!(decoder.is_complete());

        let mut truncated_decoder = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        assert!(
            decode_code_block_contribution_body(
                &mut truncated_decoder,
                &contribution,
                &[0x01],
                &limits,
            )
            .is_err()
        );
        assert_eq!(truncated_decoder.next_pass(), 0);
    }

    #[test]
    fn decodes_ht_cleanup_with_packet_missing_msbs() {
        // Independently encoded one-pixel residuals from OpenJPH 0.33.0.
        for (body, magnitude) in [
            ([0x8e, 0x01, 0x07, 0x74, 0x00], 72),
            ([0xfc, 0x01, 0x07, 0x74, 0x00], 127),
        ] {
            let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
            let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
            segment_passes[0] = 1;
            segment_lengths[0] = 5;
            let contribution = CodeBlockContribution {
                first_inclusion: true,
                zero_bitplanes: 6,
                new_passes: 1,
                data_length: 5,
                high_throughput: true,
                segment_count: 1,
                segment_passes,
                segment_lengths,
            };
            let mut decoder = Tier1CodeBlockDecoder::new(
                1,
                1,
                7,
                crate::tier1::SubbandOrientation::Ll,
                Tier1CodingStyle::from_code_block_style(0x40).unwrap(),
            )
            .unwrap();
            decoder.set_ht_magnitude_bound(8).unwrap();
            decode_code_block_contribution_body(
                &mut decoder,
                &contribution,
                &body,
                &ResourceLimits::default(),
            )
            .unwrap();
            let coefficient = decoder.plane().coefficient(0, 0).unwrap();
            assert!(coefficient.significant);
            assert!(!coefficient.negative);
            assert_eq!(coefficient.magnitude, magnitude);
            assert_eq!(decoder.next_pass(), 1);
        }
    }

    #[test]
    fn decodes_ht_embedded_precision_separately_from_value() {
        let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        segment_passes[0] = 1;
        segment_lengths[0] = 8;
        let contribution = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 6,
            new_passes: 1,
            data_length: 8,
            high_throughput: true,
            segment_count: 1,
            segment_passes,
            segment_lengths,
        };
        let mut decoder = Tier1CodeBlockDecoder::new(
            2,
            2,
            7,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::from_code_block_style(0x40).unwrap(),
        )
        .unwrap();
        decoder.set_ht_magnitude_bound(8).unwrap();
        let body = [0x00, 0x82, 0xc0, 0xf1, 0x00, 0x43, 0x94, 0x00];
        decode_code_block_contribution_body(
            &mut decoder,
            &contribution,
            &body,
            &ResourceLimits::default(),
        )
        .unwrap();
        for (x, y, value) in [(0, 0, 1), (1, 0, 2), (0, 1, 3), (1, 1, 72)] {
            assert_eq!(decoder.plane().signed_coefficient(x, y).unwrap(), value);
        }
    }

    #[test]
    fn ht_final_magnitude_refinement_truncates_the_integer_half_bin() {
        for (bit, magnitude) in [(0, 2), (1, 3)] {
            let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
            let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
            segment_passes[0] = 1;
            segment_passes[1] = 2;
            segment_lengths[0] = 4;
            segment_lengths[1] = 1;
            let contribution = CodeBlockContribution {
                first_inclusion: true,
                zero_bitplanes: 5,
                new_passes: 3,
                data_length: 5,
                high_throughput: true,
                segment_count: 2,
                segment_passes,
                segment_lengths,
            };
            let mut decoder = Tier1CodeBlockDecoder::new(
                1,
                1,
                7,
                crate::tier1::SubbandOrientation::Ll,
                Tier1CodingStyle::from_code_block_style(0x40).unwrap(),
            )
            .unwrap();
            decoder.set_ht_magnitude_bound(8).unwrap();
            let body = [0xfe, 0x00, 0x63, 0x00, bit];
            decode_code_block_contribution_body(
                &mut decoder,
                &contribution,
                &body,
                &ResourceLimits::default(),
            )
            .unwrap();
            assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), magnitude);
            assert!(decoder.plane().coefficient(0, 0).unwrap().refined);
        }
    }

    #[test]
    fn routes_valid_ht_body_after_grouping_to_coefficient_boundary() {
        let limits = ResourceLimits::default();
        let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        segment_passes[0] = 1;
        segment_lengths[0] = 3;
        segment_passes[1] = 1;
        segment_lengths[1] = 0;
        let contribution = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 2,
            data_length: 3,
            high_throughput: true,
            segment_count: 2,
            segment_passes,
            segment_lengths,
        };
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let mut decoder =
            Tier1CodeBlockDecoder::new(1, 1, 3, SubbandOrientation::Ll, style).unwrap();
        let decoded = decode_code_block_contribution_body(
            &mut decoder,
            &contribution,
            &[0x00, 0x63, 0x00],
            &limits,
        )
        .unwrap();
        assert_eq!(decoded.passes, 2);
        assert_eq!(decoded.segments, 2);
        assert_eq!(decoder.next_pass(), 2);
        assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), -6);

        let mut malformed_decoder =
            Tier1CodeBlockDecoder::new(1, 1, 3, SubbandOrientation::Ll, style).unwrap();
        assert!(matches!(
            decode_code_block_contribution_body(
                &mut malformed_decoder,
                &contribution,
                &[0x01, 0x00, 0x00],
                &limits,
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidCodeBlock,
                ..
            })
        ));
    }

    #[test]
    fn routes_cleanup_only_ht_body_into_coefficient_plane() {
        let limits = ResourceLimits::default();
        let mut segment_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut segment_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        segment_passes[0] = 1;
        segment_lengths[0] = 4;
        let contribution = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 4,
            high_throughput: true,
            segment_count: 1,
            segment_passes,
            segment_lengths,
        };
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let mut decoder =
            Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, style).unwrap();
        let decoded = decode_code_block_contribution_body(
            &mut decoder,
            &contribution,
            &[0xfe, 0x00, 0x63, 0x00],
            &limits,
        )
        .unwrap();
        assert_eq!(
            decoded,
            CodeBlockBodyDecode {
                segments: 1,
                passes: 1,
                bytes: 4,
            }
        );
        assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), 1);
        assert!(decoder.is_complete());
    }

    #[test]
    fn routes_ht_refinement_across_layer_contributions() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let mut decoder =
            Tier1CodeBlockDecoder::new(1, 1, 2, SubbandOrientation::Ll, style).unwrap();

        let mut cleanup_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut cleanup_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        cleanup_passes[0] = 1;
        cleanup_lengths[0] = 3;
        let cleanup = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 3,
            high_throughput: true,
            segment_count: 1,
            segment_passes: cleanup_passes,
            segment_lengths: cleanup_lengths,
        };
        decode_code_block_contribution_body(&mut decoder, &cleanup, &[0x00, 0x63, 0x00], &limits)
            .unwrap();
        assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), -3);

        let mut refinement_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let refinement_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        refinement_passes[0] = 1;
        let significance = CodeBlockContribution {
            first_inclusion: false,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 0,
            high_throughput: true,
            segment_count: 1,
            segment_passes: refinement_passes,
            segment_lengths: refinement_lengths,
        };
        decode_code_block_contribution_body(&mut decoder, &significance, &[], &limits).unwrap();
        assert_eq!(decoder.next_pass(), 2);

        let mut magnitude_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        magnitude_lengths[0] = 1;
        let magnitude = CodeBlockContribution {
            data_length: 1,
            segment_lengths: magnitude_lengths,
            ..significance
        };
        decode_code_block_contribution_body(&mut decoder, &magnitude, &[0x01], &limits).unwrap();
        assert_eq!(decoder.next_pass(), 3);
        assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), -3);
        assert_eq!(decoder.decoded_segments(), 3);
        assert_eq!(decoder.decoded_bytes(), 4);
    }

    #[test]
    fn routes_placeholder_passes_before_first_ht_cleanup() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let mut decoder =
            Tier1CodeBlockDecoder::new(1, 1, 3, SubbandOrientation::Ll, style).unwrap();

        let mut placeholder_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let placeholder_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        placeholder_passes[0] = 1;
        placeholder_passes[1] = 2;
        let placeholders = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 3,
            data_length: 0,
            high_throughput: true,
            segment_count: 2,
            segment_passes: placeholder_passes,
            segment_lengths: placeholder_lengths,
        };
        decode_code_block_contribution_body(&mut decoder, &placeholders, &[], &limits).unwrap();
        assert_eq!(decoder.next_pass(), 3);
        assert_eq!(decoder.plane().signed_coefficient(0, 0).unwrap(), 0);

        let mut cleanup_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut cleanup_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        cleanup_passes[0] = 1;
        cleanup_lengths[0] = 4;
        let cleanup = CodeBlockContribution {
            first_inclusion: false,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 4,
            high_throughput: true,
            segment_count: 1,
            segment_passes: cleanup_passes,
            segment_lengths: cleanup_lengths,
        };
        decode_code_block_contribution_body(
            &mut decoder,
            &cleanup,
            &[0xfe, 0x00, 0x63, 0x00],
            &limits,
        )
        .unwrap();
        assert_eq!(decoder.next_pass(), 4);
        assert_ne!(decoder.plane().signed_coefficient(0, 0).unwrap(), 0);
    }

    #[test]
    fn enforces_singleht_and_routes_multiple_ht_sets_transactionally() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let cleanup_bytes = [0xfe, 0x00, 0x63, 0x00];
        let mut set_passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        let mut set_lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
        set_passes[0] = 1;
        set_lengths[0] = 4;
        set_passes[1] = 2;
        let first_set = CodeBlockContribution {
            first_inclusion: true,
            zero_bitplanes: 0,
            new_passes: 3,
            data_length: 4,
            high_throughput: true,
            segment_count: 2,
            segment_passes: set_passes,
            segment_lengths: set_lengths,
        };
        let cleanup_only = CodeBlockContribution {
            new_passes: 1,
            segment_count: 1,
            ..first_set
        };
        let second_set = CodeBlockContribution {
            first_inclusion: false,
            zero_bitplanes: 0,
            new_passes: 1,
            data_length: 4,
            high_throughput: true,
            segment_count: 1,
            segment_passes: {
                let mut passes = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
                passes[0] = 1;
                passes
            },
            segment_lengths: {
                let mut lengths = [0; MAX_CODE_BLOCK_SEGMENTS_PER_CONTRIBUTION];
                lengths[0] = 4;
                lengths
            },
        };

        let mut single =
            Tier1CodeBlockDecoder::new(1, 1, 3, SubbandOrientation::Ll, style).unwrap();
        let mut cleanup_reference =
            Tier1CodeBlockDecoder::new(1, 1, 3, SubbandOrientation::Ll, style).unwrap();
        decode_code_block_contribution_body(
            &mut cleanup_reference,
            &cleanup_only,
            &cleanup_bytes,
            &limits,
        )
        .unwrap();
        decode_code_block_contribution_body(&mut single, &first_set, &cleanup_bytes, &limits)
            .unwrap();
        assert_eq!(
            single.plane().signed_coefficient(0, 0).unwrap(),
            cleanup_reference.plane().signed_coefficient(0, 0).unwrap()
        );
        let before_pass = single.next_pass();
        let before_value = single.plane().signed_coefficient(0, 0).unwrap();
        assert!(
            decode_code_block_contribution_body(&mut single, &second_set, &cleanup_bytes, &limits,)
                .is_err()
        );
        assert_eq!(single.next_pass(), before_pass);
        assert_eq!(
            single.plane().signed_coefficient(0, 0).unwrap(),
            before_value
        );

        let mut multiple =
            Tier1CodeBlockDecoder::new(1, 1, 3, SubbandOrientation::Ll, style).unwrap();
        multiple.set_multiple_ht_sets_allowed(true);
        decode_code_block_contribution_body(&mut multiple, &first_set, &cleanup_bytes, &limits)
            .unwrap();
        decode_code_block_contribution_body(&mut multiple, &second_set, &cleanup_bytes, &limits)
            .unwrap();
        assert_eq!(multiple.next_pass(), 4);
        assert_eq!(multiple.decoded_segments(), 3);
    }

    #[test]
    fn executes_retained_multi_code_block_packet_and_advances_by_prefix() {
        let limits = ResourceLimits::default();
        let mut data = two_block_first_layer_header();
        data.extend_from_slice(&[0x01, 0xae, 0x01, 0xae]);
        data.push(0);
        let tile_part = TilePartPayload {
            tile_index: 0,
            tile_part_index: 0,
            tile_part_count: 1,
            coding_style: coding_style(false, false),
            quantization: Quantization::reversible_default(coding_style(false, false)),
            component_coding_styles: component_styles(coding_style(false, false)).0,
            component_quantizations: component_styles(coding_style(false, false)).1,
            roi_shifts: crate::codestream::RoiShifts::none(),
            progression_changes: ProgressionChanges::none(),
            multiple_component_transform: None,
            part2_extensions: crate::codestream::Part2Extensions::default(),
            packet_lengths: crate::codestream::PacketLengths::none(),
            packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
            has_functional_header_marker: false,
            has_roi_marker: false,
            data: &data,
        };
        let mut states = [CodeBlockPacketState::new(), CodeBlockPacketState::new()];
        let mut inclusion = TagTree::new(2, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(2, 1, &limits).unwrap();
        let contexts = [
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 1,
                tier1_style: Tier1CodingStyle::default(),
            },
            CodeBlockPacketContext {
                leaf_x: 1,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 1,
                tier1_style: Tier1CodingStyle::default(),
            },
        ];
        let first = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let second = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let mut decoders = [first, second];
        let decoded = decode_multi_code_block_packet(
            &tile_part,
            0,
            PacketCodeBlockExecution {
                states: &mut states,
                inclusion_tree: &mut inclusion,
                zero_bitplane_tree: &mut zero_bitplanes,
                contexts: &contexts,
                decoders: &mut decoders,
            },
            &limits,
        )
        .unwrap();
        assert_eq!(
            decoded,
            PacketBodyDecode {
                present: true,
                code_blocks: 2,
                contributions: 2,
                segments: 2,
                passes: 2,
                header_bytes: 3,
                body_bytes: 4,
                bytes_consumed: 7,
            }
        );
        assert!(states.iter().all(CodeBlockPacketState::included));
        assert!(decoders.iter().all(Tier1CodeBlockDecoder::is_complete));

        let absent = decode_multi_code_block_packet(
            &tile_part,
            usize::try_from(decoded.bytes_consumed).unwrap(),
            PacketCodeBlockExecution {
                states: &mut states,
                inclusion_tree: &mut inclusion,
                zero_bitplane_tree: &mut zero_bitplanes,
                contexts: &contexts,
                decoders: &mut decoders,
            },
            &limits,
        )
        .unwrap();
        assert_eq!(absent.bytes_consumed, 1);
        assert!(!absent.present);
        assert_eq!(absent.contributions, 0);
    }

    #[test]
    fn empty_resolution_precinct_consumes_either_packet_present_flag() {
        let limits = ResourceLimits::default();
        for header in [0_u8, 0x80] {
            let data = [header];
            let style = coding_style(false, false);
            let tile = TilePartPayload {
                tile_index: 0,
                tile_part_index: 0,
                tile_part_count: 1,
                coding_style: style,
                quantization: Quantization::reversible_default(style),
                component_coding_styles: component_styles(style).0,
                component_quantizations: component_styles(style).1,
                roi_shifts: crate::codestream::RoiShifts::none(),
                progression_changes: ProgressionChanges::none(),
                multiple_component_transform: None,
                part2_extensions: crate::codestream::Part2Extensions::default(),
                packet_lengths: crate::codestream::PacketLengths::none(),
                packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
                has_functional_header_marker: false,
                has_roi_marker: false,
                data: &data,
            };
            let mut precinct = PacketPrecinctDecoder::new_subbands(
                0,
                1,
                0,
                Tier1CodingStyle::default(),
                Vec::new(),
                &limits,
            )
            .unwrap();
            let (decoded, consumed, packed) =
                decode_precinct_packet(&tile, 0, &[], 0, Some(1), 0, &mut precinct, true, &limits)
                    .unwrap();
            assert_eq!(decoded.present, header != 0);
            assert_eq!(decoded.code_blocks, 0);
            assert_eq!(decoded.passes, 0);
            assert_eq!(consumed, 1);
            assert_eq!(packed, 0);
            assert!(
                decode_precinct_packet(&tile, 0, &[], 0, Some(2), 0, &mut precinct, true, &limits,)
                    .is_err()
            );
        }
    }

    #[test]
    fn validates_packet_markers_and_preflights_all_bodies() {
        let limits = ResourceLimits::default();
        let marker_data = [0xff, 0x91, 0, 4, 0x12, 0x34, 0, 0xff, 0x92];
        let marked_tile = TilePartPayload {
            tile_index: 0,
            tile_part_index: 0,
            tile_part_count: 1,
            coding_style: coding_style(true, true),
            quantization: Quantization::reversible_default(coding_style(true, true)),
            component_coding_styles: component_styles(coding_style(true, true)).0,
            component_quantizations: component_styles(coding_style(true, true)).1,
            roi_shifts: crate::codestream::RoiShifts::none(),
            progression_changes: ProgressionChanges::none(),
            multiple_component_transform: None,
            part2_extensions: crate::codestream::Part2Extensions::default(),
            packet_lengths: crate::codestream::PacketLengths::none(),
            packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
            has_functional_header_marker: false,
            has_roi_marker: false,
            data: &marker_data,
        };
        let mut states = [CodeBlockPacketState::new()];
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let contexts = [CodeBlockPacketContext {
            leaf_x: 0,
            leaf_y: 0,
            layer: 0,
            maximum_zero_bitplanes: 1,
            tier1_style: Tier1CodingStyle::default(),
        }];
        let decoder = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let mut decoders = [decoder];
        let empty = decode_multi_code_block_packet(
            &marked_tile,
            0,
            PacketCodeBlockExecution {
                states: &mut states,
                inclusion_tree: &mut inclusion,
                zero_bitplane_tree: &mut zero_bitplanes,
                contexts: &contexts,
                decoders: &mut decoders,
            },
            &limits,
        )
        .unwrap();
        assert_eq!(empty.bytes_consumed, 9);
        assert_eq!(empty.header_bytes, 1);
        assert!(!empty.present);

        let mut truncated = two_block_first_layer_header();
        truncated.extend_from_slice(&[0x01, 0xae]);
        let truncated_tile = TilePartPayload {
            tile_index: 0,
            tile_part_index: 0,
            tile_part_count: 1,
            coding_style: coding_style(false, false),
            quantization: Quantization::reversible_default(coding_style(false, false)),
            component_coding_styles: component_styles(coding_style(false, false)).0,
            component_quantizations: component_styles(coding_style(false, false)).1,
            roi_shifts: crate::codestream::RoiShifts::none(),
            progression_changes: ProgressionChanges::none(),
            multiple_component_transform: None,
            part2_extensions: crate::codestream::Part2Extensions::default(),
            packet_lengths: crate::codestream::PacketLengths::none(),
            packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
            has_functional_header_marker: false,
            has_roi_marker: false,
            data: &truncated,
        };
        let mut states = [CodeBlockPacketState::new(), CodeBlockPacketState::new()];
        let mut inclusion = TagTree::new(2, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(2, 1, &limits).unwrap();
        let contexts = [
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 1,
                tier1_style: Tier1CodingStyle::default(),
            },
            CodeBlockPacketContext {
                leaf_x: 1,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 1,
                tier1_style: Tier1CodingStyle::default(),
            },
        ];
        let first = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let second = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let mut decoders = [first, second];
        assert!(matches!(
            decode_multi_code_block_packet(
                &truncated_tile,
                0,
                PacketCodeBlockExecution {
                    states: &mut states,
                    inclusion_tree: &mut inclusion,
                    zero_bitplane_tree: &mut zero_bitplanes,
                    contexts: &contexts,
                    decoders: &mut decoders,
                },
                &limits,
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::Truncated,
                ..
            })
        ));
        assert!(decoders.iter().all(|decoder| decoder.next_pass() == 0));

        let limits = limits.with_maximum_code_blocks(1).unwrap();
        assert!(matches!(
            decode_multi_code_block_packet(
                &truncated_tile,
                0,
                PacketCodeBlockExecution {
                    states: &mut states,
                    inclusion_tree: &mut inclusion,
                    zero_bitplane_tree: &mut zero_bitplanes,
                    contexts: &contexts,
                    decoders: &mut decoders,
                },
                &limits,
            ),
            Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlocks,
                ..
            })
        ));
    }

    #[test]
    fn traverses_retained_tile_parts_and_preserves_precinct_state() {
        let limits = ResourceLimits::default();
        let mut first_data = pack_packet_bits(&[
            true,  // non-empty packet
            true,  // first inclusion
            true,  // zero bitplanes
            false, // one pass
            false, // no Lblock increment
            true, false, true, // five body bytes
        ]);
        first_data.extend_from_slice(&[0; 5]);
        let second_data = [0];
        let style = coding_style(false, false);
        let tile_parts = [
            TilePartPayload {
                tile_index: 3,
                tile_part_index: 0,
                tile_part_count: 2,
                coding_style: style,
                quantization: Quantization::reversible_default(style),
                component_coding_styles: component_styles(style).0,
                component_quantizations: component_styles(style).1,
                roi_shifts: crate::codestream::RoiShifts::none(),
                progression_changes: ProgressionChanges::none(),
                multiple_component_transform: None,
                part2_extensions: crate::codestream::Part2Extensions::default(),
                packet_lengths: crate::codestream::PacketLengths::none(),
                packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
                has_functional_header_marker: false,
                has_roi_marker: false,
                data: &first_data,
            },
            TilePartPayload {
                tile_index: 3,
                tile_part_index: 1,
                tile_part_count: 2,
                coding_style: style,
                quantization: Quantization::reversible_default(style),
                component_coding_styles: component_styles(style).0,
                component_quantizations: component_styles(style).1,
                roi_shifts: crate::codestream::RoiShifts::none(),
                progression_changes: ProgressionChanges::none(),
                multiple_component_transform: None,
                part2_extensions: crate::codestream::Part2Extensions::default(),
                packet_lengths: crate::codestream::PacketLengths::none(),
                packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
                has_functional_header_marker: false,
                has_roi_marker: false,
                data: &second_data,
            },
        ];
        let grid = PacketGrid::new(2, 1, 1, vec![1], &limits).unwrap();
        let decoder = Tier1CodeBlockDecoder::new(
            1,
            1,
            1,
            crate::tier1::SubbandOrientation::Ll,
            Tier1CodingStyle::default(),
        )
        .unwrap();
        let precinct = PacketPrecinctDecoder::new(
            PacketPrecinctGeometry {
                component: 0,
                resolution: 0,
                precinct: 0,
                code_blocks_across: 1,
                code_blocks_down: 1,
                maximum_zero_bitplanes: 1,
            },
            Tier1CodingStyle::default(),
            vec![decoder],
            &limits,
        )
        .unwrap();
        let mut precincts = [precinct];
        let decoded =
            decode_progression_tile_packets(&tile_parts, &grid, &mut precincts, &limits).unwrap();
        assert_eq!(
            decoded,
            TilePacketDecode {
                tile_index: 3,
                tile_parts: 2,
                packets: 2,
                present_packets: 1,
                contributions: 1,
                segments: 1,
                passes: 1,
                packet_header_bytes: 2,
                packet_body_bytes: 5,
                payload_bytes: 7,
            }
        );
        assert_eq!(precincts[0].coordinate(), (0, 0, 0));
        assert!(precincts[0].states()[0].included());
        assert_eq!(precincts[0].states()[0].total_passes(), 1);
        assert!(precincts[0].decoders()[0].is_complete());
    }

    #[test]
    fn progression_traversal_requires_exact_payload_and_tile_part_sequence() {
        let limits = ResourceLimits::default();
        let style = coding_style(false, false);
        let grid = PacketGrid::new(2, 1, 1, vec![1], &limits).unwrap();
        let make_precinct = || {
            let decoder = Tier1CodeBlockDecoder::new(
                1,
                1,
                1,
                crate::tier1::SubbandOrientation::Ll,
                Tier1CodingStyle::default(),
            )
            .unwrap();
            PacketPrecinctDecoder::new(
                PacketPrecinctGeometry {
                    component: 0,
                    resolution: 0,
                    precinct: 0,
                    code_blocks_across: 1,
                    code_blocks_down: 1,
                    maximum_zero_bitplanes: 1,
                },
                Tier1CodingStyle::default(),
                vec![decoder],
                &limits,
            )
            .unwrap()
        };

        let trailing_data = [0, 0, 0];
        let trailing_part = [TilePartPayload {
            tile_index: 0,
            tile_part_index: 0,
            tile_part_count: 1,
            coding_style: style,
            quantization: Quantization::reversible_default(style),
            component_coding_styles: component_styles(style).0,
            component_quantizations: component_styles(style).1,
            roi_shifts: crate::codestream::RoiShifts::none(),
            progression_changes: ProgressionChanges::none(),
            multiple_component_transform: None,
            part2_extensions: crate::codestream::Part2Extensions::default(),
            packet_lengths: crate::codestream::PacketLengths::none(),
            packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
            has_functional_header_marker: false,
            has_roi_marker: false,
            data: &trailing_data,
        }];
        assert!(matches!(
            decode_progression_tile_packets(&trailing_part, &grid, &mut [make_precinct()], &limits,),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::TrailingData,
                ..
            })
        ));

        let truncated_data = [0];
        let truncated_part = [TilePartPayload {
            tile_index: 0,
            tile_part_index: 0,
            tile_part_count: 1,
            coding_style: style,
            quantization: Quantization::reversible_default(style),
            component_coding_styles: component_styles(style).0,
            component_quantizations: component_styles(style).1,
            roi_shifts: crate::codestream::RoiShifts::none(),
            progression_changes: ProgressionChanges::none(),
            multiple_component_transform: None,
            part2_extensions: crate::codestream::Part2Extensions::default(),
            packet_lengths: crate::codestream::PacketLengths::none(),
            packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
            has_functional_header_marker: false,
            has_roi_marker: false,
            data: &truncated_data,
        }];
        assert!(matches!(
            decode_progression_tile_packets(
                &truncated_part,
                &grid,
                &mut [make_precinct()],
                &limits,
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::Truncated,
                ..
            })
        ));

        let empty = [];
        let invalid_parts = [
            TilePartPayload {
                tile_index: 0,
                tile_part_index: 0,
                tile_part_count: 2,
                coding_style: style,
                quantization: Quantization::reversible_default(style),
                component_coding_styles: component_styles(style).0,
                component_quantizations: component_styles(style).1,
                roi_shifts: crate::codestream::RoiShifts::none(),
                progression_changes: ProgressionChanges::none(),
                multiple_component_transform: None,
                part2_extensions: crate::codestream::Part2Extensions::default(),
                packet_lengths: crate::codestream::PacketLengths::none(),
                packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
                has_functional_header_marker: false,
                has_roi_marker: false,
                data: &empty,
            },
            TilePartPayload {
                tile_index: 0,
                tile_part_index: 2,
                tile_part_count: 2,
                coding_style: style,
                quantization: Quantization::reversible_default(style),
                component_coding_styles: component_styles(style).0,
                component_quantizations: component_styles(style).1,
                roi_shifts: crate::codestream::RoiShifts::none(),
                progression_changes: ProgressionChanges::none(),
                multiple_component_transform: None,
                part2_extensions: crate::codestream::Part2Extensions::default(),
                packet_lengths: crate::codestream::PacketLengths::none(),
                packed_packet_headers: crate::codestream::PackedPacketHeaders::none(),
                has_functional_header_marker: false,
                has_roi_marker: false,
                data: &empty,
            },
        ];
        assert!(matches!(
            decode_progression_tile_packets(&invalid_parts, &grid, &mut [make_precinct()], &limits,),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTilePart,
                ..
            })
        ));
    }
}
