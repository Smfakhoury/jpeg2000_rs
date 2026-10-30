//! Required byte ranges, missing intervals, and progressive availability.

use super::{
    DatabinCache, DatabinId, DecodeError, JpipAvailability, JpipAvailabilityReport,
    JpipDatabinRange, JpipMissingRange, JpipPrecinctCodestreamPlan, JpipPrecinctPacket,
    MalformedKind, Resource,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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

impl DatabinCache {
    pub(super) fn complete_databin_missing(&self, id: DatabinId) -> Vec<JpipMissingRange> {
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

    pub(super) fn precinct_availability(
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

    pub(super) fn progressive_availability(
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

    pub(super) fn read_packet_ranges(
        &self,
        packets: &[JpipPrecinctPacket],
    ) -> Result<Vec<u8>, DecodeError> {
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
}

pub(super) fn append_missing_ranges(
    output: &mut Vec<JpipMissingRange>,
    input: &[JpipMissingRange],
) {
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
