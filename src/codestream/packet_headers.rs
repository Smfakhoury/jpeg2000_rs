//! Packed packet headers, packet-length decoding and resource accounting.

use super::reader::read_u32;
use super::{
    DecodeError, MalformedKind, PLT, PPT, PackedHeaderFrame, PackedPacketHeaderSource,
    PackedPacketHeaders, PacketLengthIter, PacketLengthSource, PacketLengths, Parser, Resource,
    ResourceLimits, Segment,
};
#[cfg(verus_keep_ghost)]
use super::{
    packed_header_frames_outcome, packed_header_frames_view, packed_header_reassembly_outcome,
    packed_packet_headers_present, packed_packet_headers_view, packet_length_groups_view,
    packet_length_iter_view, packet_lengths_view, parser_view, segment_view,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == (DecodeError::MalformedInput { offset, kind }),
))]
fn packet_input_error(offset: u64, kind: MalformedKind) -> DecodeError {
    DecodeError::MalformedInput { offset, kind }
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_input_limit(resource, limit, actual),
))]
fn packet_input_limit(resource: Resource, limit: u64, actual: u64) -> DecodeError {
    DecodeError::LimitExceeded {
        resource,
        limit,
        actual,
    }
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures packed_packet_headers_view(&result) == (crate::specs::Part1PackedHeaders {
        source: crate::specs::Part1PackedHeaderSource::None, length: 0, fragments: 0 }),
        !packed_packet_headers_present(&result),
))]
pub(crate) const fn packed_packet_headers_none<'a>() -> PackedPacketHeaders<'a> {
    PackedPacketHeaders {
        source: PackedPacketHeaderSource::None,
        length: 0,
        fragments: 0,
    }
}
impl PackedPacketHeaders<'_> {
    pub const fn none() -> Self {
        packed_packet_headers_none()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == packed_packet_headers_present(self),
            result == !matches!(packed_packet_headers_view(self).source, crate::specs::Part1PackedHeaderSource::None),
    ))]
    pub const fn is_present(&self) -> bool {
        !matches!(self.source, PackedPacketHeaderSource::None)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packed_packet_headers_view(self).length,))]
    pub const fn len(&self) -> usize {
        self.length
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (packed_packet_headers_view(self).length == 0),))]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packed_packet_headers_view(self).fragments,))]
    pub const fn fragment_count(&self) -> u16 {
        self.fragments
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packed_header_reassembly_outcome(result) == crate::specs::part1_packed_header_reassembly(
            packed_packet_headers_view(self), crate::limits::resource_packet_header_byte_limit(limits)),
    ))]
    pub(crate) fn reassemble(&self, limits: &ResourceLimits) -> Result<Vec<u8>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packed_header_reassembly); }
        if self.length as u64 > limits.maximum_packet_header_bytes() {
            return Err(packet_input_limit(
                Resource::PacketHeaderBytes,
                limits.maximum_packet_header_bytes(),
                self.length as u64,
            ));
        }
        let PackedPacketHeaderSource::MarkerSegments {
            header,
            marker,
            logical_start,
        } = self.source
        else {
            return Ok(Vec::new());
        };
        let logical_end = match logical_start.checked_add(self.length) {
            Some(end) => end,
            None => return Err(packet_input_error(0, MalformedKind::ArithmeticOverflow)),
        };
        let mut output = Vec::with_capacity(self.length);
        let mut position = 0_usize;
        let mut logical_position = 0_usize;
        let mut remaining_segments = header.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= header.len(), logical_position <= position,
                    remaining_segments <= header.len(),
                    logical_end as int == logical_start as int + self.length as int,
                    output@.len() == (if logical_position <= logical_start { 0int }
                        else if logical_position < logical_end { logical_position as int - logical_start as int }
                        else { self.length as int }),
                    crate::specs::part1_packed_header_reassembly(packed_packet_headers_view(self), crate::limits::resource_packet_header_byte_limit(limits))
                        == crate::specs::part1_packed_header_scan(header@, marker, logical_start, logical_end, self.length,
                            crate::specs::Part1PackedHeaderScan { position, logical_position, remaining_segments, output: output@ }),
                decreases remaining_segments,
            )
        )]
        while position < header.len() {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = crate::specs::Part1PackedHeaderScan {
            position, logical_position, remaining_segments, output: output@ }; }
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_packed_header_scan); }
            if remaining_segments == 0 {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_segments -= 1;
            if header.len().saturating_sub(position) < 4 {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidPackedPacketHeader,
                ));
            }
            let current_marker =
                crate::refinement::marker_value(header[position], header[position + 1]);
            let segment_length = usize::from(crate::refinement::marker_value(
                header[position + 2],
                header[position + 3],
            ));
            if segment_length < 2 {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let after_marker = match position.checked_add(2) {
                Some(value) => value,
                None => {
                    return Err(packet_input_error(
                        position as u64,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            let end = match after_marker.checked_add(segment_length) {
                Some(end) => end,
                None => {
                    return Err(packet_input_error(
                        position as u64,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            if end > header.len() {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            if current_marker == marker {
                let payload_start = match position.checked_add(5) {
                    Some(start) => start,
                    None => {
                        return Err(packet_input_error(
                            position as u64,
                            MalformedKind::ArithmeticOverflow,
                        ));
                    }
                };
                if payload_start > end {
                    return Err(packet_input_error(
                        position as u64,
                        MalformedKind::InvalidMarkerLength,
                    ));
                }
                let fragment = &header[payload_start..end];
                let fragment_end = match logical_position.checked_add(fragment.len()) {
                    Some(end) => end,
                    None => {
                        return Err(packet_input_error(
                            position as u64,
                            MalformedKind::ArithmeticOverflow,
                        ));
                    }
                };
                let copy_start = logical_start.max(logical_position);
                let copy_end = logical_end.min(fragment_end);
                if copy_start < copy_end {
                    let local_start = copy_start - logical_position;
                    let local_end = copy_end - logical_position;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(fragment@.subrange(local_start as int, local_end as int)
                            =~= header@.subrange(payload_start as int + local_start as int, payload_start as int + local_end as int));
                    }
                    output.extend_from_slice(&fragment[local_start..local_end]);
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(output@ =~= before.output + header@.subrange(
                            payload_start as int + local_start as int, payload_start as int + local_end as int));
                    }
                }
                logical_position = fragment_end;
            }
            position = end;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(crate::specs::part1_packed_header_scan(header@, marker, logical_start, logical_end, self.length, before)
                    == crate::specs::part1_packed_header_scan(header@, marker, logical_start, logical_end, self.length,
                        crate::specs::Part1PackedHeaderScan { position, logical_position, remaining_segments, output: output@ }));
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packed_header_scan); }
        if output.len() != self.length {
            return Err(packet_input_error(
                logical_position as u64,
                MalformedKind::InvalidPackedPacketHeader,
            ));
        }
        Ok(output)
    }
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures packet_lengths_view(&result) == (crate::specs::Part1PacketLengths {
        source: crate::specs::Part1PacketLengthSource::None, packet_count: 0, encoded_bytes: 0 }),
))]
pub(crate) const fn packet_lengths_none<'a>() -> PacketLengths<'a> {
    PacketLengths {
        source: PacketLengthSource::None,
        packet_count: 0,
        encoded_bytes: 0,
    }
}
impl PacketLengths<'_> {
    pub const fn none() -> Self {
        packet_lengths_none()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == !matches!(packet_lengths_view(self).source, crate::specs::Part1PacketLengthSource::None),
    ))]
    pub const fn is_present(&self) -> bool {
        !matches!(self.source, PacketLengthSource::None)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packet_lengths_view(self).packet_count,))]
    pub const fn packet_count(&self) -> u64 {
        self.packet_count
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packet_lengths_view(self).encoded_bytes,))]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packet_length_iter_view(&result) == crate::specs::part1_packet_lengths_iter(packet_lengths_view(self)),
    ))]
    pub(crate) const fn iter(&self) -> PacketLengthIter<'_> {
        PacketLengthIter {
            source: self.source,
            position: 0,
            segment_end: 0,
        }
    }
}
impl PacketLengthIter<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, packet_length_iter_view(final(self))) == crate::specs::part1_packet_length_next(packet_length_iter_view(old(self))),
            match result { Ok(Some(value)) => value > 0, _ => true },
            match result { Ok(Some(_)) => packet_length_iter_view(final(self)).position > packet_length_iter_view(old(self)).position, _ => true },
    ))]
    pub(crate) fn next_length(&mut self) -> Result<Option<u32>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = packet_length_iter_view(self); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_length_next); }
        match self.source {
            PacketLengthSource::None => Ok(None),
            PacketLengthSource::Plm(bytes) => {
                if self.position == bytes.len() {
                    return Ok(None);
                }
                let (length, next) = decode_packet_length(bytes, self.position, bytes.len())?;
                self.position = next;
                Ok(Some(length))
            }
            PacketLengthSource::Plt(header) =>
            {
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant initial == packet_length_iter_view(old(self)),
                            packet_length_iter_view(self).source == initial.source,
                            initial.source == crate::specs::Part1PacketLengthSource::Plt(header@),
                            initial.position <= self.position,
                            crate::specs::part1_packet_length_next(initial)
                                == crate::specs::part1_packet_length_plt(packet_length_iter_view(self)),
                        decreases header.len() as int - self.position as int,
                    )
                )]
                loop {
                    #[cfg(verus_keep_ghost)]
                    proof! { reveal(crate::specs::part1_packet_length_plt); }
                    let previous_position = self.position;
                    if self.position < self.segment_end {
                        let (length, next) =
                            decode_packet_length(header, self.position, self.segment_end)?;
                        self.position = next;
                        return Ok(Some(length));
                    }
                    if self.position == header.len() {
                        return Ok(None);
                    }
                    if header.len().saturating_sub(self.position) < 4 {
                        return Err(packet_input_error(
                            self.position as u64,
                            MalformedKind::InvalidPacketLength,
                        ));
                    }
                    let marker = crate::refinement::marker_value(
                        header[self.position],
                        header[self.position + 1],
                    );
                    let marker_length = usize::from(crate::refinement::marker_value(
                        header[self.position + 2],
                        header[self.position + 3],
                    ));
                    if marker_length < 2 {
                        return Err(packet_input_error(
                            self.position as u64,
                            MalformedKind::InvalidMarkerLength,
                        ));
                    }
                    let after_marker = match self.position.checked_add(2) {
                        Some(value) => value,
                        None => {
                            return Err(packet_input_error(
                                self.position as u64,
                                MalformedKind::ArithmeticOverflow,
                            ));
                        }
                    };
                    let end = match after_marker.checked_add(marker_length) {
                        Some(end) => end,
                        None => {
                            return Err(packet_input_error(
                                self.position as u64,
                                MalformedKind::ArithmeticOverflow,
                            ));
                        }
                    };
                    if end > header.len() {
                        return Err(packet_input_error(
                            self.position as u64,
                            MalformedKind::InvalidMarkerLength,
                        ));
                    }
                    self.position = end;
                    if marker == PLT {
                        let before_index = match self.position.checked_sub(marker_length) {
                            Some(value) => value,
                            None => {
                                return Err(packet_input_error(
                                    self.position as u64,
                                    MalformedKind::ArithmeticOverflow,
                                ));
                            }
                        };
                        let data_start = match before_index.checked_add(3) {
                            Some(start) => start,
                            None => {
                                return Err(packet_input_error(
                                    self.position as u64,
                                    MalformedKind::ArithmeticOverflow,
                                ));
                            }
                        };
                        if data_start > end {
                            return Err(packet_input_error(
                                self.position as u64,
                                MalformedKind::InvalidMarkerLength,
                            ));
                        }
                        self.position = data_start;
                        self.segment_end = end;
                    }
                    if self.position <= previous_position || self.position > header.len() {
                        return Err(packet_input_error(
                            previous_position as u64,
                            MalformedKind::InvalidPacketLength,
                        ));
                    }
                }
            }
        }
    }
}
impl<'a> Parser<'a> {
    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), *final(next_index), packet_length_groups_view(final(groups)@))
            == crate::specs::part1_parse_plm(parser_view(old(self)), segment_view(segment),
                *old(next_index), packet_length_groups_view(old(groups)@)),
    ))]
    pub(super) fn parse_plm(
        &mut self,
        segment: Segment<'a>,
        next_index: &mut u16,
        groups: &mut Vec<PacketLengths<'a>>,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost initial_index = *next_index;
            let ghost initial_groups = packet_length_groups_view(groups@);
            let ghost goal = crate::specs::part1_parse_plm(input, segment_view(segment), initial_index, initial_groups);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_plm); }
        if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index || index > u16::from(u8::MAX) {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerOrder));
        }
        *next_index += 1;

        let mut position = 1_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                    input == parser_view(old(self)),
                    initial_index == *old(next_index),
                    initial_groups == packet_length_groups_view(old(groups)@),
                    goal == crate::specs::part1_parse_plm(input, segment_view(segment), initial_index, initial_groups),
                    goal == crate::specs::part1_plm_groups_from(parser_view(self), segment_view(segment),
                        *next_index, position as nat, packet_length_groups_view(groups@)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_plm_groups_from, 1); }
            if segment.payload.len() - position < 4 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ])) {
                Ok(value) => value,
                Err(_) => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            position += 4;
            let end = match position.checked_add(encoded_length) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            if end > segment.payload.len() {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let encoded = &segment.payload[position..end];
            let (packet_count, _) = self.validate_packet_lengths(encoded, segment.offset)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = packet_length_groups_view(groups@); }
            groups.push(PacketLengths {
                source: PacketLengthSource::Plm(encoded),
                packet_count,
                encoded_bytes: encoded.len() as u64,
            });
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(packet_length_groups_view(groups@) =~= before.push(crate::specs::Part1PacketLengths {
                    source: crate::specs::Part1PacketLengthSource::Plm(encoded@), packet_count,
                    encoded_bytes: encoded.len() as u64,
                }));
            }
            if groups.len() as u64 > u64::from(self.limits.maximum_tile_parts()) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TileParts,
                    limit: u64::from(self.limits.maximum_tile_parts()),
                    actual: groups.len() as u64,
                });
            }
            position = end;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_plm_groups_from, 1); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), *final(next_index), *final(fragments), final(bytes)@)
            == crate::specs::part1_parse_packed_fragment(parser_view(old(self)), segment_view(segment), expected_marker,
                *old(next_index), *old(fragments), old(bytes)@),
    ))]
    pub(super) fn parse_packed_fragment(
        &mut self,
        segment: Segment<'a>,
        expected_marker: u16,
        next_index: &mut u16,
        fragments: &mut u16,
        bytes: &mut Vec<u8>,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_packed_fragment); }
        if segment.marker != expected_marker || segment.payload.is_empty() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index || index > u16::from(u8::MAX) {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerOrder));
        }
        *next_index += 1;
        *fragments = match fragments.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let fragment = &segment.payload[1..];
        let new_length = match bytes.len().checked_add(fragment.len()) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let total_header_bytes = match self.packet_length_bytes.checked_add(fragment.len() as u64) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if total_header_bytes > self.limits.maximum_packet_header_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                limit: self.limits.maximum_packet_header_bytes(),
                actual: total_header_bytes,
            });
        }
        self.packet_length_bytes = total_header_bytes;
        self.charge_work(fragment.len() as u64)?;
        bytes.reserve(new_length - bytes.len());
        bytes.extend_from_slice(fragment);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packed_header_frames_outcome(result) == crate::specs::part1_parse_ppm_frames(
            bytes@, fragments, self.limits),
    ))]
    pub(super) fn parse_ppm_frames(
        &self,
        bytes: &[u8],
        fragments: u16,
    ) -> Result<Vec<PackedHeaderFrame>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost goal = crate::specs::part1_parse_ppm_frames(bytes@, fragments, self.limits); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_ppm_frames); }
        if fragments == 0 {
            let empty = Vec::new();
            #[cfg(verus_keep_ghost)]
            proof! { assert(packed_header_frames_view(empty@) =~= Seq::empty()); }
            return Ok(empty);
        }
        let mut frames = Vec::new();
        #[cfg(verus_keep_ghost)]
        proof! { assert(packed_header_frames_view(frames@) =~= Seq::empty()); }
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= bytes.len(),
                    goal == crate::specs::part1_parse_ppm_frames(bytes@, fragments, self.limits),
                    goal == crate::specs::part1_ppm_frames_from(bytes@,
                        crate::limits::resource_tile_part_limit(self.limits), position as nat, packed_header_frames_view(frames@)),
                decreases bytes.len() - position,
            )
        )]
        while position < bytes.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_ppm_frames_from, 1); }
            if bytes.len() - position < 4 {
                return Err(self.malformed_at(position, MalformedKind::InvalidPackedPacketHeader));
            }
            let length = match usize::try_from(read_u32([
                bytes[position],
                bytes[position + 1],
                bytes[position + 2],
                bytes[position + 3],
            ])) {
                Ok(value) => value,
                Err(_) => {
                    return Err(self.malformed_at(position, MalformedKind::ArithmeticOverflow));
                }
            };
            position += 4;
            let start = position;
            position = match position.checked_add(length) {
                Some(value) => value,
                None => return Err(self.malformed_at(start, MalformedKind::ArithmeticOverflow)),
            };
            if position > bytes.len() {
                return Err(self.malformed_at(start, MalformedKind::InvalidPackedPacketHeader));
            }
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = packed_header_frames_view(frames@); }
            frames.push(PackedHeaderFrame { start, length });
            #[cfg(verus_keep_ghost)]
            proof! { assert(packed_header_frames_view(frames@) =~= before.push((start, length))); }
            if frames.len() as u64 > u64::from(self.limits.maximum_tile_parts()) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TileParts,
                    limit: u64::from(self.limits.maximum_tile_parts()),
                    actual: frames.len() as u64,
                });
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_ppm_frames_from, 1); }
        Ok(frames)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self))) == crate::specs::part1_validate_packet_lengths(
            parser_view(old(self)), encoded@, marker_offset),
    ))]
    pub(super) fn validate_packet_lengths(
        &mut self,
        encoded: &[u8],
        marker_offset: usize,
    ) -> Result<(u64, u64), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost goal = crate::specs::part1_validate_packet_lengths(input, encoded@, marker_offset);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_validate_packet_lengths); }
        let encoded_bytes = encoded.len() as u64;
        self.packet_length_bytes = match self.packet_length_bytes.checked_add(encoded_bytes) {
            Some(value) => value,
            None => return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow)),
        };
        if self.packet_length_bytes > self.limits.maximum_packet_header_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                limit: self.limits.maximum_packet_header_bytes(),
                actual: self.packet_length_bytes,
            });
        }
        self.charge_work(encoded_bytes)?;
        let mut position = 0_usize;
        let mut count = 0_u64;
        let mut total = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= encoded.len(),
                    input == parser_view(old(self)),
                    goal == crate::specs::part1_validate_packet_lengths(input, encoded@, marker_offset),
                    goal == crate::specs::part1_packet_length_admission_from(parser_view(self), encoded@,
                        marker_offset, position, count, total),
                decreases encoded.len() - position,
            )
        )]
        while position < encoded.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_packet_length_admission_from, 1); }
            let (length, next) = match decode_packet_length(encoded, position, encoded.len()) {
                Ok(value) => value,
                Err(_) => {
                    return Err(
                        self.malformed_at(marker_offset, MalformedKind::InvalidPacketLength)
                    );
                }
            };
            if next <= position || next > encoded.len() {
                return Err(self.malformed_at(marker_offset, MalformedKind::InvalidPacketLength));
            }
            count = match count.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow));
                }
            };
            total = match total.checked_add(u64::from(length)) {
                Some(value) => value,
                None => {
                    return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow));
                }
            };
            position = next;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_packet_length_admission_from, 1); }
        self.packet_lengths = match self.packet_lengths.checked_add(count) {
            Some(value) => value,
            None => return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow)),
        };
        if self.packet_lengths > self.limits.maximum_packets() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                limit: self.limits.maximum_packets(),
                actual: self.packet_lengths,
            });
        }
        Ok((count, total))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_packet_lengths(
        &self,
        tile_header_start: usize,
        tile_header_end: usize,
        main_packet_lengths: PacketLengths<'a>,
        plt_seen: bool,
        plt_packet_count: u64,
        plt_encoded_bytes: u64,
        plt_payload_bytes: u64,
        data_length: usize,
        packed_header_length: usize,
        tile_part_offset: usize,
    ) -> Result<PacketLengths<'a>, DecodeError> {
        let packet_lengths = if plt_seen {
            PacketLengths {
                source: PacketLengthSource::Plt(
                    self.input
                        .get(tile_header_start..tile_header_end)
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(tile_part_offset),
                                MalformedKind::InvalidTilePart,
                            )
                        })?,
                ),
                packet_count: plt_packet_count,
                encoded_bytes: plt_encoded_bytes,
            }
        } else {
            main_packet_lengths
        };
        if packet_lengths.is_present() {
            let declared_payload_bytes = if plt_seen {
                plt_payload_bytes
            } else {
                sum_packet_lengths(packet_lengths.source)?
            };
            let logical_payload_bytes = (data_length as u64)
                .checked_add(packed_header_length as u64)
                .ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(tile_part_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            if declared_payload_bytes != logical_payload_bytes {
                return Err(DecodeError::malformed(
                    self.offset(tile_part_offset),
                    MalformedKind::InvalidPacketLength,
                ));
            }
        }
        Ok(packet_lengths)
    }

    pub(super) fn finish_packed_packet_headers(
        &self,
        tile_header_start: usize,
        tile_header_end: usize,
        main: PackedPacketHeaders<'a>,
        ppt_fragments: u16,
        ppt_length: usize,
        tile_part_offset: usize,
    ) -> Result<PackedPacketHeaders<'a>, DecodeError> {
        if ppt_fragments == 0 {
            return Ok(main);
        }
        let header = self
            .input
            .get(tile_header_start..tile_header_end)
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(tile_part_offset),
                    MalformedKind::InvalidTilePart,
                )
            })?;
        Ok(PackedPacketHeaders {
            source: PackedPacketHeaderSource::MarkerSegments {
                header,
                marker: PPT,
                logical_start: 0,
            },
            length: ppt_length,
            fragments: ppt_fragments,
        })
    }
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_length_decode(encoded@, start, end),
        match result { Ok((value, next)) => value > 0 && start < next && next <= end, Err(_) => true },
))]
pub(super) fn decode_packet_length(
    encoded: &[u8],
    start: usize,
    end: usize,
) -> Result<(u32, usize), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_length_decode); }
    if start >= end || end > encoded.len() {
        return Err(packet_input_error(
            start as u64,
            MalformedKind::InvalidPacketLength,
        ));
    }
    let mut position = start;
    let mut value = 0_u32;
    let mut groups = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant start <= position, position <= end, end <= encoded.len(),
                crate::specs::part1_packet_length_decode(encoded@, start, end)
                    == crate::specs::part1_packet_length_groups(encoded@, start, end, position, value, groups),
            decreases end - position,
        )
    )]
    loop {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_length_groups); }
        if position >= end || position >= encoded.len() {
            return Err(packet_input_error(
                start as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        let byte = encoded[position];
        groups = match groups.checked_add(1) {
            Some(count) => count,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        if groups > 5 || (groups == 5 && value > 0x01ff_ffff) {
            return Err(packet_input_error(
                start as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        let prefix = match value.checked_mul(128) {
            Some(prefix) => prefix,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
        };
        value = match prefix.checked_add(u32::from(byte & 0x7f)) {
            Some(word) => word,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
        };
        position = match position.checked_add(1) {
            Some(next) => next,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        if byte & 0x80 == 0 {
            if value == 0 {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
            return Ok((value, position));
        }
    }
}
fn sum_packet_lengths(source: PacketLengthSource<'_>) -> Result<u64, DecodeError> {
    let bytes = match source {
        PacketLengthSource::Plm(bytes) => bytes,
        PacketLengthSource::None => return Ok(0),
        PacketLengthSource::Plt(_) => {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidPacketLength,
            ));
        }
    };
    let mut position = 0_usize;
    let mut total = 0_u64;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= bytes.len(),
            decreases bytes.len() - position,
        )
    )]
    while position < bytes.len() {
        let (length, next) = decode_packet_length(bytes, position, bytes.len())?;
        if next <= position || next > bytes.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        total = total
            .checked_add(u64::from(length))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        position = next;
    }
    Ok(total)
}
