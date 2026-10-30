//! Binary message headers, inherited identifiers, and bounded response ingestion.

use super::{
    DatabinCache, DatabinClass, DatabinId, DecodeError, JpipEndOfResponse, JpipFragment,
    JpipHeaderContext, JpipMessage, JpipMessageHeader, JpipMessageParser, JpipResponseSummary,
    MAX_VBAS_BYTES, MalformedKind, Resource, ResourceLimits,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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

    pub(super) const fn same_as(&self, other: Self) -> bool {
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

impl<'a> JpipMessage<'a> {
    pub const fn body(&self) -> &'a [u8] {
        match self {
            Self::Data { body, .. } => body,
            Self::EndOfResponse(end) => end.body,
        }
    }
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
