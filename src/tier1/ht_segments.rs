use super::state::{arithmetic_overflow, invalid_code_block};
use super::{
    HtBlockSegments, HtCleanupSegment, HtPassGroup, HtRefinementSegment, HtStreamDirection,
    HtSubstreamView, MAX_HT_CLEANUP_BYTES, MAX_HT_REFINEMENT_BYTES, MAX_HT_SCUP,
};
#[cfg(verus_keep_ghost)]
use super::{
    ht_block_segments_outcome, ht_block_segments_view, ht_cleanup_segment_view,
    ht_direction_forward, ht_optional_bytes_view, ht_refinement_option_view,
    ht_refinement_segment_view, ht_substream_view,
};
use crate::{DecodeError, Resource, ResourceLimits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl HtPassGroup {
    pub fn from_packet_passes(passes: u8) -> Result<Self, DecodeError> {
        ht_pass_group_from_packet(passes)
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::ht_packet_pass_group(passes),)
)]
pub(super) fn ht_pass_group_from_packet(passes: u8) -> Result<HtPassGroup, DecodeError> {
    if passes == 0 || passes > 164 {
        return Err(invalid_code_block());
    }
    let placeholder_passes = crate::refinement::ht_placeholder_passes(passes);
    let skipped_bitplanes = placeholder_passes / 3;
    let substantive_passes = crate::refinement::ht_substantive_passes(passes);
    Ok(HtPassGroup {
        cleanup_passes: placeholder_passes
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?,
        refinement_passes: substantive_passes
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?,
        placeholder_passes,
        skipped_bitplanes,
    })
}

impl<'a> HtSubstreamView<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == ht_substream_view(self).bytes,))]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_substream_view(self).offset,))]
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_direction_forward(result) == ht_substream_view(self).forward,))]
    pub const fn direction(&self) -> HtStreamDirection {
        self.direction
    }
}

impl<'a> HtCleanupSegment<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == ht_cleanup_segment_view(self).encoded,))]
    pub fn encoded(&self) -> &'a [u8] {
        self.encoded
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_cleanup_segment_view(self).scup,))]
    pub const fn scup(&self) -> u16 {
        self.scup
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).mag_sgn,))]
    pub const fn mag_sgn(&self) -> HtSubstreamView<'a> {
        self.mag_sgn
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).mel_vlc,))]
    pub const fn mel_vlc(&self) -> HtSubstreamView<'a> {
        self.mel_vlc
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).mel,))]
    pub const fn mel(&self) -> HtSubstreamView<'a> {
        self.mel
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).vlc,))]
    pub const fn vlc(&self) -> HtSubstreamView<'a> {
        self.vlc
    }
}

impl<'a> HtRefinementSegment<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == ht_refinement_segment_view(self).encoded,))]
    pub fn encoded(&self) -> &'a [u8] {
        self.encoded
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_refinement_segment_view(self).significance_propagation,))]
    pub const fn significance_propagation(&self) -> HtSubstreamView<'a> {
        self.significance_propagation
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_refinement_segment_view(self).magnitude_refinement,))]
    pub const fn magnitude_refinement(&self) -> HtSubstreamView<'a> {
        self.magnitude_refinement
    }
}

impl<'a> HtBlockSegments<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_cleanup_segment_view(&result) == ht_block_segments_view(self).cleanup,))]
    pub const fn cleanup(&self) -> HtCleanupSegment<'a> {
        self.cleanup
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_refinement_option_view(result) == ht_block_segments_view(self).refinement,))]
    pub const fn refinement(&self) -> Option<HtRefinementSegment<'a>> {
        self.refinement
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_block_segments_view(self).bytes,))]
    pub const fn bytes(&self) -> u32 {
        self.bytes
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_block_segments_outcome(result) == crate::specs::ht_parse_block_segments(
            cleanup@, ht_optional_bytes_view(refinement), crate::limits::resource_code_block_byte_limit(limits),
        ),
    )
)]
pub fn parse_ht_block_segments<'a>(
    cleanup: &'a [u8],
    refinement: Option<&'a [u8]>,
    limits: &ResourceLimits,
) -> Result<HtBlockSegments<'a>, DecodeError> {
    if cleanup.len() < 2 || cleanup.len() > MAX_HT_CLEANUP_BYTES {
        return Err(invalid_code_block());
    }
    let cleanup_bytes = cleanup.len() as u64;
    let refinement_bytes = match refinement {
        Some(bytes) => bytes.len() as u64,
        None => 0_u64,
    };
    if refinement_bytes > MAX_HT_REFINEMENT_BYTES as u64 {
        return Err(invalid_code_block());
    }
    let total_bytes = cleanup_bytes
        .checked_add(refinement_bytes)
        .ok_or_else(arithmetic_overflow)?;
    if total_bytes > limits.maximum_code_block_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::CodeBlockBytes,
            limit: limits.maximum_code_block_bytes(),
            actual: total_bytes,
        });
    }

    let penultimate = *cleanup
        .get(cleanup.len() - 2)
        .ok_or_else(invalid_code_block)?;
    let last = *cleanup.last().ok_or_else(invalid_code_block)?;
    let scup = crate::refinement::ht_scup(penultimate, last);
    if scup < 2 {
        return Err(invalid_code_block());
    }
    if scup > MAX_HT_SCUP || scup > cleanup.len() {
        return Err(invalid_code_block());
    }
    let mag_sgn_end = crate::refinement::ht_magsgn_end(cleanup.len(), scup);
    let mel_end = cleanup
        .len()
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    if mag_sgn_end > mel_end || mel_end > cleanup.len() {
        return Err(invalid_code_block());
    }
    let mag_sgn_bytes = cleanup.get(..mag_sgn_end).ok_or_else(invalid_code_block)?;
    let mel_vlc_bytes = cleanup.get(mag_sgn_end..).ok_or_else(invalid_code_block)?;
    let mel_bytes = cleanup
        .get(mag_sgn_end..mel_end)
        .ok_or_else(invalid_code_block)?;
    let mag_sgn_offset = 0_u32;
    let mel_vlc_offset = u32::try_from(mag_sgn_end).map_err(|_| arithmetic_overflow())?;
    let cleanup_segment = HtCleanupSegment {
        encoded: cleanup,
        scup: u16::try_from(scup).map_err(|_| arithmetic_overflow())?,
        mag_sgn: HtSubstreamView {
            bytes: mag_sgn_bytes,
            offset: mag_sgn_offset,
            direction: HtStreamDirection::Forward,
        },
        mel_vlc: HtSubstreamView {
            bytes: mel_vlc_bytes,
            offset: mel_vlc_offset,
            direction: HtStreamDirection::Forward,
        },
        mel: HtSubstreamView {
            bytes: mel_bytes,
            offset: mel_vlc_offset,
            direction: HtStreamDirection::Forward,
        },
        vlc: HtSubstreamView {
            bytes: mel_vlc_bytes,
            offset: mel_vlc_offset,
            direction: HtStreamDirection::Reverse,
        },
    };
    let refinement_segment = match refinement {
        Some(encoded) => Some(HtRefinementSegment {
            encoded,
            significance_propagation: HtSubstreamView {
                bytes: encoded,
                offset: u32::try_from(cleanup.len()).map_err(|_| arithmetic_overflow())?,
                direction: HtStreamDirection::Forward,
            },
            magnitude_refinement: HtSubstreamView {
                bytes: encoded,
                offset: u32::try_from(cleanup.len()).map_err(|_| arithmetic_overflow())?,
                direction: HtStreamDirection::Reverse,
            },
        }),
        None => None,
    };
    Ok(HtBlockSegments {
        cleanup: cleanup_segment,
        refinement: refinement_segment,
        bytes: u32::try_from(total_bytes).map_err(|_| arithmetic_overflow())?,
    })
}
