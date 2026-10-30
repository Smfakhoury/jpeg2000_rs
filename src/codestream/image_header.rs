//! Image size, registration, bit-depth and tile-length marker parsing.

use super::geometry::ceil_div;
use super::reader::{read_u16, read_u32};
use super::{
    ComponentBitDepth, ComponentInfo, ComponentRegistration, DecodeError, ImageSize,
    MAX_COMPONENTS, MalformedKind, Parser, Part2Extensions, Resource, Segment, TilePartLength,
};
#[cfg(verus_keep_ghost)]
use super::{
    owned_extensions_view, parsed_siz_outcome, parser_view, segment_view, siz_components_view,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl<'a> Parser<'a> {
    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), *final(next_index), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_tlm(parser_view(old(self)), segment_view(segment),
                *old(next_index), tile_count, owned_extensions_view(old(extensions))),
    ))]
    pub(super) fn parse_tlm(
        &mut self,
        segment: Segment<'a>,
        next_index: &mut u16,
        tile_count: u64,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost index_input = *next_index;
            let ghost goal = crate::specs::part1_parse_tlm(input, segment_view(segment), index_input, tile_count, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_tlm); }
        self.charge_extension_metadata(segment, extensions)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
        }
        if segment.payload.len() < 2 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerOrder));
        }
        *next_index = match next_index.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let style = segment.payload[1];
        let tile_bytes = crate::refinement::tlm_tile_index_bytes(style);
        let length_bytes = crate::refinement::tlm_length_bytes(style);
        #[cfg(verus_keep_ghost)]
        proof! { assert((style >> 4) & 3 <= 3u8) by (bit_vector); }
        if style & 0x8f != 0 || tile_bytes == 3 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTilePart));
        }
        let record_bytes = match tile_bytes.checked_add(length_bytes) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let records = segment.payload.len() - 2;
        if record_bytes == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let remainder = records % record_bytes;
        if remainder != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = crate::specs::part1_tlm_records(segment_view(segment), tile_count,
                tile_bytes, length_bytes, 2, charged_extensions.lengths);
        }
        let mut position = 2_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    2 <= position <= segment.payload.len(),
                    tile_bytes <= 2, length_bytes == 2 || length_bytes == 4,
                    2 <= record_bytes <= 6,
                    record_bytes == tile_bytes + length_bytes,
                    (segment.payload.len() - position) % (record_bytes as int) == 0,
                    input == parser_view(old(self)),
                    extension_input == owned_extensions_view(old(extensions)),
                    index_input == *old(next_index),
                    index_input <= 255, *next_index == index_input + 1,
                    parser_view(self) == charged_parser,
                    owned_extensions_view(extensions) == crate::specs::part1_extension_lengths(
                        charged_extensions, extensions.tile_part_lengths@),
                    target == crate::specs::part1_tlm_records(segment_view(segment), tile_count,
                        tile_bytes, length_bytes, position as nat, extensions.tile_part_lengths@),
                    goal == crate::specs::part1_parse_tlm(input, segment_view(segment), index_input, tile_count, extension_input),
                    goal == (target.0, charged_parser, *next_index,
                        crate::specs::part1_extension_lengths(charged_extensions, target.1)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_tlm_records, 1);
                reveal(crate::specs::part1_segment_byte);
                if segment.payload.len() - position < record_bytes {
                    vstd::arithmetic::div_mod::lemma_small_mod(
                        (segment.payload.len() - position) as nat, record_bytes as nat);
                }
                assert(position + record_bytes <= segment.payload.len());
            }
            let tile_index = if tile_bytes == 0 {
                None
            } else if tile_bytes == 1 {
                Some(u16::from(self.segment_byte(segment, position, 0)?))
            } else {
                Some(read_u16([
                    self.segment_byte(segment, position, 0)?,
                    self.segment_byte(segment, position, 1)?,
                ]))
            };
            if matches!(tile_index, Some(value) if u64::from(value) >= tile_count) {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTilePart));
            }
            let length_position = match position.checked_add(tile_bytes) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            let length = if length_bytes == 2 {
                u32::from(read_u16([
                    self.segment_byte(segment, length_position, 0)?,
                    self.segment_byte(segment, length_position, 1)?,
                ]))
            } else {
                read_u32([
                    self.segment_byte(segment, length_position, 0)?,
                    self.segment_byte(segment, length_position, 1)?,
                    self.segment_byte(segment, length_position, 2)?,
                    self.segment_byte(segment, length_position, 3)?,
                ])
            };
            if length < 14 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTilePart));
            }
            extensions
                .tile_part_lengths
                .push(TilePartLength { tile_index, length });
            let next_position = match position.checked_add(record_bytes) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            if next_position <= position || next_position > segment.payload.len() {
                return Err(self.malformed_here(MalformedKind::InvalidMarkerLength));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                vstd::arithmetic::div_mod::lemma_mod_sub_multiples_vanish(
                    segment.payload.len() as int - position as int, record_bytes as int);
            }
            position = next_position;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_tlm_records, 1); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_crg(parser_view(old(self)), segment_view(segment),
                components, owned_extensions_view(old(extensions))),
    ))]
    pub(super) fn parse_crg(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_crg(input, segment_view(segment), components, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_crg); }
        self.charge_extension_metadata(segment, extensions)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
        }
        let expected = crate::refinement::crg_payload_bytes(components);
        if segment.payload.len() != expected || !extensions.component_registrations.is_empty() {
            return Err(self.malformed_at(
                segment.offset,
                if segment.payload.len() == expected {
                    MalformedKind::InvalidMarkerOrder
                } else {
                    MalformedKind::InvalidMarkerLength
                },
            ));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(goal == (Ok(()), charged_parser, crate::specs::part1_extension_components(
                charged_extensions, charged_extensions.metadata_bytes, crate::specs::part1_crg_registrations(segment.payload@))));
            assert(charged_extensions.registrations =~= Seq::empty());
            assert(crate::specs::part1_crg_registrations(segment.payload@.subrange(0, 0)) =~= Seq::empty());
        }
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    position <= segment.payload.len(),
                    position % 4 == 0,
                    segment.payload.len() == 4int * components as int,
                    input == parser_view(old(self)),
                    extension_input == owned_extensions_view(old(extensions)),
                    parser_view(self) == charged_parser,
                    owned_extensions_view(extensions) == crate::specs::part1_extension_components(
                        charged_extensions, charged_extensions.metadata_bytes,
                        crate::specs::part1_crg_registrations(segment.payload@.subrange(0, position as int))),
                    goal == crate::specs::part1_parse_crg(input, segment_view(segment), components, extension_input),
                    goal == (Ok(()), charged_parser, crate::specs::part1_extension_components(
                        charged_extensions, charged_extensions.metadata_bytes, crate::specs::part1_crg_registrations(segment.payload@))),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_segment_byte);
                assert(position as int + 4 <= segment.payload.len());
            }
            extensions
                .component_registrations
                .push(ComponentRegistration {
                    horizontal: read_u16([
                        self.segment_byte(segment, position, 0)?,
                        self.segment_byte(segment, position, 1)?,
                    ]),
                    vertical: read_u16([
                        self.segment_byte(segment, position, 2)?,
                        self.segment_byte(segment, position, 3)?,
                    ]),
                });
            let next_position = position
                .checked_add(4)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if next_position > segment.payload.len() {
                return Err(self.malformed_here(MalformedKind::InvalidMarkerLength));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                let raw = segment.payload@;
                let previous = crate::specs::part1_crg_registrations(raw.subrange(0, position as int));
                let next = crate::specs::part1_crg_registrations(raw.subrange(0, next_position as int));
                let entry = ComponentRegistration {
                    horizontal: crate::specs::part1_marker_value(raw[position as int], raw[position as int + 1]),
                    vertical: crate::specs::part1_marker_value(raw[position as int + 2], raw[position as int + 3]),
                };
                assert(next =~= previous.push(entry)) by {
                    assert forall|index: int| 0 <= index < next.len() implies
                        next[index] == previous.push(entry)[index] by {
                        if index < previous.len() {
                        } else {
                            assert(4 * index == position as int);
                        }
                    };
                }
                assert(extensions.component_registrations@ =~= next);
            }
            position = next_position;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(segment.payload@.subrange(0, position as int) =~= segment.payload@);
            assert((Ok(()), parser_view(self), owned_extensions_view(extensions)) == goal);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_cbd(parser_view(old(self)), segment_view(segment),
                components, owned_extensions_view(old(extensions))),
    ))]
    pub(super) fn parse_cbd(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_cbd(input, segment_view(segment), components, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_cbd); }
        self.charge_extension_metadata(segment, extensions)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
        }
        if segment.payload.len() < 2 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        let repeated = descriptor & 0x8000 != 0;
        let count = descriptor & 0x7fff;
        let descriptor_count = if repeated {
            1_usize
        } else {
            usize::from(count)
        };
        #[cfg(verus_keep_ghost)]
        proof! { assert(descriptor & 0x7fff <= 32767u16) by (bit_vector); }
        let expected = match 2_usize.checked_add(descriptor_count) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        if count != components || segment.payload.len() != expected {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = crate::specs::part1_cbd_run(
                segment_view(segment), 2, Seq::empty(), repeated, components);
        }
        extensions.component_bit_depths.clear();
        let mut position = 2_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    2 <= position <= segment.payload.len(),
                    repeated ==> segment.payload.len() == 3,
                    extensions.component_bit_depths.len() == position - 2,
                    input == parser_view(old(self)),
                    extension_input == owned_extensions_view(old(extensions)),
                    parser_view(self) == charged_parser,
                    owned_extensions_view(extensions) == crate::specs::part1_extension_depths(
                        charged_extensions, extensions.component_bit_depths@),
                    target == crate::specs::part1_cbd_run(segment_view(segment), position as nat,
                        extensions.component_bit_depths@, repeated, components),
                    goal == crate::specs::part1_parse_cbd(input, segment_view(segment), components, extension_input),
                    goal == (target.0, charged_parser,
                        crate::specs::part1_extension_depths(charged_extensions, target.1)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_cbd_run, 1);
                reveal(crate::specs::part1_segment_byte);
            }
            let packed = self.segment_byte(segment, position, 0)?;
            let precision = crate::refinement::cbd_precision(packed);
            if precision > 38 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            extensions.component_bit_depths.push(ComponentBitDepth {
                precision,
                signed: crate::refinement::cbd_signed(packed),
            });
            position = match position.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_cbd_run, 1); }
        if repeated {
            let depth = match extensions.component_bit_depths.first() {
                Some(value) => *value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength)
                    );
                }
            };
            let component_count = usize::from(components);
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant
                        1 <= extensions.component_bit_depths.len(),
                        extensions.component_bit_depths.len() <= if component_count == 0 { 1int } else { component_count as int },
                        forall|index: int| 0 <= index < extensions.component_bit_depths.len() ==>
                            extensions.component_bit_depths@[index] == depth,
                        target.0 == Ok(()),
                        target.1 == Seq::new(if component_count == 0 { 1nat } else { component_count as nat },
                            |index: int| depth),
                        input == parser_view(old(self)),
                        extension_input == owned_extensions_view(old(extensions)),
                        parser_view(self) == charged_parser,
                        owned_extensions_view(extensions) == crate::specs::part1_extension_depths(
                            charged_extensions, extensions.component_bit_depths@),
                        goal == crate::specs::part1_parse_cbd(input, segment_view(segment), components, extension_input),
                        goal == (target.0, charged_parser,
                            crate::specs::part1_extension_depths(charged_extensions, target.1)),
                    decreases component_count as int
                        - extensions.component_bit_depths.len() as int,
                )
            )]
            while extensions.component_bit_depths.len() < component_count {
                extensions.component_bit_depths.push(depth);
            }
            #[cfg(verus_keep_ghost)]
            proof! { assert(extensions.component_bit_depths@ =~= target.1); }
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_siz_outcome(result) == crate::specs::part1_parse_siz(segment_view(segment), self.limits),
    ))]
    pub(super) fn parse_siz(&self, segment: Segment<'a>) -> Result<(ImageSize, u16), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_siz);
            reveal(crate::specs::part1_siz_header);
        }
        if segment.payload.len() < 36 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let xsiz = read_u32([
            segment.payload[2],
            segment.payload[3],
            segment.payload[4],
            segment.payload[5],
        ]);
        let ysiz = read_u32([
            segment.payload[6],
            segment.payload[7],
            segment.payload[8],
            segment.payload[9],
        ]);
        let xosiz = read_u32([
            segment.payload[10],
            segment.payload[11],
            segment.payload[12],
            segment.payload[13],
        ]);
        let yosiz = read_u32([
            segment.payload[14],
            segment.payload[15],
            segment.payload[16],
            segment.payload[17],
        ]);
        let xtsiz = read_u32([
            segment.payload[18],
            segment.payload[19],
            segment.payload[20],
            segment.payload[21],
        ]);
        let ytsiz = read_u32([
            segment.payload[22],
            segment.payload[23],
            segment.payload[24],
            segment.payload[25],
        ]);
        let xtosiz = read_u32([
            segment.payload[26],
            segment.payload[27],
            segment.payload[28],
            segment.payload[29],
        ]);
        let ytosiz = read_u32([
            segment.payload[30],
            segment.payload[31],
            segment.payload[32],
            segment.payload[33],
        ]);
        let components = read_u16([segment.payload[34], segment.payload[35]]);
        let first_tile_x1 = match xtosiz.checked_add(xtsiz) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let first_tile_y1 = match ytosiz.checked_add(ytsiz) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let component_bytes = match usize::from(components).checked_mul(3) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let expected_length = match 36_usize.checked_add(component_bytes) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if segment.payload.len() != expected_length
            || xsiz <= xosiz
            || ysiz <= yosiz
            || xtsiz == 0
            || ytsiz == 0
            || xtosiz > xosiz
            || ytosiz > yosiz
            || first_tile_x1 <= xosiz
            || first_tile_y1 <= yosiz
            || components == 0
        {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidImageGeometry));
        }
        if components > self.limits.maximum_components() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Components,
                limit: u64::from(self.limits.maximum_components()),
                actual: u64::from(components),
            });
        }

        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost h = crate::specs::Part1SizHeader {
                xsiz, ysiz, xosiz, yosiz, xtsiz, ytsiz, xtosiz, ytosiz, components,
                r_siz: crate::specs::part1_marker_value(segment.payload@[0], segment.payload@[1]),
            };
            let ghost component_goal = crate::specs::part1_siz_components(
                segment.payload@, segment.offset, crate::specs::part1_siz_components_initial(),
            );
            let ghost goal = crate::specs::part1_parse_siz(segment_view(segment), self.limits);
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(crate::specs::part1_siz_header(segment_view(segment), self.limits) == Ok(h)); }
        let mut component_offset = 36_usize;
        let mut first_precision = 0_u8;
        let mut first_signed = false;
        let mut components_uniform = true;
        let mut component_index = 0_u16;
        let mut component_info = [ComponentInfo {
            precision: 0,
            signed: false,
            x_subsampling: 0,
            y_subsampling: 0,
            x_registration: 0,
            y_registration: 0,
        }; MAX_COMPONENTS];
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(component_info@ =~= Seq::new(MAX_COMPONENTS as nat, |i: int| crate::specs::part1_empty_component_info()));
            assert(siz_components_view(component_info@, component_offset, component_index,
                first_precision, first_signed, components_uniform) == crate::specs::part1_siz_components_initial());
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_offset <= segment.payload.len(),
                component_index <= components,
                component_offset as int == 36 + 3 * component_index as int,
                segment.payload.len() as int == 36 + 3 * components as int,
                h == (crate::specs::Part1SizHeader {
                    xsiz, ysiz, xosiz, yosiz, xtsiz, ytsiz, xtosiz, ytosiz, components,
                    r_siz: crate::specs::part1_marker_value(segment.payload@[0], segment.payload@[1]),
                }),
                crate::specs::part1_siz_header(segment_view(segment), self.limits) == Ok(h),
                xsiz > xosiz, ysiz > yosiz, xosiz >= xtosiz, yosiz >= ytosiz, xtsiz > 0, ytsiz > 0,
                goal == crate::specs::part1_parse_siz(segment_view(segment), self.limits),
                goal == match component_goal {
                    Err(error) => Err(error),
                    Ok(state) => crate::specs::part1_siz_finish(h, state, segment.offset, self.limits),
                },
                component_goal == crate::specs::part1_siz_components(
                    segment.payload@, segment.offset,
                    siz_components_view(component_info@, component_offset, component_index, first_precision, first_signed, components_uniform),
                ),
                decreases segment.payload.len() - component_offset,
            )
        )]
        while component_offset < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_parse_siz);
                reveal_with_fuel(crate::specs::part1_siz_components, 1);
            }
            if segment.payload.len() - component_offset < 3 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let precision_value = u16::from(segment.payload[component_offset] & 0x7f) + 1;
            let signed = segment.payload[component_offset] & 0x80 != 0;
            if precision_value > 38
                || segment.payload[component_offset + 1] == 0
                || segment.payload[component_offset + 2] == 0
            {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidImageGeometry));
            }
            let precision = match u8::try_from(precision_value) {
                Ok(value) => value,
                Err(_) => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let info_index = usize::from(component_index);
            if info_index >= component_info.len() {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidImageGeometry));
            }
            component_info[info_index] = ComponentInfo {
                precision,
                signed,
                x_subsampling: segment.payload[component_offset + 1],
                y_subsampling: segment.payload[component_offset + 2],
                x_registration: 0,
                y_registration: 0,
            };
            if component_index == 0 {
                first_precision = precision;
                first_signed = signed;
            } else if precision != first_precision || signed != first_signed {
                components_uniform = false;
            }
            component_index = match component_index.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            component_offset = match component_offset.checked_add(3) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(crate::specs::part1_siz_components, 1);
            reveal(crate::specs::part1_siz_finish);
        }

        let width = xsiz - xosiz;
        let height = ysiz - yosiz;
        if width > self.limits.maximum_width() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Width,
                limit: u64::from(self.limits.maximum_width()),
                actual: u64::from(width),
            });
        }
        if height > self.limits.maximum_height() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Height,
                limit: u64::from(self.limits.maximum_height()),
                actual: u64::from(height),
            });
        }
        let pixels = match u64::from(width).checked_mul(u64::from(height)) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if pixels > self.limits.maximum_pixels() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Pixels,
                limit: self.limits.maximum_pixels(),
                actual: pixels,
            });
        }

        let tiles_across = ceil_div(xsiz - xtosiz, xtsiz)?;
        let tiles_down = ceil_div(ysiz - ytosiz, ytsiz)?;
        let tiles = match u64::from(tiles_across).checked_mul(u64::from(tiles_down)) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if tiles > u64::from(self.limits.maximum_tiles()) {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Tiles,
                limit: u64::from(self.limits.maximum_tiles()),
                actual: tiles,
            });
        }

        Ok((
            ImageSize {
                width,
                height,
                x_origin: xosiz,
                y_origin: yosiz,
                components,
                uniform_bit_depth: if components_uniform {
                    Some(first_precision)
                } else {
                    None
                },
                uniform_signed: if components_uniform {
                    Some(first_signed)
                } else {
                    None
                },
                tile_width: xtsiz,
                tile_height: ytsiz,
                tile_x_origin: xtosiz,
                tile_y_origin: ytosiz,
                tiles_across,
                tiles_down,
                component_info,
            },
            read_u16([segment.payload[0], segment.payload[1]]),
        ))
    }
}
