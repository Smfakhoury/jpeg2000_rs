//! Precinct identifiers, packet selections, and tile-part transition mapping.

use super::{
    DatabinCache, DatabinId, DecodeError, JpipAvailability, JpipByteView, JpipContainerAssembly,
    JpipMissingRange, JpipPacketSelection, JpipPrecinctCoordinate, JpipPrecinctMap,
    JpipPrecinctMapping, JpipSemanticNeed, JpipSemanticNeedKind, JpipTilePartTransition,
    JpipTilePartTransitionReport, MalformedKind, Resource, ResourceLimits, SOC,
    automatic_tile_part, build_automatic_template, ceil_div_jpip, copy_bytes, marker_segment,
    read_u16_be, read_u32_be,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl JpipPrecinctMapping {
    pub const fn databin(&self) -> DatabinId {
        self.databin
    }

    pub const fn coordinate(&self) -> JpipPrecinctCoordinate {
        self.coordinate
    }
}

impl JpipSemanticNeed {
    pub const fn kind(&self) -> JpipSemanticNeedKind {
        self.kind
    }

    pub const fn identifier(&self) -> u64 {
        self.identifier
    }
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

    pub(super) fn includes_component(&self, component: u16) -> bool {
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

impl DatabinCache {
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
}

pub(super) fn parse_tile_part_transitions(
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

pub(super) fn siz_cardinality(
    main_header: &[u8],
    limits: &ResourceLimits,
) -> Result<(u64, u16), DecodeError> {
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

#[allow(clippy::too_many_arguments)]
pub(super) fn automatic_precinct_in_class(
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

pub(super) fn validate_packet_component_selection(
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

pub(super) fn jpip_tile_selected(selection: Option<&crate::DecodeSelection>, tile: u16) -> bool {
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

pub(super) fn jpip_resolution_selected(
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
