//! Bounded JPIP message parsing, deterministic data-bin reassembly, and
//! automatic or caller-planned precinct quality reconstruction.
//!
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers. It reassembles classes 0 through 8 and can reconstruct a
//! bounded multi-tile packet/codestream views from explicit packet ranges or
//! SIZ/COD/COC-derived precinct mappings across every progression order.
//! It does not implement cache signaling, transport, request generation,
//! authentication, server behavior, or TLS.

use crate::{DecodeError, MalformedKind, Resource, ResourceLimits};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const MAX_VBAS_BYTES: usize = 10;
const SOC: [u8; 2] = [0xff, 0x4f];
const EOC: [u8; 2] = [0xff, 0xd9];

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq, Hash))]
#[non_exhaustive]
pub enum DatabinClass {
    Precinct,
    TileHeader,
    TileData,
    MainHeader,
    Metadata,
}

impl DatabinClass {
    const fn from_identifier(identifier: u64) -> Option<Self> {
        if !crate::refinement::jpip_class_identifier_valid(identifier) {
            return None;
        }
        match identifier {
            0 | 1 => Some(Self::Precinct),
            2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
            8 => Some(Self::Metadata),
            _ => None,
        }
    }

    pub const fn has_auxiliary(identifier: u64) -> bool {
        crate::refinement::jpip_auxiliary_present(identifier)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq, Hash))]
pub struct DatabinId {
    class_identifier: u64,
    class: DatabinClass,
    codestream: u64,
    in_class: u64,
}

impl DatabinId {
    pub fn new(class_identifier: u64, codestream: u64, in_class: u64) -> Result<Self, DecodeError> {
        let class = DatabinClass::from_identifier(class_identifier)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?;
        Ok(Self {
            class_identifier,
            class,
            codestream,
            in_class,
        })
    }

    pub const fn class_identifier(&self) -> u64 {
        self.class_identifier
    }

    pub const fn class(&self) -> DatabinClass {
        self.class
    }

    pub const fn codestream(&self) -> u64 {
        self.codestream
    }

    pub const fn in_class(&self) -> u64 {
        self.in_class
    }

    const fn same_as(&self, other: Self) -> bool {
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipHeaderContext {
    previous_class: Option<u64>,
    previous_codestream: Option<u64>,
}

impl JpipHeaderContext {
    pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }

    pub const fn previous_class(&self) -> Option<u64> {
        self.previous_class
    }

    pub const fn previous_codestream(&self) -> Option<u64> {
        self.previous_codestream
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipMessageHeader {
    databin: DatabinId,
    offset: u64,
    length: u64,
    last_byte: bool,
    auxiliary: Option<u64>,
    header_length: usize,
}

impl JpipMessageHeader {
    pub const fn databin(&self) -> DatabinId {
        self.databin
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn is_last_byte(&self) -> bool {
        self.last_byte
    }

    pub const fn final_length(&self) -> Option<u64> {
        if self.last_byte {
            Some(self.offset.saturating_add(self.length))
        } else {
            None
        }
    }

    pub const fn auxiliary(&self) -> Option<u64> {
        self.auxiliary
    }

    pub const fn header_length(&self) -> usize {
        self.header_length
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipEndOfResponse<'a> {
    reason_code: u8,
    body: &'a [u8],
    header_length: usize,
}

impl<'a> JpipEndOfResponse<'a> {
    pub const fn reason_code(&self) -> u8 {
        self.reason_code
    }

    pub const fn body(&self) -> &'a [u8] {
        self.body
    }

    pub const fn header_length(&self) -> usize {
        self.header_length
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum JpipMessage<'a> {
    Data {
        header: JpipMessageHeader,
        body: &'a [u8],
    },
    EndOfResponse(JpipEndOfResponse<'a>),
}

impl<'a> JpipMessage<'a> {
    pub const fn body(&self) -> &'a [u8] {
        match self {
            Self::Data { body, .. } => body,
            Self::EndOfResponse(end) => end.body,
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipMessageParser {
    context: JpipHeaderContext,
    limits: ResourceLimits,
}

impl JpipMessageParser {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            context: JpipHeaderContext::new(),
            limits,
        }
    }

    pub const fn context(&self) -> JpipHeaderContext {
        self.context
    }

    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<(JpipMessage<'a>, usize), DecodeError> {
        parse_jpip_message(input, &mut self.context, &self.limits)
    }
}

pub fn parse_jpip_message<'a>(
    input: &'a [u8],
    context: &mut JpipHeaderContext,
    limits: &ResourceLimits,
) -> Result<(JpipMessage<'a>, usize), DecodeError> {
    if input.is_empty() {
        return Err(DecodeError::malformed(0, MalformedKind::Truncated));
    }
    if input.len() as u64 > limits.maximum_input_bytes() {
        return Err(DecodeError::limit(
            Resource::InputBytes,
            limits.maximum_input_bytes(),
            input.len() as u64,
        ));
    }
    if input[0] == 0 {
        return parse_end_of_response(input);
    }

    let form = (input[0] >> 5) & 3;
    if !crate::refinement::jpip_header_form_valid(form) {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    }
    let last_byte = input[0] & 0x10 != 0;
    let mut position = 0_usize;
    let in_class = read_vbas(input, &mut position, Some(0x0f))?;

    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
    let codestream = if crate::refinement::jpip_header_has_explicit_codestream(form) {
        read_vbas(input, &mut position, None)?
    } else if let Some(previous) = context.previous_codestream {
        previous
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
    let databin = DatabinId::new(class_identifier, codestream, in_class)?;
    let offset = read_vbas(input, &mut position, None)?;
    let length = read_vbas(input, &mut position, None)?;
    let auxiliary = if DatabinClass::has_auxiliary(class_identifier) {
        Some(read_vbas(input, &mut position, None)?)
    } else {
        None
    };
    let end = offset.checked_add(length).ok_or_else(|| {
        DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
    })?;
    if end > limits.maximum_input_bytes() {
        return Err(DecodeError::limit(
            Resource::InputBytes,
            limits.maximum_input_bytes(),
            end,
        ));
    }
    let body_length = usize::try_from(length)
        .map_err(|_| DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow))?;
    let body_end = position.checked_add(body_length).ok_or_else(|| {
        DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
    })?;
    if body_end > input.len() {
        return Err(DecodeError::malformed(
            position as u64,
            MalformedKind::Truncated,
        ));
    }

    let header = JpipMessageHeader {
        databin,
        offset,
        length,
        last_byte,
        auxiliary,
        header_length: position,
    };
    context.previous_class = Some(class_identifier);
    context.previous_codestream = Some(codestream);
    Ok((
        JpipMessage::Data {
            header,
            body: &input[position..body_end],
        },
        body_end,
    ))
}

fn parse_end_of_response(input: &[u8]) -> Result<(JpipMessage<'_>, usize), DecodeError> {
    if input.len() < 2 {
        return Err(DecodeError::malformed(1, MalformedKind::Truncated));
    }
    let reason_code = input[1];
    let mut position = 2_usize;
    let length = read_vbas(input, &mut position, None)?;
    let body_length = usize::try_from(length)
        .map_err(|_| DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow))?;
    let body_end = position.checked_add(body_length).ok_or_else(|| {
        DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
    })?;
    if crate::refinement::jpip_eor_extent(position, body_length) != body_end as u128 {
        return Err(DecodeError::malformed(
            position as u64,
            MalformedKind::ArithmeticOverflow,
        ));
    }
    if body_end > input.len() {
        return Err(DecodeError::malformed(
            position as u64,
            MalformedKind::Truncated,
        ));
    }
    Ok((
        JpipMessage::EndOfResponse(JpipEndOfResponse {
            reason_code,
            body: &input[position..body_end],
            header_length: position,
        }),
        body_end,
    ))
}

fn read_vbas(
    input: &[u8],
    position: &mut usize,
    first_mask: Option<u8>,
) -> Result<u64, DecodeError> {
    let start = *position;
    let first = *input
        .get(*position)
        .ok_or_else(|| DecodeError::malformed(*position as u64, MalformedKind::Truncated))?;
    let mask = first_mask.unwrap_or(0x7f);
    let mut value = u64::from(first & mask);
    let mut byte = first;
    let mut count = 1_usize;
    *position = position
        .checked_add(1)
        .ok_or_else(|| DecodeError::malformed(start as u64, MalformedKind::ArithmeticOverflow))?;
    if first_mask.is_none()
        && !crate::refinement::jpip_vbas_first_is_canonical(
            u8::try_from(value).map_err(|_| {
                DecodeError::malformed(start as u64, MalformedKind::InvalidJpipVbas)
            })?,
            byte & 0x80 != 0,
        )
    {
        return Err(DecodeError::malformed(
            start as u64,
            MalformedKind::InvalidJpipVbas,
        ));
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant count <= MAX_VBAS_BYTES,
            decreases MAX_VBAS_BYTES - count,
        )
    )]
    while byte & 0x80 != 0 {
        if count >= MAX_VBAS_BYTES {
            return Err(DecodeError::malformed(
                start as u64,
                MalformedKind::InvalidJpipVbas,
            ));
        }
        byte = *input
            .get(*position)
            .ok_or_else(|| DecodeError::malformed(*position as u64, MalformedKind::Truncated))?;
        let group = byte & 0x7f;
        let stepped = value
            .checked_mul(128)
            .and_then(|shifted| shifted.checked_add(u64::from(group)))
            .ok_or_else(|| DecodeError::malformed(start as u64, MalformedKind::InvalidJpipVbas))?;
        if value > (u64::MAX - u64::from(group)) / 128
            || crate::refinement::jpip_vbas_step(value, group) != stepped
        {
            return Err(DecodeError::malformed(
                start as u64,
                MalformedKind::InvalidJpipVbas,
            ));
        }
        value = stepped;
        *position = position.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(start as u64, MalformedKind::ArithmeticOverflow)
        })?;
        count += 1;
    }
    Ok(value)
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipFragment {
    databin: DatabinId,
    offset: u64,
    bytes: Vec<u8>,
    final_length: Option<u64>,
    auxiliary: Option<u64>,
}

impl JpipFragment {
    pub fn new(
        databin: DatabinId,
        offset: u64,
        bytes: Vec<u8>,
        is_final: bool,
        auxiliary: Option<u64>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        if bytes.is_empty() && !is_final {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidJpipFragment,
            ));
        }
        let length = bytes.len() as u64;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        if !crate::refinement::jpip_fragment_valid(offset, length, limits.maximum_input_bytes()) {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                limits.maximum_input_bytes(),
                end,
            ));
        }
        Ok(Self {
            databin,
            offset,
            bytes,
            final_length: if is_final { Some(end) } else { None },
            auxiliary,
        })
    }

    pub fn from_message(
        header: JpipMessageHeader,
        body: &[u8],
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        if body.len() as u64 != header.length {
            return Err(DecodeError::malformed(
                header.offset,
                MalformedKind::InvalidJpipFragment,
            ));
        }
        Self::new(
            header.databin,
            header.offset,
            copy_bytes(body),
            header.last_byte,
            header.auxiliary,
            limits,
        )
    }

    pub const fn databin(&self) -> DatabinId {
        self.databin
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn final_length(&self) -> Option<u64> {
        self.final_length
    }

    pub const fn auxiliary(&self) -> Option<u64> {
        self.auxiliary
    }
}

#[derive(Clone)]
struct StoredFragment {
    offset: u64,
    bytes: Vec<u8>,
}

impl StoredFragment {
    const fn end(&self) -> u64 {
        self.offset.saturating_add(self.bytes.len() as u64)
    }
}

#[derive(Clone)]
struct Databin {
    id: DatabinId,
    fragments: Vec<StoredFragment>,
    final_length: Option<u64>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DatabinStatus {
    final_length: Option<u64>,
    contiguous_prefix: u64,
    stored_bytes: u64,
    intervals: u64,
}

impl DatabinStatus {
    pub const fn final_length(&self) -> Option<u64> {
        self.final_length
    }

    pub const fn contiguous_prefix(&self) -> u64 {
        self.contiguous_prefix
    }

    pub const fn stored_bytes(&self) -> u64 {
        self.stored_bytes
    }

    pub const fn interval_count(&self) -> u64 {
        self.intervals
    }

    pub const fn is_complete(&self) -> bool {
        match self.final_length {
            Some(length) => {
                crate::refinement::jpip_databin_complete(self.contiguous_prefix, length)
            }
            None => false,
        }
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipByteView {
    bytes: Vec<u8>,
}

impl JpipByteView {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl AsRef<[u8]> for JpipByteView {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipDatabinRange {
    databin: DatabinId,
    offset: u64,
    length: u64,
}

impl JpipDatabinRange {
    pub fn new(databin: DatabinId, offset: u64, length: u64) -> Result<Self, DecodeError> {
        if length == 0 {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidJpipFragment,
            ));
        }
        offset
            .checked_add(length)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        Ok(Self {
            databin,
            offset,
            length,
        })
    }

    pub const fn databin(&self) -> DatabinId {
        self.databin
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn length(&self) -> u64 {
        self.length
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipMissingRange {
    databin: DatabinId,
    offset: u64,
    length: Option<u64>,
}

impl JpipMissingRange {
    pub const fn databin(&self) -> DatabinId {
        self.databin
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Returns `None` only when the data-bin has not announced its final
    /// length, so the required tail is not yet knowable.
    pub const fn length(&self) -> Option<u64> {
        self.length
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum JpipAvailability {
    Missing,
    Partial,
    Available,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipAvailabilityReport {
    status: JpipAvailability,
    required_bytes: u64,
    available_bytes: u64,
    missing: Vec<JpipMissingRange>,
}

impl JpipAvailabilityReport {
    pub const fn status(&self) -> JpipAvailability {
        self.status
    }

    pub const fn required_bytes(&self) -> u64 {
        self.required_bytes
    }

    pub const fn available_bytes(&self) -> u64 {
        self.available_bytes
    }

    pub fn missing_ranges(&self) -> &[JpipMissingRange] {
        &self.missing
    }

    pub const fn is_available(&self) -> bool {
        matches!(self.status, JpipAvailability::Available)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipPrecinctPacket {
    range: JpipDatabinRange,
    layer: u16,
}

impl JpipPrecinctPacket {
    pub fn new(
        databin: DatabinId,
        layer: u16,
        offset: u64,
        length: u64,
    ) -> Result<Self, DecodeError> {
        if !matches!(databin.class(), DatabinClass::Precinct) {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidJpipLayout,
            ));
        }
        Ok(Self {
            range: JpipDatabinRange::new(databin, offset, length)?,
            layer,
        })
    }

    pub const fn range(&self) -> JpipDatabinRange {
        self.range
    }

    pub const fn layer(&self) -> u16 {
        self.layer
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipPrecinctCodestreamPlan {
    main_header: DatabinId,
    tile_header: DatabinId,
    packets: Vec<JpipPrecinctPacket>,
    layer_ends: Vec<usize>,
}

impl JpipPrecinctCodestreamPlan {
    pub fn new(
        main_header: DatabinId,
        tile_header: DatabinId,
        packets: Vec<JpipPrecinctPacket>,
        layers: u16,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        if layers == 0 || layers > limits.maximum_layers() {
            return Err(DecodeError::limit(
                Resource::Layers,
                u64::from(limits.maximum_layers()),
                u64::from(layers),
            ));
        }
        if !matches!(main_header.class(), DatabinClass::MainHeader)
            || main_header.in_class() != 0
            || !matches!(tile_header.class(), DatabinClass::TileHeader)
            || main_header.codestream() != tile_header.codestream()
        {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
        }
        if packets.is_empty() || packets.len() as u64 > limits.maximum_packets() {
            return Err(DecodeError::limit(
                Resource::Packets,
                limits.maximum_packets(),
                packets.len() as u64,
            ));
        }
        let mut layer_ends = Vec::with_capacity(usize::from(layers));
        let mut packet_index = 0_usize;
        let mut layer = 0_u16;
        let mut total_bytes = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant packet_index <= packets.len() && layer <= layers,
                decreases layers - layer,
            )
        )]
        while layer < layers {
            let layer_start = packet_index;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant packet_index <= packets.len(),
                    decreases packets.len() - packet_index,
                )
            )]
            while packet_index < packets.len() && packets[packet_index].layer == layer {
                let packet = packets[packet_index];
                if packet.range.databin.codestream() != main_header.codestream() {
                    return Err(DecodeError::malformed(
                        packet.range.offset,
                        MalformedKind::InvalidJpipLayout,
                    ));
                }
                total_bytes = total_bytes
                    .checked_add(packet.range.length)
                    .ok_or_else(|| {
                        DecodeError::malformed(
                            packet.range.offset,
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                if total_bytes > limits.maximum_input_bytes() {
                    return Err(DecodeError::limit(
                        Resource::InputBytes,
                        limits.maximum_input_bytes(),
                        total_bytes,
                    ));
                }
                packet_index += 1;
            }
            if packet_index == layer_start {
                return Err(DecodeError::malformed(
                    u64::from(layer),
                    MalformedKind::InvalidJpipLayout,
                ));
            }
            layer_ends.push(packet_index);
            layer += 1;
        }
        if packet_index != packets.len() {
            return Err(DecodeError::malformed(
                packet_index as u64,
                MalformedKind::InvalidJpipLayout,
            ));
        }
        Ok(Self {
            main_header,
            tile_header,
            packets,
            layer_ends,
        })
    }

    pub const fn main_header(&self) -> DatabinId {
        self.main_header
    }

    pub const fn tile_header(&self) -> DatabinId {
        self.tile_header
    }

    pub fn packets(&self) -> &[JpipPrecinctPacket] {
        &self.packets
    }

    pub fn layers(&self) -> u16 {
        self.layer_ends.len() as u16
    }

    fn packet_end_for_layers(&self, layers: u16) -> Result<usize, DecodeError> {
        if layers == 0 || layers > self.layers() {
            return Err(DecodeError::malformed(
                u64::from(layers),
                MalformedKind::InvalidJpipLayout,
            ));
        }
        match self.layer_ends.get(usize::from(layers - 1)) {
            Some(end) => Ok(*end),
            None => Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)),
        }
    }
}

pub struct JpipPrecinctTranscode {
    layers: u16,
    availability: JpipAvailabilityReport,
    codestream: Option<JpipByteView>,
}

pub struct JpipPrecinctPackets {
    layers: u16,
    availability: JpipAvailabilityReport,
    packets: Option<JpipByteView>,
}

impl JpipPrecinctPackets {
    pub const fn layers(&self) -> u16 {
        self.layers
    }

    pub const fn availability(&self) -> &JpipAvailabilityReport {
        &self.availability
    }

    /// Returns the caller-planned LRCP packet byte stream. Packet bytes are
    /// copied exactly; no packet-header syntax is invented or rewritten.
    pub const fn packets(&self) -> Option<&JpipByteView> {
        self.packets.as_ref()
    }

    pub fn into_packets(self) -> Option<JpipByteView> {
        self.packets
    }
}

impl JpipPrecinctTranscode {
    pub const fn layers(&self) -> u16 {
        self.layers
    }

    pub const fn availability(&self) -> &JpipAvailabilityReport {
        &self.availability
    }

    pub const fn codestream(&self) -> Option<&JpipByteView> {
        self.codestream.as_ref()
    }

    pub fn into_codestream(self) -> Option<JpipByteView> {
        self.codestream
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum JpipProgressiveStatus {
    WaitingForData,
    QualityAdvanced,
    Complete,
}

pub struct JpipProgressiveUpdate {
    status: JpipProgressiveStatus,
    previous_layers: u16,
    available_layers: u16,
    missing: Vec<JpipMissingRange>,
    image: Option<crate::image::TypedImage>,
}

impl JpipProgressiveUpdate {
    pub const fn status(&self) -> JpipProgressiveStatus {
        self.status
    }

    pub const fn previous_layers(&self) -> u16 {
        self.previous_layers
    }

    pub const fn available_layers(&self) -> u16 {
        self.available_layers
    }

    pub fn missing_ranges(&self) -> &[JpipMissingRange] {
        &self.missing
    }

    pub const fn image(&self) -> Option<&crate::image::TypedImage> {
        self.image.as_ref()
    }

    pub fn into_image(self) -> Option<crate::image::TypedImage> {
        self.image
    }
}

pub struct JpipProgressiveDecoder {
    plan: JpipPrecinctCodestreamPlan,
    decoded_layers: u16,
}

impl JpipProgressiveDecoder {
    pub const fn new(plan: JpipPrecinctCodestreamPlan) -> Self {
        Self {
            plan,
            decoded_layers: 0,
        }
    }

    pub const fn plan(&self) -> &JpipPrecinctCodestreamPlan {
        &self.plan
    }

    pub const fn decoded_layers(&self) -> u16 {
        self.decoded_layers
    }

    pub fn update(
        &mut self,
        cache: &DatabinCache,
        options: &crate::DecodeOptions,
    ) -> Result<JpipProgressiveUpdate, DecodeError> {
        self.update_inner(cache, options, None)
    }

    pub fn update_selected(
        &mut self,
        cache: &DatabinCache,
        options: &crate::DecodeOptions,
        selection: &crate::DecodeSelection,
    ) -> Result<JpipProgressiveUpdate, DecodeError> {
        self.update_inner(cache, options, Some(selection))
    }

    fn update_inner(
        &mut self,
        cache: &DatabinCache,
        options: &crate::DecodeOptions,
        selection: Option<&crate::DecodeSelection>,
    ) -> Result<JpipProgressiveUpdate, DecodeError> {
        let previous_layers = self.decoded_layers;
        let (available_layers, missing) = cache.progressive_availability(&self.plan)?;
        if !crate::refinement::jpip_progressive_update_valid(
            previous_layers,
            available_layers,
            self.plan.layers(),
        ) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
        }
        if !crate::refinement::jpip_quality_prefix_available(available_layers, previous_layers)
            || available_layers <= previous_layers
        {
            return Ok(JpipProgressiveUpdate {
                status: if available_layers == self.plan.layers() {
                    JpipProgressiveStatus::Complete
                } else {
                    JpipProgressiveStatus::WaitingForData
                },
                previous_layers,
                available_layers,
                missing,
                image: None,
            });
        }
        let transcoded = cache.transcode_precinct_codestream(&self.plan, available_layers)?;
        let view = transcoded
            .codestream
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        let image = match selection {
            Some(selection) => {
                crate::decode_codestream_selected(view.as_ref(), options, selection)?
            }
            None => crate::decode_codestream(view.as_ref(), options)?,
        };
        self.decoded_layers = available_layers;
        Ok(JpipProgressiveUpdate {
            status: if available_layers == self.plan.layers() {
                JpipProgressiveStatus::Complete
            } else {
                JpipProgressiveStatus::QualityAdvanced
            },
            previous_layers,
            available_layers,
            missing,
            image: Some(image),
        })
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipPrecinctCoordinate {
    pub tile: u16,
    pub component: u16,
    pub resolution: u8,
    pub precinct: u32,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipPrecinctMapping {
    databin: DatabinId,
    coordinate: JpipPrecinctCoordinate,
}

impl JpipPrecinctMapping {
    pub const fn databin(&self) -> DatabinId {
        self.databin
    }

    pub const fn coordinate(&self) -> JpipPrecinctCoordinate {
        self.coordinate
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipPrecinctMap {
    codestream: u64,
    mappings: Vec<JpipPrecinctMapping>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum JpipSemanticNeedKind {
    MetadataBin,
    Codestream,
    TilePartTransition,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipSemanticNeed {
    kind: JpipSemanticNeedKind,
    identifier: u64,
}

impl JpipSemanticNeed {
    pub const fn kind(&self) -> JpipSemanticNeedKind {
        self.kind
    }

    pub const fn identifier(&self) -> u64 {
        self.identifier
    }
}

pub struct JpipContainerAssembly {
    status: JpipAvailability,
    missing: Vec<JpipMissingRange>,
    needs: Vec<JpipSemanticNeed>,
    container: Option<JpipByteView>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipTilePartTransition {
    tile: u16,
    tile_part: u8,
    packet_count: Option<u64>,
    data_length: Option<u64>,
    functional: bool,
    header: Vec<u8>,
}

impl JpipTilePartTransition {
    pub const fn tile(&self) -> u16 {
        self.tile
    }

    pub const fn tile_part(&self) -> u8 {
        self.tile_part
    }

    pub const fn packet_count(&self) -> Option<u64> {
        self.packet_count
    }

    /// Original tile-part data bytes derived from a non-zero `Psot`.
    ///
    /// `None` denotes the legal final-tile-part form whose `Psot` is zero.
    pub const fn data_length(&self) -> Option<u64> {
        self.data_length
    }

    pub const fn has_functional_change(&self) -> bool {
        self.functional
    }

    pub fn header_bytes(&self) -> &[u8] {
        &self.header
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipTilePartTransitionReport {
    status: JpipAvailability,
    transitions: Vec<JpipTilePartTransition>,
    needs: Vec<JpipSemanticNeed>,
    missing: Vec<JpipMissingRange>,
}

impl JpipTilePartTransitionReport {
    pub const fn status(&self) -> JpipAvailability {
        self.status
    }

    pub fn transitions(&self) -> &[JpipTilePartTransition] {
        &self.transitions
    }

    pub fn semantic_needs(&self) -> &[JpipSemanticNeed] {
        &self.needs
    }

    pub fn missing_ranges(&self) -> &[JpipMissingRange] {
        &self.missing
    }
}

impl JpipContainerAssembly {
    pub const fn status(&self) -> JpipAvailability {
        self.status
    }

    pub fn missing_ranges(&self) -> &[JpipMissingRange] {
        &self.missing
    }

    pub fn semantic_needs(&self) -> &[JpipSemanticNeed] {
        &self.needs
    }

    pub const fn container(&self) -> Option<&JpipByteView> {
        self.container.as_ref()
    }

    pub fn into_container(self) -> Option<JpipByteView> {
        self.container
    }
}

impl JpipPrecinctMap {
    pub const fn codestream(&self) -> u64 {
        self.codestream
    }

    pub fn mappings(&self) -> &[JpipPrecinctMapping] {
        &self.mappings
    }
}

#[derive(Clone, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipPacketSelection {
    components: Vec<u16>,
}

impl JpipPacketSelection {
    pub const fn all_components() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    pub fn components(components: Vec<u16>) -> Self {
        Self { components }
    }

    pub fn selected_components(&self) -> &[u16] {
        &self.components
    }

    fn includes_component(&self, component: u16) -> bool {
        if self.components.is_empty() {
            return true;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.components.len(),
                decreases self.components.len() - index,
            )
        )]
        while index < self.components.len() {
            if self.components[index] == component {
                return true;
            }
            index += 1;
        }
        false
    }
}

pub struct JpipAutomaticProgressiveDecoder {
    codestream: u64,
    decoded_layers: u16,
}

impl JpipAutomaticProgressiveDecoder {
    pub const fn new(codestream: u64) -> Self {
        Self {
            codestream,
            decoded_layers: 0,
        }
    }

    pub const fn codestream(&self) -> u64 {
        self.codestream
    }

    pub const fn decoded_layers(&self) -> u16 {
        self.decoded_layers
    }

    pub fn update(
        &mut self,
        cache: &DatabinCache,
        options: &crate::DecodeOptions,
    ) -> Result<JpipProgressiveUpdate, DecodeError> {
        self.update_inner(cache, options, None, &JpipPacketSelection::all_components())
    }

    pub fn update_selected(
        &mut self,
        cache: &DatabinCache,
        options: &crate::DecodeOptions,
        selection: &crate::DecodeSelection,
        packets: &JpipPacketSelection,
    ) -> Result<JpipProgressiveUpdate, DecodeError> {
        self.update_inner(cache, options, Some(selection), packets)
    }

    fn update_inner(
        &mut self,
        cache: &DatabinCache,
        options: &crate::DecodeOptions,
        selection: Option<&crate::DecodeSelection>,
        packets: &JpipPacketSelection,
    ) -> Result<JpipProgressiveUpdate, DecodeError> {
        let previous_layers = self.decoded_layers;
        let result = cache.build_automatic_snapshot(
            self.codestream,
            selection,
            packets,
            options.limits(),
        )?;
        let Some(snapshot) = result.ready else {
            return Ok(JpipProgressiveUpdate {
                status: JpipProgressiveStatus::WaitingForData,
                previous_layers,
                available_layers: 0,
                missing: result.missing,
                image: None,
            });
        };
        if !crate::refinement::jpip_progressive_update_valid(
            previous_layers,
            snapshot.available_layers,
            snapshot.layers,
        ) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
        }
        if !crate::refinement::jpip_quality_prefix_available(
            snapshot.available_layers,
            previous_layers,
        ) || snapshot.available_layers <= previous_layers
        {
            return Ok(JpipProgressiveUpdate {
                status: if snapshot.available_layers == snapshot.layers {
                    JpipProgressiveStatus::Complete
                } else {
                    JpipProgressiveStatus::WaitingForData
                },
                previous_layers,
                available_layers: snapshot.available_layers,
                missing: snapshot.missing,
                image: None,
            });
        }
        let image = match selection {
            Some(selection) => {
                crate::decode_codestream_selected(snapshot.codestream.as_ref(), options, selection)?
            }
            None => crate::decode_codestream(snapshot.codestream.as_ref(), options)?,
        };
        self.decoded_layers = snapshot.available_layers;
        Ok(JpipProgressiveUpdate {
            status: if snapshot.available_layers == snapshot.layers {
                JpipProgressiveStatus::Complete
            } else {
                JpipProgressiveStatus::QualityAdvanced
            },
            previous_layers,
            available_layers: snapshot.available_layers,
            missing: snapshot.missing,
            image: Some(image),
        })
    }
}

struct AutomaticSnapshotResult {
    ready: Option<AutomaticSnapshot>,
    missing: Vec<JpipMissingRange>,
}

struct AutomaticSnapshot {
    codestream: JpipByteView,
    available_layers: u16,
    layers: u16,
    missing: Vec<JpipMissingRange>,
}

struct AutomaticPacket {
    databin: DatabinId,
    bytes: Option<Vec<u8>>,
    next_offset: u64,
    required: bool,
    ordinal: u16,
    layer: u16,
}

struct AutomaticTile {
    parts: Vec<AutomaticTilePart>,
}

struct AutomaticTilePart {
    header: Vec<u8>,
    packets: Vec<AutomaticPacket>,
    sop: bool,
    eph: bool,
}

struct AutomaticHeaders {
    main: Vec<u8>,
    tiles: Vec<Vec<u8>>,
    tile_count: u64,
    components: u16,
}

pub struct DatabinCache {
    limits: ResourceLimits,
    databins: Vec<Databin>,
    stored_bytes: u64,
    accepted_fragments: u64,
}

impl DatabinCache {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            databins: Vec::new(),
            stored_bytes: 0,
            accepted_fragments: 0,
        }
    }

    pub const fn stored_bytes(&self) -> u64 {
        self.stored_bytes
    }

    pub const fn accepted_fragments(&self) -> u64 {
        self.accepted_fragments
    }

    pub fn databin_count(&self) -> usize {
        self.databins.len()
    }

    #[allow(clippy::collapsible_if)]
    pub fn insert(&mut self, fragment: JpipFragment) -> Result<(), DecodeError> {
        let next_count = self.accepted_fragments.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(fragment.offset, MalformedKind::ArithmeticOverflow)
        })?;
        if next_count > self.limits.maximum_jpip_fragments() {
            return Err(DecodeError::limit(
                Resource::JpipFragments,
                self.limits.maximum_jpip_fragments(),
                next_count,
            ));
        }
        let index = self.find_databin_index(fragment.databin);
        let mut databin = if let Some(index) = index {
            self.databins
                .get(index)
                .ok_or_else(|| {
                    DecodeError::malformed(fragment.offset, MalformedKind::ArithmeticOverflow)
                })?
                .clone()
        } else {
            Databin {
                id: fragment.databin,
                fragments: Vec::new(),
                final_length: None,
            }
        };

        let incoming_end = fragment
            .offset
            .checked_add(fragment.bytes.len() as u64)
            .ok_or_else(|| {
                DecodeError::malformed(fragment.offset, MalformedKind::ArithmeticOverflow)
            })?;
        if let Some(final_length) = databin.final_length {
            if incoming_end > final_length {
                return Err(conflict(fragment.offset));
            }
        }
        if let Some(new_final) = fragment.final_length {
            if let Some(final_length) = databin.final_length {
                if !crate::refinement::jpip_final_length_consistent(final_length, new_final) {
                    return Err(conflict(fragment.offset));
                }
            }
            let mut stored_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant stored_index <= databin.fragments.len(),
                    decreases databin.fragments.len() - stored_index,
                )
            )]
            while stored_index < databin.fragments.len() {
                if databin.fragments[stored_index].end() > new_final {
                    return Err(conflict(fragment.offset));
                }
                stored_index += 1;
            }
            databin.final_length = Some(new_final);
        }

        let mut first = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant first <= databin.fragments.len(),
                decreases databin.fragments.len() - first,
            )
        )]
        while first < databin.fragments.len() && databin.fragments[first].end() < fragment.offset {
            first += 1;
        }
        let mut merged_start = fragment.offset;
        let mut merged_end = incoming_end;
        let mut last = first;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant last <= databin.fragments.len(),
                decreases databin.fragments.len() - last,
            )
        )]
        while last < databin.fragments.len() && databin.fragments[last].offset <= merged_end {
            let stored = &databin.fragments[last];
            let overlap_start = stored.offset.max(fragment.offset);
            let overlap_end = stored.end().min(incoming_end);
            if overlap_start < overlap_end {
                let stored_start =
                    usize::try_from(overlap_start - stored.offset).map_err(|_| {
                        DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                    })?;
                let incoming_start =
                    usize::try_from(overlap_start - fragment.offset).map_err(|_| {
                        DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                    })?;
                let overlap_length =
                    usize::try_from(overlap_end - overlap_start).map_err(|_| {
                        DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                    })?;
                let stored_end = stored_start.checked_add(overlap_length).ok_or_else(|| {
                    DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                })?;
                let incoming_end = incoming_start.checked_add(overlap_length).ok_or_else(|| {
                    DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                })?;
                let stored_overlap =
                    stored.bytes.get(stored_start..stored_end).ok_or_else(|| {
                        DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                    })?;
                let incoming_overlap = fragment
                    .bytes
                    .get(incoming_start..incoming_end)
                    .ok_or_else(|| {
                        DecodeError::malformed(overlap_start, MalformedKind::ArithmeticOverflow)
                    })?;
                if !crate::refinement::jpip_overlap_consistent(stored_overlap == incoming_overlap) {
                    return Err(conflict(overlap_start));
                }
            }
            merged_start = crate::refinement::jpip_merge_start(merged_start, stored.offset);
            merged_end = crate::refinement::jpip_merge_end(merged_end, stored.end());
            last += 1;
        }

        let merged_span = merged_end.checked_sub(merged_start).ok_or_else(|| {
            DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
        })?;
        let merged_length = usize::try_from(merged_span)
            .map_err(|_| DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow))?;
        let mut replaced_bytes = 0_u64;
        let mut replaced_index = first;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases last as int - replaced_index as int,)
        )]
        while replaced_index < last {
            replaced_bytes = replaced_bytes
                .checked_add(
                    databin
                        .fragments
                        .get(replaced_index)
                        .ok_or_else(|| {
                            DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
                        })?
                        .bytes
                        .len() as u64,
                )
                .ok_or_else(|| {
                    DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
                })?;
            replaced_index += 1;
        }
        let added_bytes = (merged_length as u64)
            .checked_sub(replaced_bytes)
            .ok_or_else(|| {
                DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
            })?;
        let new_stored_bytes = self.stored_bytes.checked_add(added_bytes).ok_or_else(|| {
            DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
        })?;
        if new_stored_bytes > self.limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                self.limits.maximum_input_bytes(),
                new_stored_bytes,
            ));
        }

        let mut merged = vec![0_u8; merged_length];
        let mut stored_index = first;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases last as int - stored_index as int,)
        )]
        while stored_index < last {
            let stored = databin.fragments.get(stored_index).ok_or_else(|| {
                DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
            })?;
            let relative = stored.offset.checked_sub(merged_start).ok_or_else(|| {
                DecodeError::malformed(stored.offset, MalformedKind::ArithmeticOverflow)
            })?;
            let start = usize::try_from(relative).map_err(|_| {
                DecodeError::malformed(stored.offset, MalformedKind::ArithmeticOverflow)
            })?;
            copy_into(&mut merged, start, &stored.bytes, stored.offset)?;
            stored_index += 1;
        }
        let incoming_relative = fragment.offset.checked_sub(merged_start).ok_or_else(|| {
            DecodeError::malformed(fragment.offset, MalformedKind::ArithmeticOverflow)
        })?;
        let incoming_start = usize::try_from(incoming_relative).map_err(|_| {
            DecodeError::malformed(fragment.offset, MalformedKind::ArithmeticOverflow)
        })?;
        copy_into(
            &mut merged,
            incoming_start,
            &fragment.bytes,
            fragment.offset,
        )?;

        let normalized_capacity = databin.fragments.len().checked_add(1).ok_or_else(|| {
            DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
        })?;
        let mut normalized = Vec::with_capacity(normalized_capacity);
        let mut prefix_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant prefix_index <= first,
                decreases first - prefix_index,
            )
        )]
        while prefix_index < first {
            normalized.push(
                databin
                    .fragments
                    .get(prefix_index)
                    .ok_or_else(|| {
                        DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
                    })?
                    .clone(),
            );
            prefix_index += 1;
        }
        normalized.push(StoredFragment {
            offset: merged_start,
            bytes: merged,
        });
        let mut suffix_index = last;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant suffix_index <= databin.fragments.len(),
                decreases databin.fragments.len() - suffix_index,
            )
        )]
        while suffix_index < databin.fragments.len() {
            normalized.push(
                databin
                    .fragments
                    .get(suffix_index)
                    .ok_or_else(|| {
                        DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
                    })?
                    .clone(),
            );
            suffix_index += 1;
        }
        databin.fragments = normalized;

        if !crate::refinement::jpip_transaction_commit(true) {
            return Err(conflict(fragment.offset));
        }
        if let Some(index) = index {
            self.databins[index] = databin;
        } else {
            self.databins.push(databin);
        }
        self.stored_bytes = new_stored_bytes;
        self.accepted_fragments = next_count;
        Ok(())
    }

    pub fn status(&self, id: DatabinId) -> Option<DatabinStatus> {
        let index = self.find_databin_index(id)?;
        let databin = self.databins.get(index)?;
        let mut prefix = 0_u64;
        let mut bytes = 0_u64;
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= databin.fragments.len(),
                decreases databin.fragments.len() - index,
            )
        )]
        while index < databin.fragments.len() {
            let fragment = &databin.fragments[index];
            bytes = bytes.saturating_add(fragment.bytes.len() as u64);
            if fragment.offset == prefix {
                prefix = fragment.end();
            }
            index += 1;
        }
        Some(DatabinStatus {
            final_length: databin.final_length,
            contiguous_prefix: prefix,
            stored_bytes: bytes,
            intervals: databin.fragments.len() as u64,
        })
    }

    pub fn is_complete(&self, id: DatabinId) -> bool {
        match self.status(id) {
            Some(status) => status.is_complete(),
            None => false,
        }
    }

    pub fn databin_view(&self, id: DatabinId) -> Result<JpipByteView, DecodeError> {
        let index = self
            .find_databin_index(id)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        let databin = self
            .databins
            .get(index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let status = self
            .status(id)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        if !status.is_complete() {
            return Err(DecodeError::malformed(
                status.contiguous_prefix,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        let bytes = if databin.fragments.is_empty() {
            Vec::new()
        } else {
            databin.fragments[0].bytes.clone()
        };
        Ok(JpipByteView { bytes })
    }

    pub fn reconstruct_codestream(
        &self,
        layout: &[DatabinId],
    ) -> Result<JpipByteView, DecodeError> {
        if layout.len() < 3 || layout.len() & 1 == 0 {
            return Err(DecodeError::malformed(
                layout.len() as u64,
                MalformedKind::InvalidJpipLayout,
            ));
        }
        let first = layout[0];
        if !matches!(first.class, DatabinClass::MainHeader) || first.in_class != 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
        }
        let mut pair = 1_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant pair <= layout.len(),
                decreases layout.len() - pair,
            )
        )]
        while pair < layout.len() {
            let data_index = pair.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(pair as u64, MalformedKind::ArithmeticOverflow)
            })?;
            let header = *layout.get(pair).ok_or_else(|| {
                DecodeError::malformed(pair as u64, MalformedKind::InvalidJpipLayout)
            })?;
            let data = *layout.get(data_index).ok_or_else(|| {
                DecodeError::malformed(data_index as u64, MalformedKind::InvalidJpipLayout)
            })?;
            if !matches!(header.class, DatabinClass::TileHeader)
                || !matches!(data.class, DatabinClass::TileData)
                || header.codestream != first.codestream
                || data.codestream != first.codestream
                || header.in_class != data.in_class
            {
                return Err(DecodeError::malformed(
                    pair as u64,
                    MalformedKind::InvalidJpipLayout,
                ));
            }
            pair += 2;
        }
        let view = self.reconstruct(layout)?;
        if !view.bytes.starts_with(&SOC) || !view.bytes.ends_with(&EOC) {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        Ok(view)
    }

    pub fn reconstruct_container(&self, layout: &[DatabinId]) -> Result<JpipByteView, DecodeError> {
        if layout.is_empty() {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= layout.len(),
                decreases layout.len() - index,
            )
        )]
        while index < layout.len() {
            if !matches!(layout[index].class, DatabinClass::Metadata) {
                return Err(DecodeError::malformed(
                    index as u64,
                    MalformedKind::InvalidJpipLayout,
                ));
            }
            index += 1;
        }
        self.reconstruct(layout)
    }

    pub fn derive_precinct_map(&self, codestream: u64) -> Result<JpipPrecinctMap, DecodeError> {
        let headers = self.automatic_headers(codestream)?;
        let template = build_automatic_template(&headers.main, &headers.tiles, &self.limits)?;
        let layout = crate::codestream::parse_layout(&template, &self.limits)?;
        let mut mappings = Vec::new();
        let mut tile = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases headers.tile_count as int - tile as int,)
        )]
        while u64::from(tile) < headers.tile_count {
            let tile_part = automatic_tile_part(&layout.tile_parts, tile)?;
            let plan = crate::decode::plan_progression_tile_at_with_component_overrides(
                &layout.info.image_size,
                tile,
                tile_part.coding_style,
                &tile_part.component_coding_styles,
                &tile_part.component_quantizations,
                &tile_part.roi_shifts,
                &self.limits,
            )?;
            let grid = plan.grid();
            let precinct_count = plan.precincts().len();
            let mut precinct_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant precinct_index <= precinct_count,
                    decreases precinct_count - precinct_index,
                )
            )]
            while precinct_index < precinct_count {
                let (component, resolution, precinct) = plan
                    .precincts()
                    .get(precinct_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
                    .coordinate();
                let in_class = automatic_precinct_in_class(
                    headers.tile_count,
                    u64::from(headers.components),
                    u64::from(tile),
                    component,
                    resolution,
                    precinct,
                    grid,
                )?;
                mappings.push(JpipPrecinctMapping {
                    databin: DatabinId::new(0, codestream, in_class)?,
                    coordinate: JpipPrecinctCoordinate {
                        tile,
                        component,
                        resolution,
                        precinct,
                    },
                });
                precinct_index += 1;
            }
            tile = tile
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        Ok(JpipPrecinctMap {
            codestream,
            mappings,
        })
    }

    pub fn tile_part_transitions(
        &self,
        codestream: u64,
        tile: u16,
    ) -> Result<JpipTilePartTransitionReport, DecodeError> {
        let id = DatabinId::new(2, codestream, u64::from(tile))?;
        if !self.is_complete(id) {
            return Ok(JpipTilePartTransitionReport {
                status: JpipAvailability::Missing,
                transitions: Vec::new(),
                needs: vec![JpipSemanticNeed {
                    kind: JpipSemanticNeedKind::TilePartTransition,
                    identifier: u64::from(tile),
                }],
                missing: self.complete_databin_missing(id),
            });
        }
        let bytes = self.databin_view(id)?.into_bytes();
        let transitions = parse_tile_part_transitions(&bytes, tile)?;
        let mut needs = Vec::new();
        let mut preceding_packets_known = true;
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= transitions.len(),
                decreases transitions.len() - index,
            )
        )]
        while index < transitions.len() {
            let transition = &transitions[index];
            if index > 0
                && !crate::refinement::jpip_tile_transition_sufficient(
                    transition.has_functional_change(),
                    preceding_packets_known,
                    transition.data_length().is_some(),
                )
            {
                needs.push(JpipSemanticNeed {
                    kind: JpipSemanticNeedKind::TilePartTransition,
                    identifier: (u64::from(tile) << 8) | u64::from(transition.tile_part()),
                });
            }
            if transition.packet_count().is_none() && transition.data_length().is_none() {
                preceding_packets_known = false;
            }
            index += 1;
        }
        Ok(JpipTilePartTransitionReport {
            status: if needs.is_empty() {
                JpipAvailability::Available
            } else {
                JpipAvailability::Partial
            },
            transitions,
            needs,
            missing: Vec::new(),
        })
    }

    pub fn assemble_semantic_container(
        &self,
        metadata_codestream: u64,
    ) -> Result<JpipContainerAssembly, DecodeError> {
        let mut output = Vec::new();
        let mut missing = Vec::new();
        let mut needs = Vec::new();
        let mut active = Vec::new();
        let complete = self.expand_metadata_bin(
            metadata_codestream,
            0,
            true,
            0,
            u32::MAX,
            &mut active,
            &mut output,
            &mut missing,
            &mut needs,
        )?;
        if !complete {
            return Ok(JpipContainerAssembly {
                status: if output.is_empty() {
                    JpipAvailability::Missing
                } else {
                    JpipAvailability::Partial
                },
                missing,
                needs,
                container: None,
            });
        }
        if output.len() as u64 > self.limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                self.limits.maximum_input_bytes(),
                output.len() as u64,
            ));
        }
        crate::jp2::parse_and_inspect(&output, &self.limits)?;
        Ok(JpipContainerAssembly {
            status: JpipAvailability::Available,
            missing,
            needs,
            container: Some(JpipByteView { bytes: output }),
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(verus_keep_ghost, verus_spec(decreases remaining_depth,))]
    fn expand_metadata_bin(
        &self,
        metadata_codestream: u64,
        bin: u64,
        boxes: bool,
        depth: u32,
        remaining_depth: u32,
        active: &mut Vec<u64>,
        output: &mut Vec<u8>,
        missing: &mut Vec<JpipMissingRange>,
        needs: &mut Vec<JpipSemanticNeed>,
    ) -> Result<bool, DecodeError> {
        if !crate::refinement::jpip_metadata_descent_allowed(
            depth,
            self.limits.maximum_box_depth(),
            false,
        ) {
            return Err(DecodeError::limit(
                Resource::BoxDepth,
                u64::from(self.limits.maximum_box_depth()),
                u64::from(depth),
            ));
        }
        let mut active_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant active_index <= active.len(),
                decreases active.len() - active_index,
            )
        )]
        while active_index < active.len() {
            if !crate::refinement::jpip_metadata_descent_allowed(
                depth,
                self.limits.maximum_box_depth(),
                active[active_index] == bin,
            ) {
                return Err(DecodeError::malformed(
                    bin,
                    MalformedKind::InvalidBoxContents,
                ));
            }
            active_index += 1;
        }
        let id = DatabinId::new(8, metadata_codestream, bin)?;
        if !self.is_complete(id) {
            append_missing_ranges(missing, &self.complete_databin_missing(id));
            needs.push(JpipSemanticNeed {
                kind: JpipSemanticNeedKind::MetadataBin,
                identifier: bin,
            });
            return Ok(false);
        }
        let bytes = self.databin_view(id)?.into_bytes();
        if !boxes {
            output.extend_from_slice(&bytes);
            return Ok(true);
        }
        active.push(bin);
        let mut reader = crate::jp2::BoxReader::new(&bytes, &self.limits)?;
        let mut complete = true;
        let mut remaining_boxes = bytes.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_boxes,)
        )]
        while let Some(box_result) = reader.next_box_ref() {
            if remaining_boxes == 0 {
                return Err(DecodeError::malformed(bin, MalformedKind::InvalidBoxLength));
            }
            remaining_boxes -= 1;
            let box_ref = box_result?;
            if box_ref
                .header
                .box_type()
                .equals(crate::jp2::BoxType::new(*b"phld"))
            {
                let placeholder = parse_jpip_placeholder(box_ref.payload)?;
                let mut payload = Vec::new();
                let mut appended = false;
                let replacement_complete = if placeholder.flags & 1 != 0 {
                    self.expand_metadata_bin(
                        metadata_codestream,
                        placeholder.bin,
                        jpip_superbox(placeholder.original_type),
                        depth.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?,
                        remaining_depth.checked_sub(1).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?,
                        active,
                        &mut payload,
                        missing,
                        needs,
                    )?
                } else if placeholder.flags & 2 != 0 {
                    let bin = placeholder.equivalent_bin.ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
                    let box_type = placeholder.equivalent_type.ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
                    let _header = placeholder.equivalent_header.as_ref().ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
                    self.expand_metadata_bin(
                        metadata_codestream,
                        bin,
                        jpip_superbox(box_type),
                        depth.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?,
                        remaining_depth.checked_sub(1).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?,
                        active,
                        &mut payload,
                        missing,
                        needs,
                    )?
                } else if placeholder.flags & 4 != 0 {
                    let first_stream = placeholder.stream.ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
                    let mut stream_index = 0_u32;
                    let mut streams_complete = true;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant stream_index <= placeholder.stream_count,
                            decreases placeholder.stream_count - stream_index,
                        )
                    )]
                    while stream_index < placeholder.stream_count {
                        let stream = first_stream
                            .checked_add(u64::from(stream_index))
                            .ok_or_else(|| {
                                DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                            })?;
                        let snapshot = self.build_automatic_snapshot(
                            stream,
                            None,
                            &JpipPacketSelection::all_components(),
                            &self.limits,
                        )?;
                        match snapshot.ready {
                            Some(snapshot) if snapshot.available_layers == snapshot.layers => {
                                if placeholder.stream_count == 1 {
                                    payload.extend_from_slice(snapshot.codestream.as_bytes());
                                } else {
                                    append_reconstructed_box(
                                        output,
                                        &placeholder.original_header,
                                        snapshot.codestream.as_bytes(),
                                    )?;
                                }
                            }
                            Some(snapshot) => {
                                append_missing_ranges(missing, &snapshot.missing);
                                needs.push(JpipSemanticNeed {
                                    kind: JpipSemanticNeedKind::Codestream,
                                    identifier: stream,
                                });
                                streams_complete = false;
                            }
                            None => {
                                append_missing_ranges(missing, &snapshot.missing);
                                needs.push(JpipSemanticNeed {
                                    kind: JpipSemanticNeedKind::Codestream,
                                    identifier: stream,
                                });
                                streams_complete = false;
                            }
                        }
                        stream_index += 1;
                    }
                    appended = placeholder.stream_count > 1 && streams_complete;
                    streams_complete
                } else {
                    payload.clear();
                    true
                };
                if replacement_complete {
                    if !appended {
                        append_reconstructed_box(output, &placeholder.original_header, &payload)?;
                    }
                } else {
                    complete = false;
                }
            } else {
                let start = usize::try_from(box_ref.header.offset())
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let length = usize::try_from(box_ref.header.total_length())
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let end = start
                    .checked_add(length)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                output.extend_from_slice(bytes.get(start..end).ok_or_else(|| {
                    DecodeError::malformed(start as u64, MalformedKind::Truncated)
                })?);
            }
        }
        active.pop();
        Ok(complete)
    }

    #[allow(clippy::manual_map)]
    fn build_automatic_snapshot(
        &self,
        codestream: u64,
        selection: Option<&crate::DecodeSelection>,
        packet_selection: &JpipPacketSelection,
        limits: &ResourceLimits,
    ) -> Result<AutomaticSnapshotResult, DecodeError> {
        let main_id = DatabinId::new(6, codestream, 0)?;
        if !self.is_complete(main_id) {
            return Ok(AutomaticSnapshotResult {
                ready: None,
                missing: self.complete_databin_missing(main_id),
            });
        }
        let main = self.databin_view(main_id)?.into_bytes();
        let (tile_count, components) = siz_cardinality(&main, limits)?;
        validate_packet_component_selection(packet_selection, components)?;
        let mut tile_headers = Vec::with_capacity(
            usize::try_from(tile_count)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        let mut header_missing = Vec::new();
        let mut tile = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile <= tile_count,
                decreases tile_count - tile,
            )
        )]
        while tile < tile_count {
            let tile_id = DatabinId::new(2, codestream, tile)?;
            if self.is_complete(tile_id) {
                tile_headers.push(self.databin_view(tile_id)?.into_bytes());
            } else {
                let additional = self.complete_databin_missing(tile_id);
                let mut missing_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant missing_index <= additional.len(),
                        decreases additional.len() - missing_index,
                    )
                )]
                while missing_index < additional.len() {
                    header_missing.push(additional[missing_index]);
                    missing_index += 1;
                }
            }
            tile = tile
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        if !header_missing.is_empty() {
            return Ok(AutomaticSnapshotResult {
                ready: None,
                missing: header_missing,
            });
        }

        let normalized_main = normalize_main_header(&main)?;
        let mut normalized_tiles = Vec::with_capacity(tile_headers.len());
        let mut tile_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_index <= tile_headers.len(),
                decreases tile_headers.len() - tile_index,
            )
        )]
        while tile_index < tile_headers.len() {
            normalized_tiles.push(normalize_automatic_tile_headers(
                tile_headers
                    .get(tile_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
                u16::try_from(tile_index)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            )?);
            tile_index += 1;
        }
        let template =
            build_automatic_transition_template(&normalized_main, &normalized_tiles, limits)?;
        let layout = crate::codestream::parse_layout(&template, limits)?;
        let mut automatic_tiles = Vec::with_capacity(normalized_tiles.len());
        let mut maximum_layers = 0_u16;
        tile_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_index <= normalized_tiles.len(),
                decreases normalized_tiles.len() - tile_index,
            )
        )]
        while tile_index < normalized_tiles.len() {
            let tile_number = u16::try_from(tile_index)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            let tile_parts: Vec<_> = layout
                .tile_parts
                .iter()
                .filter(|part| part.tile_index == tile_number)
                .collect();
            let tile_part = *tile_parts
                .first()
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
            let mut plan = crate::decode::plan_progression_tile_at_with_component_overrides(
                &layout.info.image_size,
                tile_number,
                tile_part.coding_style,
                &tile_part.component_coding_styles,
                &tile_part.component_quantizations,
                &tile_part.roi_shifts,
                limits,
            )?;
            maximum_layers = maximum_layers.max(tile_part.coding_style.layers);
            let precinct_count = plan.precincts().len();
            let layer_count = usize::from(tile_part.coding_style.layers);
            let mut ordinals = vec![vec![None; layer_count]; precinct_count];
            let mut order = Vec::new();
            let mut iterator = crate::packet::ProgressionPacketIterator::new(
                plan.grid(),
                tile_part.coding_style.progression_order,
                &tile_part.progression_changes,
                limits,
            )?;
            let progression_code = match tile_part.coding_style.progression_order {
                crate::codestream::ProgressionOrder::Lrcp => 0,
                crate::codestream::ProgressionOrder::Rlcp => 1,
                crate::codestream::ProgressionOrder::Rpcl => 2,
                crate::codestream::ProgressionOrder::Pcrl => 3,
                crate::codestream::ProgressionOrder::Cprl => 4,
            };
            if !crate::refinement::jpip_progression_supported(progression_code) {
                return Err(DecodeError::malformed(
                    0,
                    MalformedKind::InvalidProgressionOrderChange,
                ));
            }
            let mut ordinal = 0_u64;
            let mut remaining_packets = limits.maximum_work_units();
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(decreases remaining_packets,)
            )]
            while let Some(coordinate) = iterator.next_packet()? {
                if remaining_packets == 0 {
                    return Err(DecodeError::malformed(
                        0,
                        MalformedKind::InvalidProgressionOrderChange,
                    ));
                }
                remaining_packets -= 1;
                let execution = plan.grid().precinct_index(
                    coordinate.component,
                    coordinate.resolution,
                    coordinate.precinct,
                )?;
                let sequence = u16::try_from(ordinal & u64::from(u16::MAX))
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let layer_slot = ordinals
                    .get_mut(execution)
                    .and_then(|layers| layers.get_mut(usize::from(coordinate.layer)))
                    .ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidProgressionOrderChange)
                    })?;
                if layer_slot.is_some() {
                    return Err(DecodeError::malformed(
                        0,
                        MalformedKind::InvalidProgressionOrderChange,
                    ));
                }
                *layer_slot = Some(sequence);
                order.push((coordinate, execution));
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            }

            let mut parsed = vec![vec![None; layer_count]; precinct_count];
            let mut offsets = vec![0_u64; precinct_count];
            let mut databins = Vec::with_capacity(precinct_count);
            let grid = plan.grid();
            let mut mapping_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant mapping_index <= precinct_count,
                    decreases precinct_count - mapping_index,
                )
            )]
            while mapping_index < precinct_count {
                let (component, resolution, precinct) = plan
                    .precincts()
                    .get(mapping_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
                    .coordinate();
                let in_class = automatic_precinct_in_class(
                    tile_count,
                    u64::from(components),
                    tile_index as u64,
                    component,
                    resolution,
                    precinct,
                    grid,
                )?;
                databins.push(DatabinId::new(0, codestream, in_class)?);
                mapping_index += 1;
            }
            let source_tile_header = tile_headers
                .get(tile_index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            let transitions = parse_tile_part_transitions(source_tile_header, tile_number)?;
            let mut transition_index = 0_usize;
            let mut transition_packets = 0_u64;
            let mut transition_bytes = 0_u64;
            let mut blocked = vec![false; precinct_count];
            let mut order_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant order_index <= order.len(),
                    decreases order.len() - order_index,
                )
            )]
            while order_index < order.len() {
                let (coordinate, execution) = order[order_index];
                if *blocked
                    .get(execution)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
                {
                    order_index += 1;
                    continue;
                }
                let state = *tile_parts
                    .get(transition_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
                let databin = *databins
                    .get(execution)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let prefix = self.databin_prefix(databin);
                let offset = *offsets
                    .get(execution)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let start = usize::try_from(offset)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                if start >= prefix.len() {
                    *blocked.get_mut(execution).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })? = true;
                    order_index += 1;
                    continue;
                }
                let sequence = u16::try_from(order_index & usize::from(u16::MAX))
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let precinct_decoder = plan
                    .precincts_mut()
                    .get_mut(execution)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                match crate::packet::decode_jpip_precinct_packet(
                    state,
                    &prefix[start..],
                    sequence,
                    coordinate.layer,
                    precinct_decoder,
                    limits,
                ) {
                    Ok(consumed) => {
                        if consumed == 0 {
                            return Err(DecodeError::malformed(
                                offset,
                                MalformedKind::InvalidPacketLength,
                            ));
                        }
                        let end = start.checked_add(consumed).ok_or_else(|| {
                            DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow)
                        })?;
                        let packet_slice = prefix.get(start..end).ok_or_else(|| {
                            DecodeError::malformed(offset, MalformedKind::Truncated)
                        })?;
                        *parsed
                            .get_mut(execution)
                            .and_then(|layers| layers.get_mut(usize::from(coordinate.layer)))
                            .ok_or_else(|| {
                                DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                            })? = Some(copy_bytes(packet_slice));
                        *offsets.get_mut(execution).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })? = end as u64;
                        transition_packets =
                            transition_packets.checked_add(1).ok_or_else(|| {
                                DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                            })?;
                        transition_bytes = transition_bytes
                            .checked_add(consumed as u64)
                            .ok_or_else(|| {
                                DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                            })?;
                        let transition = transitions.get(transition_index).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::InvalidTilePart)
                        })?;
                        let boundary = match transition.packet_count() {
                            Some(count) => transition_packets == count,
                            None => match transition.data_length() {
                                Some(length) => transition_bytes == length,
                                None => false,
                            },
                        };
                        let next_transition = transition_index.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?;
                        if boundary && next_transition < transitions.len() {
                            transition_index = next_transition;
                            transition_packets = 0;
                            transition_bytes = 0;
                        }
                    }
                    Err(DecodeError::MalformedInput {
                        kind: MalformedKind::Truncated,
                        ..
                    }) if !self.is_complete(databin) => {
                        *blocked.get_mut(execution).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })? = true;
                    }
                    Err(error) => return Err(error),
                }
                order_index += 1;
            }

            let tile_is_selected = jpip_tile_selected(selection, tile_number);
            let mut packets = Vec::with_capacity(order.len());
            let mut order_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant order_index <= order.len(),
                    decreases order.len() - order_index,
                )
            )]
            while order_index < order.len() {
                let (coordinate, execution) = order[order_index];
                let required = tile_is_selected
                    && packet_selection.includes_component(coordinate.component)
                    && jpip_resolution_selected(
                        selection,
                        coordinate.component,
                        coordinate.resolution,
                        &tile_part.component_coding_styles,
                    )?;
                let parsed_layers = parsed
                    .get(execution)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let packet_bytes = match parsed_layers
                    .get(usize::from(coordinate.layer))
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
                {
                    Some(bytes) => Some(bytes.clone()),
                    None => None,
                };
                packets.push(AutomaticPacket {
                    databin: *databins.get(execution).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?,
                    bytes: packet_bytes,
                    next_offset: *offsets.get(execution).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?,
                    required,
                    ordinal: u16::try_from(order_index & usize::from(u16::MAX)).map_err(|_| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?,
                    layer: coordinate.layer,
                });
                order_index += 1;
            }
            let Some(parts) = partition_automatic_tile_parts(
                normalized_tiles
                    .get(tile_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
                &tile_parts,
                &transitions,
                &packets,
            )?
            else {
                let mut missing = Vec::new();
                let mut packet_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant packet_index <= packets.len(),
                        decreases packets.len() - packet_index,
                    )
                )]
                while packet_index < packets.len() {
                    let packet = packets.get(packet_index).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                    if packet.bytes.is_none()
                        && !missing
                            .iter()
                            .any(|entry: &JpipMissingRange| entry.databin().same_as(packet.databin))
                    {
                        missing.push(JpipMissingRange {
                            databin: packet.databin,
                            offset: packet.next_offset,
                            length: self
                                .status(packet.databin)
                                .and_then(|status| status.final_length())
                                .map(|length| length.saturating_sub(packet.next_offset)),
                        });
                    }
                    packet_index += 1;
                }
                return Ok(AutomaticSnapshotResult {
                    ready: None,
                    missing,
                });
            };
            automatic_tiles.push(AutomaticTile { parts });
            tile_index += 1;
        }

        let mut available_layers = maximum_layers;
        let mut tile_packet_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_packet_index <= automatic_tiles.len(),
                decreases automatic_tiles.len() - tile_packet_index,
            )
        )]
        while tile_packet_index < automatic_tiles.len() {
            let tile = &automatic_tiles[tile_packet_index];
            let mut part_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant part_index <= tile.parts.len(),
                    decreases tile.parts.len() - part_index,
                )
            )]
            while part_index < tile.parts.len() {
                let part = &tile.parts[part_index];
                let mut packet_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant packet_index <= part.packets.len(),
                        decreases part.packets.len() - packet_index,
                    )
                )]
                while packet_index < part.packets.len() {
                    let packet = &part.packets[packet_index];
                    let layer = packet.layer;
                    if packet.required && packet.bytes.is_none() {
                        available_layers = available_layers.min(layer);
                    }
                    packet_index += 1;
                }
                part_index += 1;
            }
            tile_packet_index += 1;
        }
        let mut missing = Vec::new();
        tile_packet_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_packet_index <= automatic_tiles.len(),
                decreases automatic_tiles.len() - tile_packet_index,
            )
        )]
        while tile_packet_index < automatic_tiles.len() {
            let tile = &automatic_tiles[tile_packet_index];
            let mut part_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant part_index <= tile.parts.len(),
                    decreases tile.parts.len() - part_index,
                )
            )]
            while part_index < tile.parts.len() {
                let part = &tile.parts[part_index];
                let mut packet_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant packet_index <= part.packets.len(),
                        decreases part.packets.len() - packet_index,
                    )
                )]
                while packet_index < part.packets.len() {
                    let packet = &part.packets[packet_index];
                    let layer = packet.layer;
                    if packet.required
                        && layer == available_layers
                        && packet.bytes.is_none()
                        && !missing
                            .iter()
                            .any(|entry: &JpipMissingRange| entry.databin().same_as(packet.databin))
                    {
                        let status = self.status(packet.databin);
                        let length = match status.and_then(|value| value.final_length()) {
                            Some(final_length) if final_length > packet.next_offset => {
                                Some(final_length - packet.next_offset)
                            }
                            Some(_) => Some(1),
                            None => None,
                        };
                        missing.push(JpipMissingRange {
                            databin: packet.databin,
                            offset: packet.next_offset,
                            length,
                        });
                    }
                    packet_index += 1;
                }
                part_index += 1;
            }
            tile_packet_index += 1;
        }
        if available_layers == 0 {
            return Ok(AutomaticSnapshotResult {
                ready: Some(AutomaticSnapshot {
                    codestream: JpipByteView { bytes: Vec::new() },
                    available_layers,
                    layers: maximum_layers,
                    missing,
                }),
                missing: Vec::new(),
            });
        }

        let mut bytes = normalized_main;
        tile_packet_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_packet_index <= automatic_tiles.len(),
                decreases automatic_tiles.len() - tile_packet_index,
            )
        )]
        while tile_packet_index < automatic_tiles.len() {
            let tile = &automatic_tiles[tile_packet_index];
            let mut part_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant part_index <= tile.parts.len(),
                    decreases tile.parts.len() - part_index,
                )
            )]
            while part_index < tile.parts.len() {
                let part = &tile.parts[part_index];
                let tile_start = bytes.len();
                bytes.extend_from_slice(&part.header);
                let mut packet_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant packet_index <= part.packets.len(),
                        decreases part.packets.len() - packet_index,
                    )
                )]
                while packet_index < part.packets.len() {
                    let packet = &part.packets[packet_index];
                    let layer = packet.layer;
                    if packet.required && layer < available_layers {
                        let packet_bytes = packet.bytes.as_ref().ok_or_else(|| {
                            DecodeError::malformed(
                                packet.next_offset,
                                MalformedKind::MissingRequiredStructure,
                            )
                        })?;
                        bytes.extend_from_slice(packet_bytes);
                    } else {
                        append_empty_packet(&mut bytes, part.sop, part.eph, packet.ordinal)?;
                    }
                    packet_index += 1;
                }
                let tile_length = bytes
                    .len()
                    .checked_sub(tile_start)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let tile_length = u32::try_from(tile_length)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let length_position = tile_start
                    .checked_add(6)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                write_u32_be(&mut bytes, length_position, tile_length)?;
                part_index += 1;
            }
            tile_packet_index += 1;
        }
        bytes.extend_from_slice(&EOC);
        if bytes.len() as u64 > limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                limits.maximum_input_bytes(),
                bytes.len() as u64,
            ));
        }
        crate::codestream::parse_layout(&bytes, limits)?;
        Ok(AutomaticSnapshotResult {
            ready: Some(AutomaticSnapshot {
                codestream: JpipByteView { bytes },
                available_layers,
                layers: maximum_layers,
                missing,
            }),
            missing: Vec::new(),
        })
    }

    fn automatic_headers(&self, codestream: u64) -> Result<AutomaticHeaders, DecodeError> {
        let main = self
            .databin_view(DatabinId::new(6, codestream, 0)?)?
            .into_bytes();
        let (tile_count, components) = siz_cardinality(&main, &self.limits)?;
        let mut tiles = Vec::with_capacity(
            usize::try_from(tile_count)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        let mut tile = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile <= tile_count,
                decreases tile_count - tile,
            )
        )]
        while tile < tile_count {
            tiles.push(
                self.databin_view(DatabinId::new(2, codestream, tile)?)?
                    .into_bytes(),
            );
            tile += 1;
        }
        let main = normalize_main_header(&main)?;
        let mut normalized = Vec::with_capacity(tiles.len());
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= tiles.len(),
                decreases tiles.len() - index,
            )
        )]
        while index < tiles.len() {
            let headers = normalize_automatic_tile_headers(
                &tiles[index],
                u16::try_from(index)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            )?;
            normalized.push(
                headers
                    .first()
                    .ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::MissingRequiredStructure)
                    })?
                    .clone(),
            );
            index += 1;
        }
        Ok(AutomaticHeaders {
            main,
            tiles: normalized,
            tile_count,
            components,
        })
    }

    fn complete_databin_missing(&self, id: DatabinId) -> Vec<JpipMissingRange> {
        match self.status(id) {
            Some(status) => vec![JpipMissingRange {
                databin: id,
                offset: status.contiguous_prefix(),
                length: status.final_length().map(|length| {
                    crate::refinement::jpip_missing_length(
                        length,
                        status.contiguous_prefix().min(length),
                    )
                }),
            }],
            None => vec![JpipMissingRange {
                databin: id,
                offset: 0,
                length: None,
            }],
        }
    }

    fn databin_prefix(&self, id: DatabinId) -> Vec<u8> {
        let Some(index) = self.find_databin_index(id) else {
            return Vec::new();
        };
        let Some(databin) = self.databins.get(index) else {
            return Vec::new();
        };
        let Some(first) = databin.fragments.first() else {
            return Vec::new();
        };
        if first.offset != 0 {
            return Vec::new();
        }
        first.bytes.clone()
    }

    pub fn availability(
        &self,
        ranges: &[JpipDatabinRange],
    ) -> Result<JpipAvailabilityReport, DecodeError> {
        if ranges.len() as u64 > self.limits.maximum_packets() {
            return Err(DecodeError::limit(
                Resource::Packets,
                self.limits.maximum_packets(),
                ranges.len() as u64,
            ));
        }
        let mut required_bytes = 0_u64;
        let mut missing = Vec::new();
        let mut work = 0_u64;
        let mut range_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant range_index <= ranges.len(),
                decreases ranges.len() - range_index,
            )
        )]
        while range_index < ranges.len() {
            let requested = ranges[range_index];
            let requested_end =
                requested
                    .offset
                    .checked_add(requested.length)
                    .ok_or_else(|| {
                        DecodeError::malformed(requested.offset, MalformedKind::ArithmeticOverflow)
                    })?;
            if requested_end > self.limits.maximum_input_bytes() {
                return Err(DecodeError::limit(
                    Resource::InputBytes,
                    self.limits.maximum_input_bytes(),
                    requested_end,
                ));
            }
            required_bytes = required_bytes
                .checked_add(requested.length)
                .ok_or_else(|| {
                    DecodeError::malformed(requested.offset, MalformedKind::ArithmeticOverflow)
                })?;
            if required_bytes > self.limits.maximum_input_bytes() {
                return Err(DecodeError::limit(
                    Resource::InputBytes,
                    self.limits.maximum_input_bytes(),
                    required_bytes,
                ));
            }
            let Some(databin_index) = self.find_databin_index(requested.databin) else {
                missing.push(JpipMissingRange {
                    databin: requested.databin,
                    offset: requested.offset,
                    length: Some(requested.length),
                });
                range_index += 1;
                continue;
            };
            let databin = self.databins.get(databin_index).ok_or_else(|| {
                DecodeError::malformed(requested.offset, MalformedKind::ArithmeticOverflow)
            })?;
            let mut cursor = requested.offset;
            let mut fragment_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant fragment_index <= databin.fragments.len(),
                    decreases databin.fragments.len() - fragment_index,
                )
            )]
            while fragment_index < databin.fragments.len() && cursor < requested_end {
                work = work.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(cursor, MalformedKind::ArithmeticOverflow)
                })?;
                if work > self.limits.maximum_work_units() {
                    return Err(DecodeError::limit(
                        Resource::WorkUnits,
                        self.limits.maximum_work_units(),
                        work,
                    ));
                }
                let fragment = &databin.fragments[fragment_index];
                if fragment.end() <= cursor {
                    fragment_index += 1;
                    continue;
                }
                if fragment.offset >= requested_end {
                    break;
                }
                if fragment.offset > cursor {
                    let missing_end = fragment.offset.min(requested_end);
                    missing.push(JpipMissingRange {
                        databin: requested.databin,
                        offset: cursor,
                        length: Some(missing_end - cursor),
                    });
                    cursor = missing_end;
                }
                if cursor < requested_end && fragment.end() > cursor {
                    cursor = fragment.end().min(requested_end);
                }
                fragment_index += 1;
            }
            if cursor < requested_end {
                missing.push(JpipMissingRange {
                    databin: requested.databin,
                    offset: cursor,
                    length: Some(requested_end - cursor),
                });
            }
            range_index += 1;
        }
        let mut missing_bytes = 0_u64;
        let mut missing_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant missing_index <= missing.len(),
                decreases missing.len() - missing_index,
            )
        )]
        while missing_index < missing.len() {
            missing_bytes = missing_bytes
                .checked_add(missing[missing_index].length.unwrap_or(0))
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            missing_index += 1;
        }
        let available_bytes = required_bytes
            .checked_sub(missing_bytes)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if missing_bytes > required_bytes
            || crate::refinement::jpip_available_bytes(required_bytes, missing_bytes)
                != available_bytes
        {
            return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
        }
        Ok(JpipAvailabilityReport {
            status: if crate::refinement::jpip_range_complete(available_bytes, required_bytes) {
                JpipAvailability::Available
            } else if available_bytes == 0 {
                JpipAvailability::Missing
            } else {
                JpipAvailability::Partial
            },
            required_bytes,
            available_bytes,
            missing,
        })
    }

    pub fn transcode_precinct_codestream(
        &self,
        plan: &JpipPrecinctCodestreamPlan,
        layers: u16,
    ) -> Result<JpipPrecinctTranscode, DecodeError> {
        let packet_end = plan.packet_end_for_layers(layers)?;
        let availability = self.precinct_availability(plan, packet_end)?;
        if !availability.is_available() {
            return Ok(JpipPrecinctTranscode {
                layers,
                availability,
                codestream: None,
            });
        }
        let mut main_header = self.databin_view(plan.main_header)?.into_bytes();
        let mut tile_header = self.databin_view(plan.tile_header)?.into_bytes();
        validate_and_patch_main_header(&mut main_header, layers)?;
        validate_and_patch_tile_header(&mut tile_header, layers)?;
        let selected_packets = plan
            .packets
            .get(..packet_end)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let packet_bytes = self.read_packet_ranges(selected_packets)?;
        let tile_part_length = tile_header
            .len()
            .checked_add(packet_bytes.len())
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let tile_part_length = u32::try_from(tile_part_length)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let tile_part_bytes = [
            u8::try_from((tile_part_length >> 24) & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            u8::try_from((tile_part_length >> 16) & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            u8::try_from((tile_part_length >> 8) & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            u8::try_from(tile_part_length & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        ];
        tile_header
            .get_mut(6..10)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?
            .copy_from_slice(&tile_part_bytes);
        let total = main_header
            .len()
            .checked_add(tile_header.len())
            .and_then(|value| value.checked_add(packet_bytes.len()))
            .and_then(|value| value.checked_add(EOC.len()))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let contracted_total = crate::refinement::jpip_reconstruction_length(
            main_header.len() as u64,
            tile_header.len() as u64,
            packet_bytes.len() as u64,
            EOC.len() as u64,
        );
        if contracted_total != total as u128 {
            return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
        }
        if total as u64 > self.limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                self.limits.maximum_input_bytes(),
                total as u64,
            ));
        }
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(&main_header);
        bytes.extend_from_slice(&tile_header);
        bytes.extend_from_slice(&packet_bytes);
        bytes.extend_from_slice(&EOC);
        validate_precinct_codestream(&bytes, layers, &self.limits)?;
        Ok(JpipPrecinctTranscode {
            layers,
            availability,
            codestream: Some(JpipByteView { bytes }),
        })
    }

    pub fn reconstruct_precinct_packets(
        &self,
        plan: &JpipPrecinctCodestreamPlan,
        layers: u16,
    ) -> Result<JpipPrecinctPackets, DecodeError> {
        let packet_end = plan.packet_end_for_layers(layers)?;
        let selected_packets = plan
            .packets
            .get(..packet_end)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let ranges: Vec<JpipDatabinRange> =
            selected_packets.iter().map(|packet| packet.range).collect();
        let availability = self.availability(&ranges)?;
        let packets = if availability.is_available() {
            Some(JpipByteView {
                bytes: self.read_packet_ranges(selected_packets)?,
            })
        } else {
            None
        };
        Ok(JpipPrecinctPackets {
            layers,
            availability,
            packets,
        })
    }

    fn precinct_availability(
        &self,
        plan: &JpipPrecinctCodestreamPlan,
        packet_end: usize,
    ) -> Result<JpipAvailabilityReport, DecodeError> {
        let packets = plan
            .packets
            .get(..packet_end)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let ranges: Vec<JpipDatabinRange> = packets.iter().map(|packet| packet.range).collect();
        let mut report = self.availability(&ranges)?;
        self.append_complete_databin_requirement(plan.main_header, &mut report)?;
        self.append_complete_databin_requirement(plan.tile_header, &mut report)?;
        report.status = if report.missing.is_empty() {
            JpipAvailability::Available
        } else if report.available_bytes == 0 {
            JpipAvailability::Missing
        } else {
            JpipAvailability::Partial
        };
        Ok(report)
    }

    fn append_complete_databin_requirement(
        &self,
        id: DatabinId,
        report: &mut JpipAvailabilityReport,
    ) -> Result<(), DecodeError> {
        let Some(status) = self.status(id) else {
            report.missing.push(JpipMissingRange {
                databin: id,
                offset: 0,
                length: None,
            });
            return Ok(());
        };
        let Some(final_length) = status.final_length() else {
            report.available_bytes = report
                .available_bytes
                .checked_add(status.contiguous_prefix())
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            report.missing.push(JpipMissingRange {
                databin: id,
                offset: status.contiguous_prefix(),
                length: None,
            });
            return Ok(());
        };
        let header_report = if final_length == 0 {
            JpipAvailabilityReport {
                status: JpipAvailability::Available,
                required_bytes: 0,
                available_bytes: 0,
                missing: Vec::new(),
            }
        } else {
            self.availability(&[JpipDatabinRange::new(id, 0, final_length)?])?
        };
        report.required_bytes = report
            .required_bytes
            .checked_add(header_report.required_bytes)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        report.available_bytes = report
            .available_bytes
            .checked_add(header_report.available_bytes)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let mut missing_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant missing_index <= header_report.missing.len(),
                decreases header_report.missing.len() - missing_index,
            )
        )]
        while missing_index < header_report.missing.len() {
            report.missing.push(header_report.missing[missing_index]);
            missing_index += 1;
        }
        Ok(())
    }

    fn progressive_availability(
        &self,
        plan: &JpipPrecinctCodestreamPlan,
    ) -> Result<(u16, Vec<JpipMissingRange>), DecodeError> {
        let mut header_report = JpipAvailabilityReport {
            status: JpipAvailability::Available,
            required_bytes: 0,
            available_bytes: 0,
            missing: Vec::new(),
        };
        self.append_complete_databin_requirement(plan.main_header, &mut header_report)?;
        self.append_complete_databin_requirement(plan.tile_header, &mut header_report)?;
        if !header_report.missing.is_empty() {
            return Ok((0, header_report.missing));
        }
        let mut layer = 0_usize;
        let mut start = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant start <= plan.packets.len(),
                decreases plan.layer_ends.len() - layer,
            )
        )]
        while layer < plan.layer_ends.len() {
            let end = *plan
                .layer_ends
                .get(layer)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            let packet_layer = plan
                .packets
                .get(start..end)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            let ranges: Vec<JpipDatabinRange> =
                packet_layer.iter().map(|packet| packet.range).collect();
            let report = self.availability(&ranges)?;
            if !report.is_available() {
                return Ok((
                    u16::try_from(layer).map_err(|_| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?,
                    report.missing,
                ));
            }
            start = end;
            layer += 1;
        }
        Ok((
            u16::try_from(layer)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            Vec::new(),
        ))
    }

    fn read_packet_ranges(&self, packets: &[JpipPrecinctPacket]) -> Result<Vec<u8>, DecodeError> {
        let mut total = 0_u64;
        let mut packet_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant packet_index <= packets.len(),
                decreases packets.len() - packet_index,
            )
        )]
        while packet_index < packets.len() {
            total = total
                .checked_add(packets[packet_index].range.length)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            packet_index += 1;
        }
        if total > self.limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                self.limits.maximum_input_bytes(),
                total,
            ));
        }
        let mut output = Vec::with_capacity(
            usize::try_from(total)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        packet_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant packet_index <= packets.len(),
                decreases packets.len() - packet_index,
            )
        )]
        while packet_index < packets.len() {
            let range = packets[packet_index].range;
            let databin_index = self.find_databin_index(range.databin).ok_or_else(|| {
                DecodeError::malformed(range.offset, MalformedKind::MissingRequiredStructure)
            })?;
            let databin = self.databins.get(databin_index).ok_or_else(|| {
                DecodeError::malformed(range.offset, MalformedKind::ArithmeticOverflow)
            })?;
            let end = range.offset.checked_add(range.length).ok_or_else(|| {
                DecodeError::malformed(range.offset, MalformedKind::ArithmeticOverflow)
            })?;
            let mut cursor = range.offset;
            let mut fragment_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant fragment_index <= databin.fragments.len(),
                    decreases databin.fragments.len() - fragment_index,
                )
            )]
            while fragment_index < databin.fragments.len() && cursor < end {
                let fragment = &databin.fragments[fragment_index];
                if fragment.end() <= cursor {
                    fragment_index += 1;
                    continue;
                }
                if fragment.offset > cursor {
                    return Err(DecodeError::malformed(
                        cursor,
                        MalformedKind::MissingRequiredStructure,
                    ));
                }
                let copy_end = fragment.end().min(end);
                let local_start = usize::try_from(cursor - fragment.offset).map_err(|_| {
                    DecodeError::malformed(cursor, MalformedKind::ArithmeticOverflow)
                })?;
                let local_end =
                    usize::try_from(copy_end.checked_sub(fragment.offset).ok_or_else(|| {
                        DecodeError::malformed(cursor, MalformedKind::ArithmeticOverflow)
                    })?)
                    .map_err(|_| {
                        DecodeError::malformed(cursor, MalformedKind::ArithmeticOverflow)
                    })?;
                output.extend_from_slice(fragment.bytes.get(local_start..local_end).ok_or_else(
                    || DecodeError::malformed(cursor, MalformedKind::ArithmeticOverflow),
                )?);
                cursor = copy_end;
                fragment_index += 1;
            }
            if cursor != end {
                return Err(DecodeError::malformed(
                    cursor,
                    MalformedKind::MissingRequiredStructure,
                ));
            }
            packet_index += 1;
        }
        Ok(output)
    }

    fn find_databin_index(&self, id: DatabinId) -> Option<usize> {
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.databins.len(),
                decreases self.databins.len() - index,
            )
        )]
        while index < self.databins.len() {
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn reconstruct(&self, layout: &[DatabinId]) -> Result<JpipByteView, DecodeError> {
        let mut total = 0_u64;
        let mut parts = Vec::with_capacity(layout.len());
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= layout.len(),
                decreases layout.len() - index,
            )
        )]
        while index < layout.len() {
            let view = self.databin_view(layout[index])?;
            total = total
                .checked_add(view.bytes.len() as u64)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if total > self.limits.maximum_input_bytes() {
                return Err(DecodeError::limit(
                    Resource::InputBytes,
                    self.limits.maximum_input_bytes(),
                    total,
                ));
            }
            parts.push(view);
            index += 1;
        }
        let contracted_total = crate::refinement::jpip_reconstruction_length(total, 0, 0, 0);
        let capacity = usize::try_from(contracted_total)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let mut bytes = Vec::with_capacity(capacity);
        let mut part_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant part_index <= parts.len(),
                decreases parts.len() - part_index,
            )
        )]
        while part_index < parts.len() {
            bytes.extend_from_slice(&parts[part_index].bytes);
            part_index += 1;
        }
        Ok(JpipByteView { bytes })
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct JpipResponseSummary {
    messages: u64,
    end_of_response_reason: Option<u8>,
}

impl JpipResponseSummary {
    pub const fn messages(&self) -> u64 {
        self.messages
    }

    pub const fn end_of_response_reason(&self) -> Option<u8> {
        self.end_of_response_reason
    }
}

pub fn ingest_jpip_response(
    input: &[u8],
    cache: &mut DatabinCache,
) -> Result<JpipResponseSummary, DecodeError> {
    let mut parser = JpipMessageParser::new(cache.limits);
    let mut position = 0_usize;
    let mut messages = 0_u64;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= input.len(),
            decreases input.len() - position,
        )
    )]
    while position < input.len() {
        let (message, consumed) = parser.parse(&input[position..])?;
        if consumed == 0 {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidJpipHeader,
            ));
        }
        match message {
            JpipMessage::Data { header, body } => {
                let fragment = JpipFragment::from_message(header, body, &cache.limits)?;
                cache.insert(fragment)?;
                messages = messages.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?;
            }
            JpipMessage::EndOfResponse(end) => {
                let response_end = position.checked_add(consumed).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?;
                if response_end != input.len() {
                    return Err(DecodeError::malformed(
                        response_end as u64,
                        MalformedKind::TrailingData,
                    ));
                }
                return Ok(JpipResponseSummary {
                    messages,
                    end_of_response_reason: Some(end.reason_code),
                });
            }
        }
        let next_position = position.checked_add(consumed).ok_or_else(|| {
            DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
        })?;
        if next_position <= position || next_position > input.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidJpipHeader,
            ));
        }
        position = next_position;
    }
    Ok(JpipResponseSummary {
        messages,
        end_of_response_reason: None,
    })
}

const fn conflict(offset: u64) -> DecodeError {
    DecodeError::malformed(offset, MalformedKind::ConflictingJpipFragment)
}

fn copy_bytes(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= input.len(),
            decreases input.len() - index,
        )
    )]
    while index < input.len() {
        output.push(input[index]);
        index += 1;
    }
    output
}

fn copy_into(
    output: &mut [u8],
    output_start: usize,
    input: &[u8],
    error_offset: u64,
) -> Result<(), DecodeError> {
    let output_end = output_start
        .checked_add(input.len())
        .ok_or_else(|| DecodeError::malformed(error_offset, MalformedKind::ArithmeticOverflow))?;
    let target = output
        .get_mut(output_start..output_end)
        .ok_or_else(|| DecodeError::malformed(error_offset, MalformedKind::ArithmeticOverflow))?;
    target.copy_from_slice(input);
    Ok(())
}

fn validate_and_patch_main_header(header: &mut [u8], layers: u16) -> Result<(), DecodeError> {
    if !header.starts_with(&SOC) {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    patch_cod_layers(header, 2, layers, false)?;
    Ok(())
}

fn validate_and_patch_tile_header(header: &mut [u8], layers: u16) -> Result<(), DecodeError> {
    if header.len() < 14
        || header.get(0..2) != Some(&[0xff, 0x90])
        || header.get(2..4) != Some(&[0, 10])
        || !matches!(header.get(10), Some(0))
        || !matches!(header.get(11), Some(0 | 1))
        || !header.ends_with(&[0xff, 0x93])
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
    }
    patch_cod_layers(header, 0, layers, true)?;
    Ok(())
}

fn patch_cod_layers(
    header: &mut [u8],
    start: usize,
    layers: u16,
    stop_at_sod: bool,
) -> Result<(), DecodeError> {
    if start > header.len() {
        return Err(DecodeError::malformed(
            start as u64,
            MalformedKind::Truncated,
        ));
    }
    let mut position = start;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= header.len(),
            decreases header.len() - position,
        )
    )]
    while position < header.len() {
        let marker_bytes = header
            .get(position..position.saturating_add(2))
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let marker_high = *marker_bytes
            .first()
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let marker_low = *marker_bytes
            .get(1)
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let marker = (u16::from(marker_high) << 8) | u16::from(marker_low);
        if marker == 0xff93 {
            if stop_at_sod && position + 2 == header.len() {
                return Ok(());
            }
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        if marker == 0xff4f || marker == 0xffd9 || marker == 0xff90 && position != 0 {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        let length_bytes = header
            .get(position.saturating_add(2)..position.saturating_add(4))
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let length_high = *length_bytes
            .first()
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let length_low = *length_bytes
            .get(1)
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let segment_length = usize::from((u16::from(length_high) << 8) | u16::from(length_low));
        if segment_length < 2 {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let segment_end = position
            .checked_add(2)
            .and_then(|value| value.checked_add(segment_length))
            .ok_or_else(|| {
                DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
            })?;
        if segment_end <= position || segment_end > header.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::Truncated,
            ));
        }
        if marker == 0xff52 {
            if segment_length < 7 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidCodingStyle,
                ));
            }
            let bytes = [
                u8::try_from((layers >> 8) & 0xff).map_err(|_| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?,
                u8::try_from(layers & 0xff).map_err(|_| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?,
            ];
            let layer_start = position.checked_add(6).ok_or_else(|| {
                DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
            })?;
            let layer_end = layer_start.checked_add(2).ok_or_else(|| {
                DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
            })?;
            header
                .get_mut(layer_start..layer_end)
                .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?
                .copy_from_slice(&bytes);
        }
        position = segment_end;
    }
    if stop_at_sod {
        return Err(DecodeError::malformed(
            position as u64,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    Ok(())
}

fn validate_precinct_codestream(
    bytes: &[u8],
    layers: u16,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let layout = crate::codestream::parse_layout(bytes, limits)?;
    if layout.info.image_size.tile_count()? != 1
        || layout.tile_parts.len() != 1
        || layout.info.coding_style.layers != layers
        || !matches!(
            layout.info.coding_style.progression_order,
            crate::codestream::ProgressionOrder::Lrcp
        )
        || layout.info.progression_changes.is_present()
        || layout.tile_parts[0].progression_changes.is_present()
        || layout.tile_parts[0].packet_lengths.is_present()
        || layout.tile_parts[0].packed_packet_headers.is_present()
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
    }
    Ok(())
}

struct JpipPlaceholder {
    flags: u32,
    bin: u64,
    original_header: Vec<u8>,
    original_type: crate::jp2::BoxType,
    equivalent_bin: Option<u64>,
    equivalent_header: Option<Vec<u8>>,
    equivalent_type: Option<crate::jp2::BoxType>,
    stream: Option<u64>,
    stream_count: u32,
}

fn parse_jpip_placeholder(payload: &[u8]) -> Result<JpipPlaceholder, DecodeError> {
    if payload.len() < 20 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
    let flags = read_u32_be(payload, 0)?;
    let bin_high = u64::from(read_u32_be(payload, 4)?);
    let bin_low = u64::from(read_u32_be(payload, 8)?);
    let bin = (bin_high << 32) | bin_low;
    let original_start = 12_usize;
    let first_length = read_u32_be(payload, original_start)?;
    let header_length = if first_length == 1 { 16_usize } else { 8_usize };
    let original_end = original_start
        .checked_add(header_length)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let original = payload
        .get(original_start..original_end)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?;
    let box_type = crate::jp2::BoxType::new([
        *original
            .get(4)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?,
        *original
            .get(5)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?,
        *original
            .get(6)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?,
        *original
            .get(7)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?,
    ]);
    if flags & !0x0f != 0 || flags & 8 != 0 && flags & 4 == 0 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
    let mut position = original_end;
    let (equivalent_bin, equivalent_header, equivalent_type) = if flags & 6 != 0 {
        let bin = read_u64_be(payload, position)?;
        position = position
            .checked_add(8)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let length = read_u32_be(payload, position)?;
        let header_length = if length == 1 { 16_usize } else { 8_usize };
        let end = position
            .checked_add(header_length)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let header = payload
            .get(position..end)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?;
        let box_type = crate::jp2::BoxType::new([header[4], header[5], header[6], header[7]]);
        position = end;
        (Some(bin), Some(copy_bytes(header)), Some(box_type))
    } else {
        (None, None, None)
    };
    let (stream, stream_count) = if flags & 4 != 0 {
        let stream = read_u64_be(payload, position)?;
        position = position
            .checked_add(8)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let count = if flags & 8 != 0 {
            let count = read_u32_be(payload, position)?;
            position = position
                .checked_add(4)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if count == 0 {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
            }
            count
        } else {
            1
        };
        (Some(stream), count)
    } else {
        (None, 0)
    };
    if position != payload.len() {
        if flags == 4 && payload.len() == original_end + 24 {
            // Compatibility with the previously selected provisional layout.
            return Err(DecodeError::malformed(
                original_end as u64,
                MalformedKind::InvalidBoxContents,
            ));
        } else {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
        }
    }
    Ok(JpipPlaceholder {
        flags,
        bin,
        original_header: copy_bytes(original),
        original_type: box_type,
        equivalent_bin,
        equivalent_header,
        equivalent_type,
        stream,
        stream_count,
    })
}

fn parse_tile_part_transitions(
    input: &[u8],
    expected_tile: u16,
) -> Result<Vec<JpipTilePartTransition>, DecodeError> {
    if !input.starts_with(&[0xff, 0x90]) {
        let mut functional = false;
        let mut packet_count = None;
        let mut position = 0_usize;
        let mut remaining_markers = input.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_markers,)
        )]
        while position < input.len() {
            if remaining_markers == 0 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_markers -= 1;
            if input.get(position..position.saturating_add(2)) == Some(&[0xff, 0x93]) {
                break;
            }
            let (marker, payload_start, end) = marker_segment(input, position)?;
            if marker == 0xff58 {
                let count = count_plt_packets(input.get(payload_start..end).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::Truncated)
                })?)?;
                packet_count = Some(
                    packet_count
                        .unwrap_or(0_u64)
                        .checked_add(count)
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?,
                );
            } else if !matches!(marker, 0xff61 | 0xff64) {
                functional = true;
            }
            position = end;
        }
        return Ok(vec![JpipTilePartTransition {
            tile: expected_tile,
            tile_part: 0,
            packet_count,
            data_length: None,
            functional,
            header: copy_bytes(input),
        }]);
    }
    let mut transitions = Vec::new();
    let mut position = 0_usize;
    let mut remaining_parts = input.len();
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases remaining_parts,)
    )]
    while position < input.len() {
        if remaining_parts == 0 {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidTilePart,
            ));
        }
        remaining_parts -= 1;
        let part_start = position;
        let (marker, _, sot_end) = marker_segment(input, position)?;
        if marker != 0xff90 || sot_end.checked_sub(position) != Some(12) {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidTilePart,
            ));
        }
        let tile = read_u16_be(input, position + 4)?;
        if tile != expected_tile {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidTilePart,
            ));
        }
        let tile_part = *input
            .get(position + 10)
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
        let tile_part_length = read_u32_be(input, position + 6)?;
        position = sot_end;
        let mut functional = false;
        let mut packet_count = None;
        let mut next_plt = 0_u16;
        let mut found_sod = false;
        let mut remaining_headers = input.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_headers,)
        )]
        while position < input.len() {
            if remaining_headers == 0 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_headers -= 1;
            if input.get(position..position.saturating_add(2)) == Some(&[0xff, 0x93]) {
                position = position
                    .checked_add(2)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                found_sod = true;
                break;
            }
            let (header_marker, payload_start, end) = marker_segment(input, position)?;
            if header_marker == 0xff58 {
                let payload = input.get(payload_start..end).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::Truncated)
                })?;
                let index = *payload.first().ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::InvalidPacketLength)
                })?;
                if u16::from(index) != next_plt {
                    return Err(DecodeError::malformed(
                        position as u64,
                        MalformedKind::InvalidPacketLength,
                    ));
                }
                next_plt = next_plt
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                let count = count_plt_packets(payload)?;
                packet_count = Some(
                    packet_count
                        .unwrap_or(0_u64)
                        .checked_add(count)
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                        })?,
                );
            } else if !matches!(header_marker, 0xff61 | 0xff64) {
                functional = true;
            }
            position = end;
        }
        if !found_sod {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        if tile_part_length == 0 && position < input.len() {
            return Err(DecodeError::malformed(
                part_start as u64,
                MalformedKind::InvalidTilePart,
            ));
        }
        let header_length = position.checked_sub(part_start).ok_or_else(|| {
            DecodeError::malformed(part_start as u64, MalformedKind::ArithmeticOverflow)
        })?;
        let data_length = if tile_part_length == 0 {
            None
        } else {
            let declared = usize::try_from(tile_part_length).map_err(|_| {
                DecodeError::malformed(part_start as u64, MalformedKind::ArithmeticOverflow)
            })?;
            declared.checked_sub(header_length).map(|data| data as u64)
        };
        transitions.push(JpipTilePartTransition {
            tile,
            tile_part,
            packet_count,
            data_length,
            functional,
            header: copy_bytes(input.get(part_start..position).ok_or_else(|| {
                DecodeError::malformed(part_start as u64, MalformedKind::Truncated)
            })?),
        });
    }
    Ok(transitions)
}

fn count_plt_packets(payload: &[u8]) -> Result<u64, DecodeError> {
    if payload.is_empty() {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidPacketLength,
        ));
    }
    let mut position = 1_usize;
    let mut count = 0_u64;
    let mut continuation = false;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= payload.len(),
            decreases payload.len() - position,
        )
    )]
    while position < payload.len() {
        let byte = payload[position];
        continuation = byte & 0x80 != 0;
        if !continuation {
            count = count
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        position += 1;
    }
    if continuation {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidPacketLength,
        ));
    }
    Ok(count)
}

fn jpip_superbox(box_type: crate::jp2::BoxType) -> bool {
    [
        *b"jp2h", *b"jpch", *b"jplh", *b"cgrp", *b"res ", *b"asoc", *b"comp",
    ]
    .iter()
    .any(|value| box_type.equals(crate::jp2::BoxType::new(*value)))
}

fn append_reconstructed_box(
    output: &mut Vec<u8>,
    original_header: &[u8],
    payload: &[u8],
) -> Result<(), DecodeError> {
    if original_header.len() != 8 && original_header.len() != 16 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
    let total = original_header
        .len()
        .checked_add(payload.len())
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    if crate::refinement::jpip_box_length(original_header.len() as u64, payload.len() as u64)
        != total as u128
    {
        return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
    }
    let mut header = copy_bytes(original_header);
    if header.len() == 8 {
        let length = u32::try_from(total)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        write_u32_be(&mut header, 0, length)?;
    } else {
        write_u32_be(&mut header, 0, 1)?;
        write_u64_be(&mut header, 8, total as u64)?;
    }
    output.extend_from_slice(&header);
    output.extend_from_slice(payload);
    Ok(())
}

fn append_missing_ranges(output: &mut Vec<JpipMissingRange>, input: &[JpipMissingRange]) {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= input.len(),
            decreases input.len() - index,
        )
    )]
    while index < input.len() {
        output.push(input[index]);
        index += 1;
    }
}

fn write_u64_be(output: &mut [u8], position: usize, value: u64) -> Result<(), DecodeError> {
    let high = u32::try_from(value >> 32)
        .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let low = u32::try_from(value & u64::from(u32::MAX))
        .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    write_u32_be(output, position, high)?;
    write_u32_be(
        output,
        position
            .checked_add(4)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        low,
    )
}

fn siz_cardinality(main_header: &[u8], limits: &ResourceLimits) -> Result<(u64, u16), DecodeError> {
    if !main_header.starts_with(&SOC) {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    let mut position = 2_usize;
    let mut remaining_markers = main_header.len();
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases remaining_markers,)
    )]
    while position < main_header.len() {
        if remaining_markers == 0 {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidMarkerLength,
            ));
        }
        remaining_markers -= 1;
        let (marker, payload_start, end) = marker_segment(main_header, position)?;
        if marker == 0xff51 {
            let payload = main_header
                .get(payload_start..end)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::Truncated))?;
            if payload.len() < 36 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidImageGeometry,
                ));
            }
            let x_size = read_u32_be(payload, 2)?;
            let y_size = read_u32_be(payload, 6)?;
            let tile_width = read_u32_be(payload, 18)?;
            let tile_height = read_u32_be(payload, 22)?;
            let tile_x = read_u32_be(payload, 26)?;
            let tile_y = read_u32_be(payload, 30)?;
            let components = read_u16_be(payload, 34)?;
            if components == 0
                || components > limits.maximum_components()
                || tile_width == 0
                || tile_height == 0
                || x_size <= tile_x
                || y_size <= tile_y
            {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidImageGeometry,
                ));
            }
            let tiles_x = ceil_div_jpip(u64::from(x_size - tile_x), u64::from(tile_width))?;
            let tiles_y = ceil_div_jpip(u64::from(y_size - tile_y), u64::from(tile_height))?;
            let tiles = tiles_x
                .checked_mul(tiles_y)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if tiles == 0 || tiles > u64::from(limits.maximum_tiles()) {
                return Err(DecodeError::limit(
                    Resource::Tiles,
                    u64::from(limits.maximum_tiles()),
                    tiles,
                ));
            }
            return Ok((tiles, components));
        }
        position = end;
    }
    Err(DecodeError::malformed(
        0,
        MalformedKind::MissingRequiredStructure,
    ))
}

fn normalize_main_header(input: &[u8]) -> Result<Vec<u8>, DecodeError> {
    if !input.starts_with(&SOC) {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    let mut output = vec![SOC[0], SOC[1]];
    let mut position = 2_usize;
    let mut remaining_markers = input.len();
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases remaining_markers,)
    )]
    while position < input.len() {
        if remaining_markers == 0 {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidMarkerLength,
            ));
        }
        remaining_markers -= 1;
        let (marker, _, end) = marker_segment(input, position)?;
        if matches!(marker, 0xff90 | 0xff93 | 0xffd9) {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        if !crate::refinement::jpip_reframed_marker_removed(marker, false) {
            output.extend_from_slice(input.get(position..end).ok_or_else(|| {
                DecodeError::malformed(position as u64, MalformedKind::Truncated)
            })?);
        }
        position = end;
    }
    Ok(output)
}

fn normalize_automatic_tile_headers(input: &[u8], tile: u16) -> Result<Vec<Vec<u8>>, DecodeError> {
    let transitions = parse_tile_part_transitions(input, tile)?;
    let count = u8::try_from(transitions.len())
        .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let mut output = Vec::with_capacity(transitions.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= transitions.len(),
            decreases transitions.len() - index,
        )
    )]
    while index < transitions.len() {
        let transition = transitions
            .get(index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        let source = transition.header_bytes();
        let marker_start = if source.starts_with(&[0xff, 0x90]) {
            let (marker, _, end) = marker_segment(source, 0)?;
            if marker != 0xff90 || end != 12 {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidTilePart));
            }
            end
        } else {
            0
        };
        let mut header = vec![0xff, 0x90, 0, 10];
        header.push(
            u8::try_from((tile >> 8) & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        header.push(
            u8::try_from(tile & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        header.extend_from_slice(&[0, 0, 0, 0]);
        header.push(
            u8::try_from(index)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        header.push(count);
        let mut position = marker_start;
        let mut found_sod = false;
        let mut remaining_markers = source.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_markers,)
        )]
        while position < source.len() {
            if remaining_markers == 0 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_markers -= 1;
            if source.get(position..position.saturating_add(2)) == Some(&[0xff, 0x93]) {
                position = position.checked_add(2).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?;
                found_sod = true;
                break;
            }
            let (marker, _, end) = marker_segment(source, position)?;
            if !crate::refinement::jpip_reframed_marker_removed(marker, true) {
                header.extend_from_slice(source.get(position..end).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::Truncated)
                })?);
            }
            position = end;
        }
        if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        header.extend_from_slice(&[0xff, 0x93]);
        output.push(header);
        index += 1;
    }
    Ok(output)
}

fn partition_automatic_tile_parts(
    headers: &[Vec<u8>],
    states: &[&crate::codestream::TilePartPayload<'_>],
    transitions: &[JpipTilePartTransition],
    packets: &[AutomaticPacket],
) -> Result<Option<Vec<AutomaticTilePart>>, DecodeError> {
    if headers.len() != states.len() || headers.len() != transitions.len() || headers.is_empty() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidTilePart));
    }
    let mut output = Vec::with_capacity(headers.len());
    let mut packet_start = 0_usize;
    let mut part = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant part <= headers.len(),
            decreases headers.len() - part,
        )
    )]
    while part < headers.len() {
        let transition = transitions
            .get(part)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        let mut packet_end = packet_start;
        if part + 1 == headers.len() {
            packet_end = packets.len();
        } else if let Some(count) = transition.packet_count() {
            let count = usize::try_from(count)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            packet_end = packet_start
                .checked_add(count)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            if packet_end > packets.len() {
                return Err(DecodeError::malformed(
                    0,
                    MalformedKind::InvalidPacketLength,
                ));
            }
        } else if let Some(target) = transition.data_length() {
            let mut consumed = 0_u64;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(decreases packets.len() - packet_end,)
            )]
            while packet_end < packets.len() && consumed < target {
                let packet = packets
                    .get(packet_end)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidPacketLength))?;
                let Some(bytes) = packet.bytes.as_ref() else {
                    return Ok(None);
                };
                consumed = consumed
                    .checked_add(bytes.len() as u64)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                packet_end += 1;
            }
            if consumed != target {
                return Err(DecodeError::malformed(
                    consumed,
                    MalformedKind::InvalidPacketLength,
                ));
            }
        } else {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTilePart));
        }
        let state = states
            .get(part)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        output.push(AutomaticTilePart {
            header: headers
                .get(part)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?
                .clone(),
            packets: packets
                .get(packet_start..packet_end)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidPacketLength))?
                .iter()
                .map(|packet| AutomaticPacket {
                    databin: packet.databin,
                    bytes: packet.bytes.clone(),
                    next_offset: packet.next_offset,
                    required: packet.required,
                    ordinal: packet.ordinal,
                    layer: packet.layer,
                })
                .collect(),
            sop: state.coding_style.sop_markers,
            eph: state.coding_style.eph_markers,
        });
        packet_start = packet_end;
        part += 1;
    }
    if packet_start != packets.len() {
        return Err(DecodeError::malformed(
            packet_start as u64,
            MalformedKind::InvalidPacketLength,
        ));
    }
    Ok(Some(output))
}

fn build_automatic_transition_template(
    main: &[u8],
    tile_headers: &[Vec<Vec<u8>>],
    limits: &ResourceLimits,
) -> Result<Vec<u8>, DecodeError> {
    let mut output = copy_bytes(main);
    let mut tile = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant tile <= tile_headers.len(),
            decreases tile_headers.len() - tile,
        )
    )]
    while tile < tile_headers.len() {
        let parts = tile_headers
            .get(tile)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        let mut part = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant part <= parts.len(),
                decreases parts.len() - part,
            )
        )]
        while part < parts.len() {
            let start = output.len();
            output.extend_from_slice(
                parts
                    .get(part)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?,
            );
            let length = u32::try_from(output.len() - start)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            let length_position = start
                .checked_add(6)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            write_u32_be(&mut output, length_position, length)?;
            part += 1;
        }
        tile += 1;
    }
    output.extend_from_slice(&EOC);
    if output.len() as u64 > limits.maximum_input_bytes() {
        return Err(DecodeError::limit(
            Resource::InputBytes,
            limits.maximum_input_bytes(),
            output.len() as u64,
        ));
    }
    Ok(output)
}

fn build_automatic_template(
    main: &[u8],
    tile_headers: &[Vec<u8>],
    limits: &ResourceLimits,
) -> Result<Vec<u8>, DecodeError> {
    let mut output = copy_bytes(main);
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= tile_headers.len(),
            decreases tile_headers.len() - index,
        )
    )]
    while index < tile_headers.len() {
        let start = output.len();
        output.extend_from_slice(&tile_headers[index]);
        let length = u32::try_from(output.len() - start)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let length_position = start
            .checked_add(6)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        write_u32_be(&mut output, length_position, length)?;
        index += 1;
    }
    output.extend_from_slice(&EOC);
    if output.len() as u64 > limits.maximum_input_bytes() {
        return Err(DecodeError::limit(
            Resource::InputBytes,
            limits.maximum_input_bytes(),
            output.len() as u64,
        ));
    }
    Ok(output)
}

fn automatic_tile_part<'a>(
    tile_parts: &'a [crate::codestream::TilePartPayload<'a>],
    tile: u16,
) -> Result<&'a crate::codestream::TilePartPayload<'a>, DecodeError> {
    let mut found = None;
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= tile_parts.len(),
            decreases tile_parts.len() - index,
        )
    )]
    while index < tile_parts.len() {
        if tile_parts[index].tile_index == tile {
            if found.is_some() {
                return Err(DecodeError::AmbiguousJpipMapping {
                    reason: crate::JpipMappingAmbiguity::TilePartBoundary,
                });
            }
            found = Some(&tile_parts[index]);
        }
        index += 1;
    }
    found.ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))
}

#[allow(clippy::too_many_arguments)]
fn automatic_precinct_in_class(
    tile_count: u64,
    components: u64,
    tile: u64,
    component: u16,
    resolution: u8,
    precinct: u32,
    grid: &crate::packet::PacketGrid,
) -> Result<u64, DecodeError> {
    if tile_count == 0
        || components == 0
        || tile >= tile_count
        || u64::from(component) >= components
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
    }
    let mut level_start = 0_u64;
    let mut level = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant level <= resolution,
            decreases resolution - level,
        )
    )]
    while level < resolution {
        level_start = level_start
            .checked_add(u64::from(grid.precinct_count(component, level)?))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        level += 1;
    }
    let precinct_index = level_start
        .checked_add(u64::from(precinct))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let without_tile = u64::from(component)
        .checked_add(
            precinct_index
                .checked_mul(components)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        )
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let product = without_tile
        .checked_mul(tile_count)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let checked = tile
        .checked_add(product)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    if without_tile > (u64::MAX - tile) / tile_count {
        return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
    }
    let contracted =
        crate::refinement::jpip_precinct_in_class(tile, tile_count, without_tile, product);
    if checked != contracted {
        return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
    }
    Ok(checked)
}

fn validate_packet_component_selection(
    selection: &JpipPacketSelection,
    components: u16,
) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= selection.components.len(),
            decreases selection.components.len() - index,
        )
    )]
    while index < selection.components.len() {
        let component = selection.components[index];
        let mut duplicate = false;
        let mut previous = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant previous <= index,
                decreases index - previous,
            )
        )]
        while previous < index {
            if *selection
                .components
                .get(previous)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
                == component
            {
                duplicate = true;
                break;
            }
            previous += 1;
        }
        if component >= components || duplicate {
            return Err(DecodeError::InvalidSelection {
                reason: crate::SelectionError::EmptyComponentIntersection,
            });
        }
        index += 1;
    }
    Ok(())
}

fn jpip_tile_selected(selection: Option<&crate::DecodeSelection>, tile: u16) -> bool {
    let Some(selection) = selection else {
        return true;
    };
    match selection.tile_selection() {
        crate::TileSelection::All => true,
        crate::TileSelection::Index(selected) => *selected == tile,
        crate::TileSelection::Set(selected) => {
            let mut index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= selected.len(),
                    decreases selected.len() - index,
                )
            )]
            while index < selected.len() {
                if selected[index] == tile {
                    return true;
                }
                index += 1;
            }
            false
        }
    }
}

fn jpip_resolution_selected(
    selection: Option<&crate::DecodeSelection>,
    component: u16,
    resolution: u8,
    styles: &crate::codestream::ComponentCodingStyles,
) -> Result<bool, DecodeError> {
    let discard = match selection {
        Some(selection) => selection.discard_levels(),
        None => 0,
    };
    let count = styles
        .style(component)?
        .decomposition_levels
        .checked_add(1)
        .and_then(|value| value.checked_sub(discard))
        .ok_or(DecodeError::InvalidSelection {
            reason: crate::SelectionError::ReductionTooLarge,
        })?;
    Ok(resolution < count)
}

fn append_empty_packet(
    output: &mut Vec<u8>,
    sop: bool,
    eph: bool,
    sequence: u16,
) -> Result<(), DecodeError> {
    let expected = crate::refinement::jpip_packet_reframed_length(1, sop, eph);
    let start = output.len();
    if sop {
        output.extend_from_slice(&[0xff, 0x91, 0, 4]);
        output.push(
            u8::try_from((sequence >> 8) & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        output.push(
            u8::try_from(sequence & 0xff)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
    }
    output.push(0);
    if eph {
        output.extend_from_slice(&[0xff, 0x92]);
    }
    if output.len().checked_sub(start) != Some(expected) {
        return Err(DecodeError::malformed(
            start as u64,
            MalformedKind::ArithmeticOverflow,
        ));
    }
    Ok(())
}

fn marker_segment(input: &[u8], position: usize) -> Result<(u16, usize, usize), DecodeError> {
    let marker = read_u16_be(input, position)?;
    if matches!(marker, 0xff4f | 0xff93 | 0xffd9) {
        let end = position.checked_add(2).ok_or_else(|| {
            DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
        })?;
        return Ok((marker, end, end));
    }
    let length_position = position.checked_add(2).ok_or_else(|| {
        DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
    })?;
    let length = usize::from(read_u16_be(input, length_position)?);
    if length < 2 {
        return Err(DecodeError::malformed(
            position as u64,
            MalformedKind::InvalidMarkerLength,
        ));
    }
    let end = position
        .checked_add(2)
        .and_then(|value| value.checked_add(length))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    if end > input.len() {
        return Err(DecodeError::malformed(
            position as u64,
            MalformedKind::Truncated,
        ));
    }
    let payload = position.checked_add(4).ok_or_else(|| {
        DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
    })?;
    Ok((marker, payload, end))
}

fn ceil_div_jpip(value: u64, divisor: u64) -> Result<u64, DecodeError> {
    if divisor == 0 {
        return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
    }
    value
        .checked_add(divisor - 1)
        .and_then(|sum| sum.checked_div(divisor))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
}

fn read_u64_be(input: &[u8], position: usize) -> Result<u64, DecodeError> {
    let high = u64::from(read_u32_be(input, position)?);
    let low = u64::from(read_u32_be(
        input,
        position
            .checked_add(4)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
    )?);
    Ok((high << 32) | low)
}

fn read_u16_be(input: &[u8], position: usize) -> Result<u16, DecodeError> {
    let bytes = input
        .get(position..position.saturating_add(2))
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let high = *bytes
        .first()
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let low = *bytes
        .get(1)
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    Ok((u16::from(high) << 8) | u16::from(low))
}

fn read_u32_be(input: &[u8], position: usize) -> Result<u32, DecodeError> {
    let bytes = input
        .get(position..position.saturating_add(4))
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let first = *bytes
        .first()
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let second = *bytes
        .get(1)
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let third = *bytes
        .get(2)
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let fourth = *bytes
        .get(3)
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    Ok((u32::from(first) << 24)
        | (u32::from(second) << 16)
        | (u32::from(third) << 8)
        | u32::from(fourth))
}

fn write_u32_be(output: &mut [u8], position: usize, value: u32) -> Result<(), DecodeError> {
    let bytes = output
        .get_mut(position..position.saturating_add(4))
        .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))?;
    let source = [
        u8::try_from((value >> 24) & 0xff)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        u8::try_from((value >> 16) & 0xff)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        u8::try_from((value >> 8) & 0xff)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        u8::try_from(value & 0xff)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
    ];
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= source.len(),
            decreases source.len() - index,
        )
    )]
    while index < source.len() {
        *bytes
            .get_mut(index)
            .ok_or_else(|| DecodeError::malformed(position as u64, MalformedKind::Truncated))? =
            source[index];
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(class: u64, in_class: u64) -> DatabinId {
        DatabinId::new(class, 0, in_class).unwrap()
    }

    fn fragment(databin: DatabinId, offset: u64, bytes: &[u8], final_byte: bool) -> JpipFragment {
        JpipFragment::new(
            databin,
            offset,
            bytes.to_vec(),
            final_byte,
            None,
            &ResourceLimits::default(),
        )
        .unwrap()
    }

    fn segment(marker: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = marker.to_be_bytes().to_vec();
        bytes.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn box32(box_type: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = u32::try_from(payload.len() + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        bytes.extend_from_slice(&box_type);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn metadata_placeholder(
        box_type: [u8; 4],
        original_length: u32,
        flags: u32,
        bin: u64,
        stream: Option<u64>,
    ) -> Vec<u8> {
        let mut payload = flags.to_be_bytes().to_vec();
        payload.extend_from_slice(&(bin >> 32).to_be_bytes()[4..]);
        payload.extend_from_slice(&(bin as u32).to_be_bytes());
        payload.extend_from_slice(&original_length.to_be_bytes());
        payload.extend_from_slice(&box_type);
        if let Some(stream) = stream {
            payload.extend_from_slice(&0_u64.to_be_bytes());
            payload.extend_from_slice(&original_length.to_be_bytes());
            payload.extend_from_slice(&box_type);
            payload.extend_from_slice(&stream.to_be_bytes());
        }
        box32(*b"phld", &payload)
    }

    #[test]
    fn placeholder_parser_accepts_standard_bit_flags_and_types_extensions() {
        let original = [0, 0, 0, 8, b'j', b'p', b'2', b'c'];
        let equivalent = [0, 0, 0, 8, b'j', b'p', b'2', b'c'];
        let mut payload = 6_u32.to_be_bytes().to_vec();
        payload.extend_from_slice(&3_u64.to_be_bytes());
        payload.extend_from_slice(&original);
        payload.extend_from_slice(&4_u64.to_be_bytes());
        payload.extend_from_slice(&equivalent);
        payload.extend_from_slice(&9_u64.to_be_bytes());
        let placeholder = parse_jpip_placeholder(&payload).unwrap();
        assert_eq!(placeholder.flags, 6);
        assert_eq!(placeholder.equivalent_bin, Some(4));
        assert_eq!(placeholder.stream, Some(9));
        assert_eq!(placeholder.stream_count, 1);

        let mut range = 12_u32.to_be_bytes().to_vec();
        range.extend_from_slice(&0_u64.to_be_bytes());
        range.extend_from_slice(&original);
        range.extend_from_slice(&0_u64.to_be_bytes());
        range.extend_from_slice(&equivalent);
        range.extend_from_slice(&11_u64.to_be_bytes());
        range.extend_from_slice(&2_u32.to_be_bytes());
        assert_eq!(parse_jpip_placeholder(&range).unwrap().stream_count, 2);

        let mut extension = 0x10_u32.to_be_bytes().to_vec();
        extension.extend_from_slice(&0_u64.to_be_bytes());
        extension.extend_from_slice(&original);
        assert!(matches!(
            parse_jpip_placeholder(&extension),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));
    }

    fn decodable_codestream() -> Vec<u8> {
        decodable_codestream_with_layers(1)
    }

    fn decodable_codestream_with_layers(layers: u16) -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0_u16.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1]);
        let mut bytes = SOC.to_vec();
        bytes.extend(segment(0xff51, &siz));
        let layer_bytes = layers.to_be_bytes();
        bytes.extend(segment(
            0xff52,
            &[0, 0, layer_bytes[0], layer_bytes[1], 0, 0, 0, 0, 0, 1],
        ));
        bytes.extend(segment(0xff5c, &[0, 0x40]));
        let tile_start = bytes.len();
        let mut sot = Vec::new();
        sot.extend_from_slice(&0_u16.to_be_bytes());
        sot.extend_from_slice(&(14_u32 + u32::from(layers)).to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        bytes.extend(segment(0xff90, &sot));
        bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
        bytes.extend(vec![0; usize::from(layers)]);
        assert_eq!(bytes.len() - tile_start, 14 + usize::from(layers));
        bytes.extend_from_slice(&EOC);
        bytes
    }

    fn two_tile_codestream() -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0_u16.to_be_bytes());
        siz.extend_from_slice(&2_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1]);
        let mut bytes = SOC.to_vec();
        bytes.extend(segment(0xff51, &siz));
        bytes.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
        bytes.extend(segment(0xff5c, &[0, 0x40]));
        let mut tile = 0_u16;
        while tile < 2 {
            let mut sot = Vec::new();
            sot.extend_from_slice(&tile.to_be_bytes());
            sot.extend_from_slice(&15_u32.to_be_bytes());
            sot.extend_from_slice(&[0, 1]);
            bytes.extend(segment(0xff90, &sot));
            bytes.extend_from_slice(&[0xff, 0x93, 0]);
            tile += 1;
        }
        bytes.extend_from_slice(&EOC);
        bytes
    }

    #[test]
    fn parses_all_three_standard_header_forms_and_auxiliary() {
        let limits = ResourceLimits::default();
        let mut context = JpipHeaderContext::new();
        let explicit = [0x72, 0x01, 0x01, 0x00, 0x03, 0x07, 1, 2, 3];
        let (message, consumed) = parse_jpip_message(&explicit, &mut context, &limits).unwrap();
        assert_eq!(consumed, explicit.len());
        let JpipMessage::Data { header, body } = message else {
            panic!("data message expected");
        };
        assert_eq!(header.databin().class(), DatabinClass::Precinct);
        assert_eq!(header.databin().in_class(), 2);
        assert_eq!(header.auxiliary(), Some(7));
        assert!(header.is_last_byte());
        assert_eq!(body, [1, 2, 3]);

        let inherited = [0x31, 0x03, 0x01, 0, 9];
        let (message, _) = parse_jpip_message(&inherited, &mut context, &limits).unwrap();
        let JpipMessage::Data { header, .. } = message else {
            panic!("data message expected");
        };
        assert_eq!(header.databin().class_identifier(), 1);
        assert_eq!(header.databin().codestream(), 1);

        let class_only = [0x50, 0x08, 0x00, 0x00];
        let (message, _) = parse_jpip_message(&class_only, &mut context, &limits).unwrap();
        let JpipMessage::Data { header, .. } = message else {
            panic!("data message expected");
        };
        assert_eq!(header.databin().class(), DatabinClass::Metadata);
        assert_eq!(header.databin().codestream(), 1);
    }

    #[test]
    fn parses_end_of_response() {
        let mut context = JpipHeaderContext::new();
        let input = [0, 2, 3, b'e', b'o', b'r'];
        let (message, consumed) =
            parse_jpip_message(&input, &mut context, &ResourceLimits::default()).unwrap();
        let JpipMessage::EndOfResponse(end) = message else {
            panic!("end-of-response expected");
        };
        assert_eq!(end.reason_code(), 2);
        assert_eq!(end.body(), b"eor");
        assert_eq!(consumed, input.len());
    }

    #[test]
    fn rejects_malformed_prefixes_and_vbas() {
        let limits = ResourceLimits::default();
        for input in [
            &[0x01, 0, 0][..],
            &[0x21, 0, 0][..],
            &[0x40, 0x80, 0, 0][..],
            &[0x40, 0x80][..],
            &[0xc0, 8, 0, 0x80][..],
        ] {
            assert!(parse_jpip_message(input, &mut JpipHeaderContext::new(), &limits).is_err());
        }
        let overflow = [
            0xc0, 8, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
        ];
        assert!(parse_jpip_message(&overflow, &mut JpipHeaderContext::new(), &limits).is_err());
    }

    #[test]
    fn reassembles_out_of_order_fragments() {
        let target = id(8, 4);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(target, 3, b"def", true)).unwrap();
        assert!(!cache.is_complete(target));
        cache.insert(fragment(target, 0, b"abc", false)).unwrap();
        assert!(cache.is_complete(target));
        assert_eq!(cache.databin_view(target).unwrap().as_bytes(), b"abcdef");
    }

    #[test]
    fn accepts_identical_duplicates_and_overlaps() {
        let target = id(8, 0);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(target, 0, b"abcd", false)).unwrap();
        cache.insert(fragment(target, 0, b"abcd", false)).unwrap();
        cache.insert(fragment(target, 2, b"cdef", true)).unwrap();
        let status = cache.status(target).unwrap();
        assert_eq!(status.interval_count(), 1);
        assert_eq!(status.stored_bytes(), 6);
        assert_eq!(cache.databin_view(target).unwrap().as_bytes(), b"abcdef");
    }

    #[test]
    fn rejects_conflicting_overlap_without_mutation() {
        let target = id(8, 0);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(target, 0, b"abcd", true)).unwrap();
        let before = cache.databin_view(target).unwrap();
        assert!(cache.insert(fragment(target, 2, b"XX", false)).is_err());
        assert_eq!(cache.databin_view(target).unwrap(), before);
        assert_eq!(cache.accepted_fragments(), 1);
    }

    #[test]
    fn rejects_conflicting_final_lengths_and_bytes_past_final() {
        let target = id(8, 0);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(target, 4, b"", true)).unwrap();
        assert!(cache.insert(fragment(target, 3, b"xy", false)).is_err());
        assert!(cache.insert(fragment(target, 5, b"", true)).is_err());
    }

    #[test]
    fn enforces_fragment_limit() {
        let limits = ResourceLimits::default()
            .with_maximum_jpip_fragments(1)
            .unwrap();
        let target = id(8, 0);
        let mut cache = DatabinCache::new(limits);
        cache.insert(fragment(target, 0, b"a", false)).unwrap();
        assert!(matches!(
            cache.insert(fragment(target, 1, b"b", true)),
            Err(DecodeError::LimitExceeded {
                resource: Resource::JpipFragments,
                ..
            })
        ));
    }

    #[test]
    fn reconstructs_codestream_and_container_views() {
        let main = id(6, 0);
        let tile_header = id(2, 0);
        let tile_data = id(4, 0);
        let metadata = id(8, 0);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache
            .insert(fragment(main, 0, &[0xff, 0x4f], true))
            .unwrap();
        cache
            .insert(fragment(tile_header, 0, &[0xff, 0x90], true))
            .unwrap();
        cache
            .insert(fragment(tile_data, 0, &[0xff, 0xd9], true))
            .unwrap();
        cache.insert(fragment(metadata, 0, b"jp2", true)).unwrap();
        assert_eq!(
            cache
                .reconstruct_codestream(&[main, tile_header, tile_data])
                .unwrap()
                .as_bytes(),
            [0xff, 0x4f, 0xff, 0x90, 0xff, 0xd9]
        );
        assert_eq!(
            cache.reconstruct_container(&[metadata]).unwrap().as_bytes(),
            b"jp2"
        );
    }

    #[test]
    fn reconstructed_codestream_is_consumed_by_decode_api() {
        let bytes = decodable_codestream();
        let tile_start = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let tile_data_start = tile_start + 14;
        let main = id(6, 0);
        let tile_header = id(2, 0);
        let tile_data = id(4, 0);
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        cache
            .insert(fragment(main, 0, &bytes[..tile_start], true))
            .unwrap();
        cache
            .insert(fragment(
                tile_header,
                0,
                &bytes[tile_start..tile_data_start],
                true,
            ))
            .unwrap();
        cache
            .insert(fragment(tile_data, 0, &bytes[tile_data_start..], true))
            .unwrap();
        let view = cache
            .reconstruct_codestream(&[main, tile_header, tile_data])
            .unwrap();
        let image =
            crate::decode_codestream(view.as_ref(), &crate::DecodeOptions::default()).unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }

    #[test]
    fn ingests_response_with_inherited_context() {
        let input = [
            0x40, 8, 0, 2, b'a', b'b', // explicit metadata bin 0
            0x31, 0, 1, b'c', // inherited metadata bin 1, final
            0, 1, 0, // EOR
        ];
        let mut cache = DatabinCache::new(ResourceLimits::default());
        let summary = ingest_jpip_response(&input, &mut cache).unwrap();
        assert_eq!(summary.messages(), 2);
        assert_eq!(summary.end_of_response_reason(), Some(1));
        assert_eq!(cache.databin_view(id(8, 1)).unwrap().as_bytes(), b"c");
    }

    #[test]
    fn reports_exact_missing_ranges_for_incomplete_databins() {
        let target = id(0, 9);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(target, 0, b"a", false)).unwrap();
        cache.insert(fragment(target, 3, b"d", false)).unwrap();
        let report = cache
            .availability(&[JpipDatabinRange::new(target, 0, 4).unwrap()])
            .unwrap();
        assert_eq!(report.status(), JpipAvailability::Partial);
        assert_eq!(report.required_bytes(), 4);
        assert_eq!(report.available_bytes(), 2);
        assert_eq!(
            report.missing_ranges(),
            &[JpipMissingRange {
                databin: target,
                offset: 1,
                length: Some(2),
            }]
        );
    }

    #[test]
    fn transcodes_precinct_ranges_into_a_decodable_codestream() {
        let bytes = decodable_codestream_with_layers(2);
        let tile_start = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let packet_start = tile_start + 14;
        let main = id(6, 0);
        let tile_header = id(2, 0);
        let precinct = id(0, 0);
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        cache
            .insert(fragment(main, 0, &bytes[..tile_start], true))
            .unwrap();
        cache
            .insert(fragment(
                tile_header,
                0,
                &bytes[tile_start..packet_start],
                true,
            ))
            .unwrap();
        cache
            .insert(fragment(
                precinct,
                0,
                &bytes[packet_start..packet_start + 2],
                false,
            ))
            .unwrap();
        let plan = JpipPrecinctCodestreamPlan::new(
            main,
            tile_header,
            vec![
                JpipPrecinctPacket::new(precinct, 0, 0, 1).unwrap(),
                JpipPrecinctPacket::new(precinct, 1, 1, 1).unwrap(),
            ],
            2,
            &limits,
        )
        .unwrap();
        let packets = cache.reconstruct_precinct_packets(&plan, 2).unwrap();
        assert!(packets.availability().is_available());
        assert_eq!(packets.packets().unwrap().as_bytes(), [0, 0]);
        let transcoded = cache.transcode_precinct_codestream(&plan, 1).unwrap();
        assert!(transcoded.availability().is_available());
        let image = crate::decode_codestream(
            transcoded.codestream().unwrap().as_ref(),
            &crate::DecodeOptions::default(),
        )
        .unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }

    #[test]
    fn progressive_decode_advances_only_after_a_complete_layer() {
        let bytes = decodable_codestream_with_layers(2);
        let tile_start = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let packet_start = tile_start + 14;
        let main = id(6, 0);
        let tile_header = id(2, 0);
        let precinct = id(0, 0);
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        cache
            .insert(fragment(main, 0, &bytes[..tile_start], true))
            .unwrap();
        cache
            .insert(fragment(
                tile_header,
                0,
                &bytes[tile_start..packet_start],
                true,
            ))
            .unwrap();
        let plan = JpipPrecinctCodestreamPlan::new(
            main,
            tile_header,
            vec![
                JpipPrecinctPacket::new(precinct, 0, 0, 1).unwrap(),
                JpipPrecinctPacket::new(precinct, 1, 1, 1).unwrap(),
            ],
            2,
            &limits,
        )
        .unwrap();
        let mut decoder = JpipProgressiveDecoder::new(plan);
        let waiting = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(waiting.status(), JpipProgressiveStatus::WaitingForData);
        assert_eq!(waiting.available_layers(), 0);
        assert_eq!(waiting.missing_ranges()[0].offset(), 0);

        cache
            .insert(fragment(
                precinct,
                0,
                &bytes[packet_start..packet_start + 1],
                false,
            ))
            .unwrap();
        let first = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(first.status(), JpipProgressiveStatus::QualityAdvanced);
        assert_eq!(first.previous_layers(), 0);
        assert_eq!(first.available_layers(), 1);
        assert!(first.image().is_some());

        let unchanged = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(unchanged.status(), JpipProgressiveStatus::WaitingForData);
        assert!(unchanged.image().is_none());
        assert_eq!(decoder.decoded_layers(), 1);

        cache
            .insert(fragment(
                precinct,
                1,
                &bytes[packet_start + 1..packet_start + 2],
                false,
            ))
            .unwrap();
        let complete = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(complete.status(), JpipProgressiveStatus::Complete);
        assert_eq!(complete.available_layers(), 2);
        assert!(complete.missing_ranges().is_empty());
        assert!(complete.image().is_some());
        assert_eq!(decoder.decoded_layers(), 2);
    }

    #[test]
    fn automatic_mapping_decodes_incomplete_precinct_quality_prefixes() {
        let bytes = decodable_codestream_with_layers(2);
        let tile_start = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let packet_start = tile_start + 14;
        let main = id(6, 0);
        let tile_header = id(2, 0);
        let precinct_with_auxiliary = id(1, 0);
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        cache
            .insert(fragment(main, 0, &bytes[..tile_start], true))
            .unwrap();
        cache
            .insert(fragment(
                tile_header,
                0,
                &bytes[tile_start..packet_start],
                true,
            ))
            .unwrap();
        let mapping = cache.derive_precinct_map(0).unwrap();
        assert_eq!(mapping.mappings().len(), 1);
        assert_eq!(mapping.mappings()[0].databin().in_class(), 0);
        assert_eq!(
            mapping.mappings()[0].coordinate(),
            JpipPrecinctCoordinate {
                tile: 0,
                component: 0,
                resolution: 0,
                precinct: 0,
            }
        );

        let mut decoder = JpipAutomaticProgressiveDecoder::new(0);
        let waiting = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(waiting.status(), JpipProgressiveStatus::WaitingForData);
        assert_eq!(waiting.available_layers(), 0);

        cache
            .insert(fragment(
                precinct_with_auxiliary,
                0,
                &bytes[packet_start..packet_start + 1],
                false,
            ))
            .unwrap();
        let first = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(first.status(), JpipProgressiveStatus::QualityAdvanced);
        assert_eq!(first.available_layers(), 1);
        assert!(first.image().is_some());

        cache
            .insert(fragment(
                precinct_with_auxiliary,
                1,
                &bytes[packet_start + 1..packet_start + 2],
                false,
            ))
            .unwrap();
        let complete = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(complete.status(), JpipProgressiveStatus::Complete);
        assert_eq!(complete.available_layers(), 2);
        assert!(complete.image().is_some());

        let selection = crate::DecodeSelection::full_image()
            .with_region(crate::ImageRegion::new(0, 0, 1, 1))
            .with_discard_levels(0);
        let components = JpipPacketSelection::components(vec![0]);
        let mut selected = JpipAutomaticProgressiveDecoder::new(0);
        let update = selected
            .update_selected(
                &cache,
                &crate::DecodeOptions::default(),
                &selection,
                &components,
            )
            .unwrap();
        assert_eq!(update.status(), JpipProgressiveStatus::Complete);
        assert_eq!(update.image().unwrap().width(), 1);
        assert!(
            selected
                .update_selected(
                    &cache,
                    &crate::DecodeOptions::default(),
                    &selection,
                    &JpipPacketSelection::components(vec![1]),
                )
                .is_err()
        );
    }

    #[test]
    fn automatic_header_normalization_removes_reframed_markers() {
        let mut main = SOC.to_vec();
        main.extend(segment(0xff51, &[0; 36]));
        main.extend(segment(0xff55, &[0]));
        main.extend(segment(0xff57, &[0]));
        main.extend(segment(0xff60, &[0]));
        let normalized = normalize_main_header(&main).unwrap();
        assert!(!normalized.windows(2).any(|value| value == [0xff, 0x55]));
        assert!(!normalized.windows(2).any(|value| value == [0xff, 0x57]));
        assert!(!normalized.windows(2).any(|value| value == [0xff, 0x60]));

        let mut tile = vec![0xff, 0x90, 0, 10, 0, 0, 0, 0, 0, 14, 0, 1];
        tile.extend(segment(0xff58, &[0]));
        tile.extend(segment(0xff61, &[0]));
        tile.extend_from_slice(&[0xff, 0x93]);
        let normalized_tile = normalize_automatic_tile_headers(&tile, 0)
            .unwrap()
            .remove(0);
        assert!(
            !normalized_tile
                .windows(2)
                .any(|value| value == [0xff, 0x58])
        );
        assert!(
            !normalized_tile
                .windows(2)
                .any(|value| value == [0xff, 0x61])
        );
        assert!(normalized_tile.ends_with(&[0xff, 0x93]));

        let mut ambiguous = tile;
        let mut second = vec![0xff, 0x90, 0, 10, 0, 0, 0, 0, 0, 14, 1, 2];
        second.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
        second.extend_from_slice(&[0xff, 0x93]);
        ambiguous.extend(second);
        let normalized = normalize_automatic_tile_headers(&ambiguous, 0).unwrap();
        assert_eq!(normalized.len(), 2);
        assert!(normalized[1].windows(2).any(|value| value == [0xff, 0x52]));
    }

    #[test]
    fn automatic_mapping_supports_progressions_poc_sop_eph_and_reframed_headers() {
        let limits = ResourceLimits::default();
        for progression in 0_u8..=4 {
            let mut bytes = decodable_codestream();
            let cod = bytes
                .windows(2)
                .position(|value| value == [0xff, 0x52])
                .unwrap();
            bytes[cod + 5] = progression;
            let sot = bytes
                .windows(2)
                .position(|value| value == [0xff, 0x90])
                .unwrap();
            if progression == 1 {
                bytes.splice(sot..sot, segment(0xff5f, &[0, 0, 0, 1, 1, 1, 1]));
            }
            let sot = bytes
                .windows(2)
                .position(|value| value == [0xff, 0x90])
                .unwrap();
            let packet = sot + 14;
            let mut cache = DatabinCache::new(limits);
            cache
                .insert(fragment(id(6, 0), 0, &bytes[..sot], true))
                .unwrap();
            cache
                .insert(fragment(id(2, 0), 0, &bytes[sot..packet], true))
                .unwrap();
            cache
                .insert(fragment(id(0, 0), 0, &bytes[packet..packet + 1], false))
                .unwrap();
            let mut decoder = JpipAutomaticProgressiveDecoder::new(0);
            let update = decoder
                .update(&cache, &crate::DecodeOptions::default())
                .unwrap();
            assert_eq!(update.status(), JpipProgressiveStatus::Complete);
            assert!(update.image().is_some());
        }

        let mut bytes = decodable_codestream();
        let cod = bytes
            .windows(2)
            .position(|value| value == [0xff, 0x52])
            .unwrap();
        bytes[cod + 4] = 6;
        let sot = bytes
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .unwrap();
        let packet = sot + 14;
        bytes.splice(packet..packet + 1, [0xff, 0x91, 0, 4, 0, 0, 0, 0xff, 0x92]);
        let psot = 23_u32.to_be_bytes();
        bytes[sot + 6..sot + 10].copy_from_slice(&psot);
        let mut cache = DatabinCache::new(limits);
        cache
            .insert(fragment(id(6, 0), 0, &bytes[..sot], true))
            .unwrap();
        let mut tile_header = bytes[sot..packet].to_vec();
        tile_header.splice(
            tile_header.len() - 2..tile_header.len() - 2,
            segment(0xff58, &[0]),
        );
        tile_header.splice(
            tile_header.len() - 2..tile_header.len() - 2,
            segment(0xff61, &[0]),
        );
        cache
            .insert(fragment(id(2, 0), 0, &tile_header, true))
            .unwrap();
        cache
            .insert(fragment(id(0, 0), 0, &bytes[packet..packet + 9], false))
            .unwrap();
        let mut decoder = JpipAutomaticProgressiveDecoder::new(0);
        assert_eq!(
            decoder
                .update(&cache, &crate::DecodeOptions::default())
                .unwrap()
                .status(),
            JpipProgressiveStatus::Complete
        );
    }

    #[test]
    fn automatic_mapping_interleaves_multiple_tiles_and_collapses_tile_parts() {
        let bytes = two_tile_codestream();
        let first_sot = bytes
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .unwrap();
        let second_sot = bytes[first_sot + 2..]
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .map(|value| value + first_sot + 2)
            .unwrap();
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache
            .insert(fragment(id(6, 0), 0, &bytes[..first_sot], true))
            .unwrap();
        cache
            .insert(fragment(
                id(2, 0),
                0,
                &bytes[first_sot..first_sot + 14],
                true,
            ))
            .unwrap();
        cache
            .insert(fragment(
                id(2, 1),
                0,
                &bytes[second_sot..second_sot + 14],
                true,
            ))
            .unwrap();
        cache.insert(fragment(id(0, 0), 0, &[0], false)).unwrap();
        cache.insert(fragment(id(0, 1), 0, &[0], false)).unwrap();
        let mapping = cache.derive_precinct_map(0).unwrap();
        assert_eq!(mapping.mappings().len(), 2);
        assert_eq!(mapping.mappings()[0].databin().in_class(), 0);
        assert_eq!(mapping.mappings()[1].databin().in_class(), 1);
        let mut decoder = JpipAutomaticProgressiveDecoder::new(0);
        let update = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(update.status(), JpipProgressiveStatus::Complete);
        assert_eq!(update.image().unwrap().width(), 2);

        let two_layers = decodable_codestream_with_layers(2);
        let sot = two_layers
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .unwrap();
        let mut tile_headers = two_layers[sot..sot + 14].to_vec();
        let mut second = tile_headers.clone();
        second[10] = 1;
        second[11] = 2;
        tile_headers[11] = 2;
        tile_headers.extend_from_slice(&second);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache
            .insert(fragment(id(6, 0), 0, &two_layers[..sot], true))
            .unwrap();
        cache
            .insert(fragment(id(2, 0), 0, &tile_headers, true))
            .unwrap();
        cache.insert(fragment(id(0, 0), 0, &[0, 0], false)).unwrap();
        let mut decoder = JpipAutomaticProgressiveDecoder::new(0);
        assert_eq!(
            decoder
                .update(&cache, &crate::DecodeOptions::default())
                .unwrap()
                .status(),
            JpipProgressiveStatus::Complete
        );
    }

    #[test]
    fn semantic_metadata_assembly_places_nested_boxes_and_codestreams() {
        let codestream = decodable_codestream();
        let sot = codestream
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .unwrap();
        let packet = sot + 14;
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut jp2h_payload = box32(*b"ihdr", &image_header);
        jp2h_payload.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        let jp2h_length = u32::try_from(jp2h_payload.len() + 8).unwrap();
        let mut root = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        root.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        root.extend(metadata_placeholder(*b"jp2h", jp2h_length, 1, 1, None));
        root.extend(metadata_placeholder(
            *b"jp2c",
            u32::try_from(codestream.len() + 8).unwrap(),
            4,
            0,
            Some(0),
        ));

        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(id(8, 0), 0, &root, true)).unwrap();
        let missing_metadata = cache.assemble_semantic_container(0).unwrap();
        assert_eq!(missing_metadata.status(), JpipAvailability::Partial);
        assert_eq!(
            missing_metadata.semantic_needs()[0].kind(),
            JpipSemanticNeedKind::MetadataBin
        );

        cache
            .insert(fragment(id(8, 1), 0, &jp2h_payload, true))
            .unwrap();
        let missing_codestream = cache.assemble_semantic_container(0).unwrap();
        assert_eq!(missing_codestream.status(), JpipAvailability::Partial);
        assert!(
            missing_codestream
                .semantic_needs()
                .iter()
                .any(|need| need.kind() == JpipSemanticNeedKind::Codestream)
        );

        cache
            .insert(fragment(id(6, 0), 0, &codestream[..sot], true))
            .unwrap();
        cache
            .insert(fragment(id(2, 0), 0, &codestream[sot..packet], true))
            .unwrap();
        cache
            .insert(fragment(
                id(0, 0),
                0,
                &codestream[packet..packet + 1],
                false,
            ))
            .unwrap();
        let assembled = cache.assemble_semantic_container(0).unwrap();
        assert_eq!(assembled.status(), JpipAvailability::Available);
        assert!(assembled.semantic_needs().is_empty());
        let container = assembled.container().unwrap();
        let inspected =
            crate::jp2::parse_and_inspect(container.as_bytes(), &ResourceLimits::default())
                .unwrap();
        assert_eq!(inspected.file.codestream_count(), 1);
        assert!(
            container
                .as_bytes()
                .windows(4)
                .all(|value| value != b"phld")
        );
    }

    #[test]
    fn tile_part_transition_status_distinguishes_sufficient_packet_boundaries() {
        let bytes = decodable_codestream_with_layers(2);
        let sot = bytes
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .unwrap();
        let mut first = bytes[sot..sot + 12].to_vec();
        first[11] = 2;
        first.extend(segment(0xff58, &[0, 1]));
        first.extend_from_slice(&[0xff, 0x93]);
        let mut second = bytes[sot..sot + 12].to_vec();
        second[10] = 1;
        second[11] = 2;
        second.extend(segment(0xff52, &[0, 0, 0, 2, 0, 0, 0, 0, 0, 1]));
        second.extend_from_slice(&[0xff, 0x93]);
        let mut headers = first.clone();
        headers.extend(second.clone());

        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache.insert(fragment(id(2, 0), 0, &headers, true)).unwrap();
        let report = cache.tile_part_transitions(0, 0).unwrap();
        assert_eq!(report.status(), JpipAvailability::Available);
        assert_eq!(report.transitions().len(), 2);
        assert_eq!(report.transitions()[0].packet_count(), Some(1));
        assert!(report.transitions()[1].has_functional_change());

        let mut insufficient = first[..12].to_vec();
        insufficient.extend_from_slice(&[0xff, 0x93]);
        insufficient.extend(second);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache
            .insert(fragment(id(2, 0), 0, &insufficient, true))
            .unwrap();
        let report = cache.tile_part_transitions(0, 0).unwrap();
        assert_eq!(report.status(), JpipAvailability::Available);
        assert!(report.semantic_needs().is_empty());
        assert_eq!(report.transitions()[0].data_length(), Some(2));
    }

    #[test]
    fn automatic_executes_functional_later_tile_part_without_plt() {
        let bytes = decodable_codestream_with_layers(2);
        let sot = bytes
            .windows(2)
            .position(|value| value == [0xff, 0x90])
            .unwrap();
        let mut first = bytes[sot..sot + 12].to_vec();
        first[6..10].copy_from_slice(&15_u32.to_be_bytes());
        first[11] = 2;
        first.extend_from_slice(&[0xff, 0x93]);

        let cod = bytes
            .windows(2)
            .position(|value| value == [0xff, 0x52])
            .unwrap();
        let cod_length = usize::from(u16::from_be_bytes([bytes[cod + 2], bytes[cod + 3]]));
        let cod_end = cod + 2 + cod_length;
        let mut second = bytes[sot..sot + 12].to_vec();
        second[10] = 1;
        second[11] = 2;
        let second_length = 14 + cod_end - cod + 1;
        second[6..10].copy_from_slice(&(second_length as u32).to_be_bytes());
        second.extend_from_slice(&bytes[cod..cod_end]);
        second.extend_from_slice(&[0xff, 0x93]);

        let mut headers = first;
        headers.extend(second);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        cache
            .insert(fragment(id(6, 0), 0, &bytes[..sot], true))
            .unwrap();
        cache.insert(fragment(id(2, 0), 0, &headers, true)).unwrap();
        cache.insert(fragment(id(0, 0), 0, &[0, 0], false)).unwrap();

        let report = cache.tile_part_transitions(0, 0).unwrap();
        assert_eq!(report.status(), JpipAvailability::Available);
        assert_eq!(report.transitions()[0].data_length(), Some(1));
        let mut decoder = JpipAutomaticProgressiveDecoder::new(0);
        let update = decoder
            .update(&cache, &crate::DecodeOptions::default())
            .unwrap();
        assert_eq!(update.status(), JpipProgressiveStatus::Complete);
        assert!(update.image().is_some());
    }
}
