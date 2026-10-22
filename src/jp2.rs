use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits,
    codestream::{self, CodestreamInfo},
    image::{ChannelAssociation, ChannelDefinition, ChannelType},
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const SIGNATURE_MAGIC: [u8; 4] = [0x0d, 0x0a, 0x87, 0x0a];
pub const ENUMERATED_SRGB: u32 = 16;
pub const ENUMERATED_GRAYSCALE: u32 = 17;
pub const ENUMERATED_SYCC: u32 = 18;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct BoxType(u32);

impl BoxType {
    pub const SIGNATURE: Self = Self(0x6a50_2020);
    pub const FILE_TYPE: Self = Self(0x6674_7970);
    pub const JP2_HEADER: Self = Self(0x6a70_3268);
    pub const CODESTREAM_HEADER: Self = Self(0x6a70_6368);
    pub const COMPOSITING_LAYER_HEADER: Self = Self(0x6a70_6c68);
    pub const COMPOSITION: Self = Self(0x636f_6d70);
    pub const COMPOSITION_OPTIONS: Self = Self(0x636f_7074);
    pub const INSTRUCTION_SET: Self = Self(0x696e_7374);
    pub const CODESTREAM_REGISTRATION: Self = Self(0x6372_6567);
    pub const COLOR_GROUP: Self = Self(0x6367_7270);
    pub const OPACITY: Self = Self(0x6f70_6374);
    pub const ASSOCIATION: Self = Self(0x6173_6f63);
    pub const CONTIGUOUS_CODESTREAM: Self = Self(0x6a70_3263);
    pub const IMAGE_HEADER: Self = Self(0x6968_6472);
    pub const BITS_PER_COMPONENT: Self = Self(0x6270_6363);
    pub const COLOR_SPECIFICATION: Self = Self(0x636f_6c72);
    pub const PALETTE: Self = Self(0x7063_6c72);
    pub const COMPONENT_MAPPING: Self = Self(0x636d_6170);
    pub const CHANNEL_DEFINITION: Self = Self(0x6364_6566);
    pub const RESOLUTION: Self = Self(0x7265_7320);
    pub const CAPTURE_RESOLUTION: Self = Self(0x7265_7363);
    pub const DISPLAY_RESOLUTION: Self = Self(0x7265_7364);
    pub const JP2_BRAND: Self = Self(0x6a70_3220);
    pub const JPX_BRAND: Self = Self(0x6a70_7820);
    pub const JPX_BASELINE_BRAND: Self = Self(0x6a70_7862);
    pub const JPH_BRAND: Self = Self(0x6a70_6820);

    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(
            ((bytes[0] as u32) << 24)
                | ((bytes[1] as u32) << 16)
                | ((bytes[2] as u32) << 8)
                | (bytes[3] as u32),
        )
    }

    pub const fn bytes(self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }

    pub const fn equals(self, other: Self) -> bool {
        self.0 == other.0
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct BoxHeader {
    box_type: BoxType,
    offset: u64,
    header_length: u64,
    total_length: u64,
}

impl BoxHeader {
    pub const fn box_type(self) -> BoxType {
        self.box_type
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn header_length(self) -> u64 {
        self.header_length
    }

    pub const fn total_length(self) -> u64 {
        self.total_length
    }

    pub fn payload_offset(self) -> Result<u64, DecodeError> {
        self.offset
            .checked_add(self.header_length)
            .ok_or_else(|| DecodeError::malformed(self.offset, MalformedKind::ArithmeticOverflow))
    }

    pub fn payload_length(self) -> Result<u64, DecodeError> {
        self.total_length
            .checked_sub(self.header_length)
            .ok_or_else(|| DecodeError::malformed(self.offset, MalformedKind::InvalidBoxLength))
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct BoxRef<'a> {
    pub header: BoxHeader,
    pub payload: &'a [u8],
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug))]
pub struct BoxReader<'a> {
    input: &'a [u8],
    limits: ResourceLimits,
    position: usize,
    metadata_bytes: u64,
    work_units: u64,
    failed: bool,
}

impl<'a> BoxReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        let input_length = input.len() as u64;
        if input_length > limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                limits.maximum_input_bytes(),
                input_length,
            ));
        }
        Ok(Self {
            input,
            limits: *limits,
            position: 0,
            metadata_bytes: 0,
            work_units: 0,
            failed: false,
        })
    }

    pub const fn metadata_bytes(&self) -> u64 {
        self.metadata_bytes
    }

    pub fn next_box_ref(&mut self) -> Option<Result<BoxRef<'a>, DecodeError>> {
        if self.failed {
            return None;
        }
        match self.next_box() {
            Ok(Some(box_ref)) => Some(Ok(box_ref)),
            Ok(None) => None,
            Err(error) => {
                self.failed = true;
                Some(Err(error))
            }
        }
    }

    fn next_box(&mut self) -> Result<Option<BoxRef<'a>>, DecodeError> {
        if self.position == self.input.len() {
            return Ok(None);
        }

        self.work_units = self
            .work_units
            .checked_add(1)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if self.work_units > self.limits.maximum_work_units() {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                self.limits.maximum_work_units(),
                self.work_units,
            ));
        }

        let offset = self.position;
        let remaining = self
            .input
            .len()
            .checked_sub(offset)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if remaining < 8 {
            return Err(self.malformed_here(MalformedKind::Truncated));
        }

        let length32 = read_u32([
            self.input[offset],
            self.input[offset + 1],
            self.input[offset + 2],
            self.input[offset + 3],
        ]);
        let box_type = BoxType::new([
            self.input[offset + 4],
            self.input[offset + 5],
            self.input[offset + 6],
            self.input[offset + 7],
        ]);
        let (header_length, total_length) = match length32 {
            0 => (8_u64, remaining as u64),
            1 => {
                if remaining < 16 {
                    return Err(self.malformed_here(MalformedKind::Truncated));
                }
                (
                    16_u64,
                    read_u64([
                        self.input[offset + 8],
                        self.input[offset + 9],
                        self.input[offset + 10],
                        self.input[offset + 11],
                        self.input[offset + 12],
                        self.input[offset + 13],
                        self.input[offset + 14],
                        self.input[offset + 15],
                    ]),
                )
            }
            2..=7 => return Err(self.malformed_here(MalformedKind::InvalidBoxLength)),
            value => (8_u64, u64::from(value)),
        };

        if total_length < header_length {
            return Err(self.malformed_here(MalformedKind::InvalidBoxLength));
        }
        let total_length_usize = usize::try_from(total_length)
            .map_err(|_| self.malformed_here(MalformedKind::InvalidBoxLength))?;
        let end = offset
            .checked_add(total_length_usize)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if end > self.input.len() {
            return Err(self.malformed_here(MalformedKind::Truncated));
        }
        let header_length_usize = usize::try_from(header_length)
            .map_err(|_| self.malformed_here(MalformedKind::InvalidBoxLength))?;
        let payload_start = offset
            .checked_add(header_length_usize)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;

        if !box_type.equals(BoxType::CONTIGUOUS_CODESTREAM) {
            self.metadata_bytes = self
                .metadata_bytes
                .checked_add(total_length)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if self.metadata_bytes > self.limits.maximum_metadata_bytes() {
                return Err(DecodeError::limit(
                    Resource::MetadataBytes,
                    self.limits.maximum_metadata_bytes(),
                    self.metadata_bytes,
                ));
            }
        }

        self.position = end;
        Ok(Some(BoxRef {
            header: BoxHeader {
                box_type,
                offset: offset as u64,
                header_length,
                total_length,
            },
            payload: &self.input[payload_start..end],
        }))
    }

    fn malformed_here(&self, kind: MalformedKind) -> DecodeError {
        DecodeError::malformed(self.position as u64, kind)
    }
}

#[cfg(not(verus_keep_ghost))]
impl<'a> Iterator for BoxReader<'a> {
    type Item = Result<BoxRef<'a>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_box_ref()
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct FileType {
    pub brand: BoxType,
    pub minor_version: u32,
    compatible_brands: Vec<BoxType>,
}

impl FileType {
    pub fn compatible_brands(&self) -> &[BoxType] {
        &self.compatible_brands
    }

    pub fn is_compatible_with(&self, brand: BoxType) -> bool {
        if self.brand.equals(brand) {
            return true;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.compatible_brands.len(),
                decreases self.compatible_brands.len() - index,
            )
        )]
        while index < self.compatible_brands.len() {
            let compatible = match self.compatible_brands.get(index) {
                Some(value) => value,
                None => return false,
            };
            if compatible.equals(brand) {
                return true;
            }
            index += 1;
        }
        false
    }

    pub fn is_jpx_compatible(&self) -> bool {
        self.is_compatible_with(BoxType::JPX_BRAND)
            || self.is_compatible_with(BoxType::JPX_BASELINE_BRAND)
    }

    pub fn is_jph_compatible(&self) -> bool {
        self.is_compatible_with(BoxType::JPH_BRAND)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ComponentBitDepth {
    Uniform { bits: u8, signed: bool },
    Variable,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ImageHeader {
    pub width: u32,
    pub height: u32,
    pub components: u16,
    pub bit_depth: ComponentBitDepth,
    pub unknown_color_space: bool,
    pub intellectual_property: bool,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ColorMethod {
    Enumerated { color_space: u32 },
    RestrictedIcc { profile_bytes: u64 },
    AnyIcc { profile_bytes: u64 },
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ColorSpecification {
    pub method: ColorMethod,
    pub precedence: u8,
    pub approximation: u8,
    icc_profile: Option<Vec<u8>>,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn color_specification_method(color: &ColorSpecification) -> ColorMethod {
        color.method
    }
}

impl ColorSpecification {
    pub const fn enumerated(color_space: u32) -> Self {
        Self {
            method: ColorMethod::Enumerated { color_space },
            precedence: 0,
            approximation: 0,
            icc_profile: None,
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(method => ensures method == color_specification_method(self),)
    )]
    pub const fn method(&self) -> ColorMethod {
        self.method
    }

    pub fn icc_profile(&self) -> Option<&[u8]> {
        match &self.icc_profile {
            Some(profile) => Some(profile.as_slice()),
            None => None,
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Resolution {
    pub vertical_numerator: u16,
    pub vertical_denominator: u16,
    pub horizontal_numerator: u16,
    pub horizontal_denominator: u16,
    pub vertical_exponent: i8,
    pub horizontal_exponent: i8,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Jp2Header {
    pub image: ImageHeader,
    pub color: ColorSpecification,
    pub nested_box_count: u32,
    pub palette_entries: u16,
    pub palette_columns: u8,
    pub component_mapping_entries: u16,
    pub channel_definitions: u16,
    pub palette: Option<Palette>,
    pub component_mappings: Vec<ComponentMapping>,
    pub channel_definition_records: Vec<ChannelDefinitionRecord>,
    pub capture_resolution: Option<Resolution>,
    pub display_resolution: Option<Resolution>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Jp2File<'a> {
    pub file_type: FileType,
    pub header: Jp2Header,
    codestreams: Vec<CodestreamBox<'a>>,
    compositing_layers: Vec<CompositingLayer<'a>>,
    composition: Option<Composition>,
    metadata_boxes: u32,
    metadata_depth: u32,
    pub box_count: u64,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodestreamRegistration {
    pub codestream_index: u16,
    pub horizontal_subsampling: u8,
    pub vertical_subsampling: u8,
    pub horizontal_offset: u32,
    pub vertical_offset: u32,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CompositingLayer<'a> {
    header: BoxHeader,
    payload: &'a [u8],
    horizontal_grid_size: u16,
    vertical_grid_size: u16,
    registrations: Vec<CodestreamRegistration>,
    color_group: Vec<ColorSpecification>,
    channel_definition_records: Vec<ChannelDefinitionRecord>,
    opacity: Option<LayerOpacity>,
    nested_metadata_boxes: u32,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum LayerOpacity {
    Straight,
    Premultiplied,
    ChromaKey { channels: u8, values: Vec<u8> },
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CompositionInstruction {
    pub target_x: u32,
    pub target_y: u32,
    pub target_width: u32,
    pub target_height: u32,
    pub life: u32,
    pub persistent: bool,
    pub next_reuse: u32,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CompositionInstructionSet {
    pub flags: u16,
    pub repeat: u16,
    pub tick: u32,
    pub instructions: Vec<CompositionInstruction>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Composition {
    pub width: u32,
    pub height: u32,
    pub loop_count: u8,
    pub instruction_sets: Vec<CompositionInstructionSet>,
    pub has_unknown_boxes: bool,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct InspectedJp2<'a> {
    pub file: Jp2File<'a>,
    codestreams: Vec<CodestreamInfo>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodestreamHeaderAssociation<'a> {
    pub header: BoxHeader,
    pub payload: &'a [u8],
    parsed: Option<Jp2Header>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodestreamBox<'a> {
    pub header: BoxHeader,
    pub data: &'a [u8],
    codestream_header: Option<CodestreamHeaderAssociation<'a>>,
}

impl<'a> CodestreamBox<'a> {
    pub const fn box_header(&self) -> BoxHeader {
        self.header
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn codestream_header(&self) -> Option<&CodestreamHeaderAssociation<'a>> {
        self.codestream_header.as_ref()
    }
}

impl CodestreamHeaderAssociation<'_> {
    pub const fn box_header(&self) -> BoxHeader {
        self.header
    }

    pub fn payload(&self) -> &[u8] {
        self.payload
    }

    pub fn parsed_header(&self) -> Option<&Jp2Header> {
        self.parsed.as_ref()
    }
}

impl<'a> Jp2File<'a> {
    pub fn file_type(&self) -> &FileType {
        &self.file_type
    }

    pub fn header(&self) -> &Jp2Header {
        &self.header
    }

    pub fn compositing_layer_count(&self) -> u32 {
        self.compositing_layers.len() as u32
    }

    pub const fn has_composition(&self) -> bool {
        self.composition.is_some()
    }

    pub fn compositing_layers(&self) -> &[CompositingLayer<'a>] {
        &self.compositing_layers
    }

    pub fn compositing_layer(&self, index: usize) -> Result<&CompositingLayer<'a>, DecodeError> {
        self.compositing_layers
            .get(index)
            .ok_or(DecodeError::LayerIndexOutOfRange {
                index,
                layers: self.compositing_layers.len(),
            })
    }

    pub const fn composition(&self) -> Option<&Composition> {
        self.composition.as_ref()
    }

    pub const fn metadata_boxes(&self) -> u32 {
        self.metadata_boxes
    }

    pub const fn metadata_depth(&self) -> u32 {
        self.metadata_depth
    }

    pub fn codestream_count(&self) -> usize {
        self.codestreams.len()
    }

    pub fn codestream(&self, index: usize) -> Result<&CodestreamBox<'a>, DecodeError> {
        self.codestreams
            .get(index)
            .ok_or(DecodeError::CodestreamIndexOutOfRange {
                index,
                codestreams: self.codestreams.len(),
            })
    }

    pub fn codestreams(&self) -> &[CodestreamBox<'a>] {
        &self.codestreams
    }
}

impl CompositingLayer<'_> {
    pub const fn box_header(&self) -> BoxHeader {
        self.header
    }

    pub fn payload(&self) -> &[u8] {
        self.payload
    }

    pub const fn horizontal_grid_size(&self) -> u16 {
        self.horizontal_grid_size
    }

    pub const fn vertical_grid_size(&self) -> u16 {
        self.vertical_grid_size
    }

    pub fn registrations(&self) -> &[CodestreamRegistration] {
        &self.registrations
    }

    pub const fn has_color_group(&self) -> bool {
        !self.color_group.is_empty()
    }

    pub const fn has_opacity_box(&self) -> bool {
        self.opacity.is_some()
    }

    pub fn color_group(&self) -> &[ColorSpecification] {
        &self.color_group
    }

    pub fn channel_definition_records(&self) -> &[ChannelDefinitionRecord] {
        &self.channel_definition_records
    }

    pub const fn opacity(&self) -> Option<&LayerOpacity> {
        self.opacity.as_ref()
    }

    pub const fn nested_metadata_boxes(&self) -> u32 {
        self.nested_metadata_boxes
    }
}

impl<'a> InspectedJp2<'a> {
    pub fn file(&self) -> &Jp2File<'a> {
        &self.file
    }

    pub fn codestream_count(&self) -> usize {
        self.codestreams.len()
    }

    pub fn codestream(&self, index: usize) -> Result<&CodestreamInfo, DecodeError> {
        self.codestreams
            .get(index)
            .ok_or(DecodeError::CodestreamIndexOutOfRange {
                index,
                codestreams: self.codestreams.len(),
            })
    }

    pub fn codestreams(&self) -> &[CodestreamInfo] {
        &self.codestreams
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PaletteColumn {
    pub precision: u8,
    pub signed: bool,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Palette {
    entries: u16,
    columns: Vec<PaletteColumn>,
    values: Vec<i64>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum MappingType {
    Direct,
    Palette { column: u8 },
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ComponentMapping {
    pub component: u16,
    pub mapping_type: MappingType,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ChannelDefinitionRecord {
    pub channel: u16,
    pub definition: ChannelDefinition,
}

impl Palette {
    pub const fn entries(&self) -> u16 {
        self.entries
    }

    pub fn columns(&self) -> &[PaletteColumn] {
        &self.columns
    }

    pub fn value(&self, entry: u16, column: u8) -> Result<i64, DecodeError> {
        let columns = u8::try_from(self.columns.len())
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if !crate::refinement::palette_coordinates_valid(entry, self.entries, column, columns) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
        }
        let index = usize::from(entry)
            .checked_mul(self.columns.len())
            .and_then(|row| row.checked_add(usize::from(column)))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        match self.values.get(index) {
            Some(value) => Ok(*value),
            None => Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents)),
        }
    }
}

pub fn parse_and_inspect<'a>(
    input: &'a [u8],
    limits: &ResourceLimits,
) -> Result<InspectedJp2<'a>, DecodeError> {
    let file = parse(input, limits)?;
    let codestream_count = file.codestream_count();
    let mut inspected_codestreams = Vec::with_capacity(codestream_count);
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= codestream_count,
            decreases codestream_count - index,
        )
    )]
    while index < codestream_count {
        let codestream_box = file.codestream(index)?;
        let info = codestream::parse(codestream_box.data, limits)?;
        let jph = file.file_type.is_jph_compatible();
        if jph != info.profile.is_part15() {
            return Err(DecodeError::malformed(
                codestream_box.header.offset(),
                MalformedKind::InvalidProfile,
            ));
        }
        let associated_header = match codestream_box.codestream_header() {
            Some(association) => association.parsed.as_ref(),
            None if index == 0 => Some(&file.header),
            None => None,
        };
        if let Some(header) = associated_header {
            validate_container_codestream(header, &info, codestream_box.header.offset())?;
        }
        inspected_codestreams.push(info);
        index = index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    Ok(InspectedJp2 {
        file,
        codestreams: inspected_codestreams,
    })
}

pub fn inspect_jph<'a>(
    input: &'a [u8],
    limits: &ResourceLimits,
) -> Result<InspectedJp2<'a>, DecodeError> {
    let inspected = parse_and_inspect(input, limits)?;
    if !inspected.file.file_type.is_jph_compatible() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    Ok(inspected)
}

fn validate_container_codestream(
    header: &Jp2Header,
    codestream: &CodestreamInfo,
    offset: u64,
) -> Result<(), DecodeError> {
    if header.image.width != codestream.image_size.width
        || header.image.height != codestream.image_size.height
        || header.image.components != codestream.image_size.components
    {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::ContainerCodestreamMismatch,
        ));
    }

    if let ComponentBitDepth::Uniform { bits, signed } = header.image.bit_depth {
        let matches_depth = match (
            codestream.image_size.uniform_bit_depth,
            codestream.image_size.uniform_signed,
        ) {
            (Some(codestream_bits), Some(codestream_signed)) => {
                codestream_bits == bits && codestream_signed == signed
            }
            _ => false,
        };
        if !matches_depth {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::ContainerCodestreamMismatch,
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_selected_header(
    header: &Jp2Header,
    codestream: &CodestreamInfo,
    offset: u64,
) -> Result<(), DecodeError> {
    validate_container_codestream(header, codestream, offset)
}

pub fn parse<'a>(input: &'a [u8], limits: &ResourceLimits) -> Result<Jp2File<'a>, DecodeError> {
    let mut reader = BoxReader::new(input, limits)?;
    let signature = required_box(&mut reader, 0)?;
    if !signature.header.box_type().equals(BoxType::SIGNATURE)
        || signature.header.total_length() != 12
        || signature.payload.len() != SIGNATURE_MAGIC.len()
        || signature.payload[0] != SIGNATURE_MAGIC[0]
        || signature.payload[1] != SIGNATURE_MAGIC[1]
        || signature.payload[2] != SIGNATURE_MAGIC[2]
        || signature.payload[3] != SIGNATURE_MAGIC[3]
    {
        return Err(DecodeError::malformed(
            signature.header.offset(),
            MalformedKind::InvalidSignature,
        ));
    }

    let file_type_box = required_box(&mut reader, 1)?;
    if !file_type_box.header.box_type().equals(BoxType::FILE_TYPE) {
        return Err(DecodeError::malformed(
            file_type_box.header.offset(),
            MalformedKind::InvalidBoxOrder,
        ));
    }
    let file_type = parse_file_type(file_type_box, limits)?;
    validate_file_type_profile(&file_type, file_type_box.header.offset())?;
    if !file_type.is_compatible_with(BoxType::JP2_BRAND)
        && !file_type.is_jpx_compatible()
        && !file_type.is_compatible_with(BoxType::JPH_BRAND)
    {
        return Err(DecodeError::UnsupportedProfile {
            feature: "unrecognized JPEG 2000 file brand",
        });
    }

    let mut parsed_header = None;
    let mut codestream_boxes = Vec::new();
    let mut codestream_headers = Vec::new();
    let mut compositing_layers = Vec::new();
    let mut composition = None;
    let mut metadata_boxes = 0_u32;
    let mut metadata_depth = 0_u32;
    let mut box_count = 2_u64;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases input.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > input.len() {
            return Err(DecodeError::malformed(
                previous_position as u64,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let box_ref = next?;
        box_count = box_count
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let box_type = box_ref.header.box_type();
        if box_type.equals(BoxType::JP2_HEADER) {
            if parsed_header.is_some()
                || !codestream_boxes.is_empty()
                || !codestream_headers.is_empty()
                || !compositing_layers.is_empty()
                || composition.is_some()
            {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            parsed_header = Some(parse_jp2_header(
                box_ref.payload,
                limits,
                box_ref.header.payload_offset()?,
            )?);
        } else if box_type.equals(BoxType::CODESTREAM_HEADER) {
            if !file_type.is_jpx_compatible() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidFileType,
                ));
            }
            if parsed_header.is_none() || !codestream_boxes.is_empty() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let actual = u64::try_from(codestream_headers.len())
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    DecodeError::malformed(
                        box_ref.header.offset(),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            if actual > u64::from(limits.maximum_codestreams()) {
                return Err(DecodeError::limit(
                    Resource::Codestreams,
                    u64::from(limits.maximum_codestreams()),
                    actual,
                ));
            }
            let payload_offset = box_ref.header.payload_offset()?;
            let parsed = match parse_codestream_header(
                box_ref.payload,
                parsed_header.as_ref(),
                limits,
                payload_offset,
            ) {
                Ok(header) => Some(header),
                Err(error) => return Err(error),
            };
            codestream_headers.push(CodestreamHeaderAssociation {
                header: box_ref.header,
                payload: box_ref.payload,
                parsed,
            });
        } else if box_type.equals(BoxType::COMPOSITING_LAYER_HEADER) {
            if !file_type.is_jpx_compatible() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidFileType,
                ));
            }
            if parsed_header.is_none() || !codestream_boxes.is_empty() || composition.is_some() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let actual = compositing_layers.len().checked_add(1).ok_or_else(|| {
                DecodeError::malformed(box_ref.header.offset(), MalformedKind::ArithmeticOverflow)
            })?;
            if actual as u64 > u64::from(limits.maximum_layers()) {
                return Err(DecodeError::limit(
                    Resource::Layers,
                    u64::from(limits.maximum_layers()),
                    actual as u64,
                ));
            }
            compositing_layers.push(parse_compositing_layer(box_ref, limits)?);
        } else if box_type.equals(BoxType::COMPOSITION) {
            if !file_type.is_jpx_compatible() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidFileType,
                ));
            }
            if parsed_header.is_none()
                || !codestream_boxes.is_empty()
                || composition.is_some()
                || compositing_layers.is_empty()
            {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            composition = Some(parse_composition(box_ref, limits)?);
        } else if box_type.equals(BoxType::CONTIGUOUS_CODESTREAM) {
            if parsed_header.is_none() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let actual = u64::try_from(codestream_boxes.len())
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    DecodeError::malformed(
                        box_ref.header.offset(),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            if actual > u64::from(limits.maximum_codestreams()) {
                return Err(DecodeError::limit(
                    Resource::Codestreams,
                    u64::from(limits.maximum_codestreams()),
                    actual,
                ));
            }
            if actual > 1 && !file_type.is_jpx_compatible() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxContents,
                ));
            }
            codestream_boxes.push(CodestreamBox {
                header: box_ref.header,
                data: box_ref.payload,
                codestream_header: None,
            });
        } else {
            let (count, depth) = inspect_metadata_box(box_ref, limits, 1, u32::MAX - 1)?;
            metadata_boxes = metadata_boxes
                .checked_add(count)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            metadata_depth = metadata_depth.max(depth);
        }
    }

    let header = parsed_header
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
    if codestream_boxes.is_empty() {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    if codestream_headers.len() > codestream_boxes.len() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
    let mut header_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant header_index <= codestream_headers.len(),
            decreases codestream_headers.len() - header_index,
        )
    )]
    while header_index < codestream_headers.len() {
        let association = codestream_headers
            .get(header_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?
            .clone();
        let codestream_box = codestream_boxes
            .get_mut(header_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?;
        codestream_box.codestream_header = Some(association);
        header_index = header_index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    validate_layer_references(&compositing_layers, codestream_boxes.len())?;
    Ok(Jp2File {
        file_type,
        header,
        codestreams: codestream_boxes,
        compositing_layers,
        composition,
        metadata_boxes,
        metadata_depth,
        box_count,
    })
}

fn parse_compositing_layer<'a>(
    box_ref: BoxRef<'a>,
    limits: &ResourceLimits,
) -> Result<CompositingLayer<'a>, DecodeError> {
    let base_offset = box_ref.header.payload_offset()?;
    let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut horizontal_grid_size = 1_u16;
    let mut vertical_grid_size = 1_u16;
    let mut registrations = Vec::new();
    let mut has_registration = false;
    let mut color_group = Vec::new();
    let mut channel_definition_records = Vec::new();
    let mut opacity = None;
    let mut nested_metadata_boxes = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases box_ref.payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > box_ref.payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let child = next.map_err(|error| rebase_error(error, base_offset))?;
        let child_type = child.header.box_type();
        if child_type.equals(BoxType::CODESTREAM_REGISTRATION) {
            if has_registration {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let parsed = parse_codestream_registration(child, base_offset, limits)?;
            horizontal_grid_size = parsed.0;
            vertical_grid_size = parsed.1;
            registrations = parsed.2;
            has_registration = true;
        } else if child_type.equals(BoxType::COLOR_GROUP) {
            if !color_group.is_empty() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            color_group = parse_color_group(
                child,
                limits,
                add_offset(base_offset, child.header.payload_offset()?)?,
            )?;
        } else if child_type.equals(BoxType::OPACITY) {
            if opacity.is_some() || !channel_definition_records.is_empty() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            opacity = Some(parse_layer_opacity(child, base_offset)?);
        } else if child_type.equals(BoxType::CHANNEL_DEFINITION) {
            if !channel_definition_records.is_empty() || opacity.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            channel_definition_records = parse_channel_definition(child, u16::MAX, base_offset)?;
        } else {
            let (count, _) = inspect_metadata_box(child, limits, 1, u32::MAX - 1)?;
            nested_metadata_boxes = nested_metadata_boxes.checked_add(count).ok_or_else(|| {
                DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
            })?;
        }
    }
    Ok(CompositingLayer {
        header: box_ref.header,
        payload: box_ref.payload,
        horizontal_grid_size,
        vertical_grid_size,
        registrations,
        color_group,
        channel_definition_records,
        opacity,
        nested_metadata_boxes,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(decreases remaining_depth,))]
fn inspect_metadata_box(
    box_ref: BoxRef<'_>,
    limits: &ResourceLimits,
    depth: u32,
    remaining_depth: u32,
) -> Result<(u32, u32), DecodeError> {
    if depth > limits.maximum_box_depth() {
        return Err(DecodeError::limit(
            Resource::BoxDepth,
            u64::from(limits.maximum_box_depth()),
            u64::from(depth),
        ));
    }
    if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
    let child_depth = depth
        .checked_add(1)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut count = 1_u32;
    let mut maximum_depth = depth;
    let mut remaining_boxes = box_ref.payload.len();
    #[cfg_attr(verus_keep_ghost, verus_spec(decreases remaining_boxes,))]
    loop {
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if remaining_boxes == 0 {
            return Err(DecodeError::malformed(
                box_ref.header.offset(),
                MalformedKind::InvalidBoxLength,
            ));
        }
        remaining_boxes -= 1;
        let child = next?;
        let child_remaining = remaining_depth
            .checked_sub(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let (child_count, observed_depth) =
            inspect_metadata_box(child, limits, child_depth, child_remaining)?;
        if count > u32::MAX - child_count {
            return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
        }
        count = crate::refinement::metadata_count(count, child_count);
        if u64::from(count) > limits.maximum_work_units() {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                limits.maximum_work_units(),
                u64::from(count),
            ));
        }
        maximum_depth = maximum_depth.max(observed_depth);
    }
    Ok((count, maximum_depth))
}

#[allow(clippy::manual_is_multiple_of)]
fn parse_codestream_registration(
    box_ref: BoxRef<'_>,
    base_offset: u64,
    limits: &ResourceLimits,
) -> Result<(u16, u16, Vec<CodestreamRegistration>), DecodeError> {
    if box_ref.payload.len() < 10 || (box_ref.payload.len() - 4) % 6 != 0 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let horizontal_grid_size = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let vertical_grid_size = read_u16([box_ref.payload[2], box_ref.payload[3]]);
    if horizontal_grid_size == 0 || vertical_grid_size == 0 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxContents,
        ));
    }
    let count = (box_ref.payload.len() - 4) / 6;
    if count as u64 > u64::from(limits.maximum_codestreams()) {
        return Err(DecodeError::limit(
            Resource::Codestreams,
            u64::from(limits.maximum_codestreams()),
            count as u64,
        ));
    }
    let mut registrations = Vec::with_capacity(count);
    let mut position = 4_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= box_ref.payload.len(),
            decreases box_ref.payload.len() - position,
        )
    )]
    while position < box_ref.payload.len() {
        let horizontal_subsampling = byte_at_checked(box_ref.payload, position, 2, base_offset)?;
        let vertical_subsampling = byte_at_checked(box_ref.payload, position, 3, base_offset)?;
        let horizontal_offset = byte_at_checked(box_ref.payload, position, 4, base_offset)?;
        let vertical_offset = byte_at_checked(box_ref.payload, position, 5, base_offset)?;
        if horizontal_subsampling == 0
            || vertical_subsampling == 0
            || u16::from(horizontal_offset) >= horizontal_grid_size
            || u16::from(vertical_offset) >= vertical_grid_size
        {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
        registrations.push(CodestreamRegistration {
            codestream_index: read_u16([
                byte_at_checked(box_ref.payload, position, 0, base_offset)?,
                byte_at_checked(box_ref.payload, position, 1, base_offset)?,
            ]),
            horizontal_subsampling,
            vertical_subsampling,
            horizontal_offset: u32::from(horizontal_offset),
            vertical_offset: u32::from(vertical_offset),
        });
        position = position.checked_add(6).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
        if position > box_ref.payload.len() {
            return Err(DecodeError::malformed(
                base_offset,
                MalformedKind::InvalidBoxLength,
            ));
        }
    }
    Ok((horizontal_grid_size, vertical_grid_size, registrations))
}

fn byte_at_checked(
    input: &[u8],
    position: usize,
    delta: usize,
    offset: u64,
) -> Result<u8, DecodeError> {
    let index = position
        .checked_add(delta)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    match input.get(index) {
        Some(value) => Ok(*value),
        None => Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxLength,
        )),
    }
}

fn parse_color_group(
    box_ref: BoxRef<'_>,
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<Vec<ColorSpecification>, DecodeError> {
    let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut colors = Vec::new();
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases box_ref.payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > box_ref.payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let child = next.map_err(|error| rebase_error(error, base_offset))?;
        if !child.header.box_type().equals(BoxType::COLOR_SPECIFICATION) {
            return Err(DecodeError::malformed(
                add_offset(base_offset, child.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
        colors.push(parse_color_specification(child, base_offset)?);
        if colors.len() as u64 > u64::from(limits.maximum_components()) {
            return Err(DecodeError::limit(
                Resource::Components,
                u64::from(limits.maximum_components()),
                colors.len() as u64,
            ));
        }
    }
    if colors.is_empty() {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    Ok(colors)
}

fn parse_layer_opacity(box_ref: BoxRef<'_>, base_offset: u64) -> Result<LayerOpacity, DecodeError> {
    let offset = add_offset(base_offset, box_ref.header.offset())?;
    let Some(opacity_type) = box_ref.payload.first() else {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxLength,
        ));
    };
    match *opacity_type {
        0 if box_ref.payload.len() == 1 => Ok(LayerOpacity::Straight),
        1 if box_ref.payload.len() == 1 => Ok(LayerOpacity::Premultiplied),
        2 => {
            if box_ref.payload.len() < 3 {
                return Err(DecodeError::malformed(
                    offset,
                    MalformedKind::InvalidBoxLength,
                ));
            }
            let channels = box_ref.payload[1];
            if channels == 0 {
                return Err(DecodeError::malformed(
                    offset,
                    MalformedKind::InvalidBoxContents,
                ));
            }
            Ok(LayerOpacity::ChromaKey {
                channels,
                values: copy_bytes(&box_ref.payload[2..])?,
            })
        }
        0..=2 => Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxLength,
        )),
        _ => Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxContents,
        )),
    }
}

fn copy_bytes(input: &[u8]) -> Result<Vec<u8>, DecodeError> {
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
        output.push(
            *input
                .get(index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        index = index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    Ok(output)
}

#[allow(clippy::manual_is_multiple_of)]
fn parse_instruction_set(
    box_ref: BoxRef<'_>,
    composition: (u32, u32, u8),
    base_offset: u64,
    limits: &ResourceLimits,
) -> Result<CompositionInstructionSet, DecodeError> {
    let offset = add_offset(base_offset, box_ref.header.offset())?;
    if box_ref.payload.len() < 8 {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let flags = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    if flags & !0x0027 != 0 {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxContents,
        ));
    }
    let repeat = read_u16([box_ref.payload[2], box_ref.payload[3]]);
    let tick = read_u32([
        box_ref.payload[4],
        box_ref.payload[5],
        box_ref.payload[6],
        box_ref.payload[7],
    ]);
    let have_target_position = flags & 1 != 0;
    let have_target_size = flags & 2 != 0;
    let have_life = flags & 4 != 0;
    let have_source_region = flags & 32 != 0;
    let mut words = 0_usize;
    if have_target_position {
        words = words
            .checked_add(2)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    }
    if have_target_size {
        words = words
            .checked_add(2)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    }
    if have_life {
        words = words
            .checked_add(2)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    }
    if have_source_region {
        words = words
            .checked_add(4)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    }
    let record_bytes = words
        .checked_mul(4)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    let remaining = box_ref.payload.len() - 8;
    if record_bytes == 0 {
        if remaining != 0 {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidBoxLength,
            ));
        }
        return Ok(CompositionInstructionSet {
            flags,
            repeat,
            tick,
            instructions: Vec::new(),
        });
    }
    if remaining % record_bytes != 0 {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let count = remaining / record_bytes;
    if count as u64 > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            count as u64,
        ));
    }
    let mut instructions = Vec::with_capacity(count);
    let mut position = 8_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= box_ref.payload.len(),
            decreases box_ref.payload.len() - position,
        )
    )]
    while position < box_ref.payload.len() {
        let record_end = position
            .checked_add(record_bytes)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        if record_end > box_ref.payload.len() {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidBoxLength,
            ));
        }
        if record_end <= position {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::ArithmeticOverflow,
            ));
        }
        let mut cursor = position;
        let mut target_x = 0;
        let mut target_y = 0;
        let mut target_width = composition.0;
        let mut target_height = composition.1;
        let mut life_value = 0;
        let mut persistent = true;
        let mut next_reuse = 0;
        let mut source_x = 0;
        let mut source_y = 0;
        let mut source_width = 0;
        let mut source_height = 0;
        if have_target_position {
            target_x = read_u32_at(box_ref.payload, &mut cursor, offset)?;
            target_y = read_u32_at(box_ref.payload, &mut cursor, offset)?;
        }
        if have_target_size {
            target_width = read_u32_at(box_ref.payload, &mut cursor, offset)?;
            target_height = read_u32_at(box_ref.payload, &mut cursor, offset)?;
        }
        if have_life {
            let encoded_life = read_u32_at(box_ref.payload, &mut cursor, offset)?;
            persistent = encoded_life & 0x8000_0000 != 0;
            life_value = encoded_life & 0x7fff_ffff;
            next_reuse = read_u32_at(box_ref.payload, &mut cursor, offset)?;
        }
        if have_source_region {
            source_x = read_u32_at(box_ref.payload, &mut cursor, offset)?;
            source_y = read_u32_at(box_ref.payload, &mut cursor, offset)?;
            source_width = read_u32_at(box_ref.payload, &mut cursor, offset)?;
            source_height = read_u32_at(box_ref.payload, &mut cursor, offset)?;
        }
        if cursor != record_end {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidBoxLength,
            ));
        }
        if target_width == 0
            || target_height == 0
            || (have_source_region && (source_width == 0 || source_height == 0))
        {
            return Err(DecodeError::malformed(
                offset,
                MalformedKind::InvalidImageGeometry,
            ));
        }
        let life = life_value
            .checked_mul(tick)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        instructions.push(CompositionInstruction {
            target_x,
            target_y,
            target_width,
            target_height,
            life,
            persistent,
            next_reuse,
            source_x,
            source_y,
            source_width,
            source_height,
        });
        position = record_end;
    }
    Ok(CompositionInstructionSet {
        flags,
        repeat,
        tick,
        instructions,
    })
}

fn read_u32_at(input: &[u8], position: &mut usize, offset: u64) -> Result<u32, DecodeError> {
    let end = position
        .checked_add(4)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    let bytes = input
        .get(*position..end)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::InvalidBoxLength))?;
    *position = end;
    Ok(read_u32([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn parse_composition(
    box_ref: BoxRef<'_>,
    limits: &ResourceLimits,
) -> Result<Composition, DecodeError> {
    let base_offset = box_ref.header.payload_offset()?;
    let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut options = None;
    let mut instruction_sets = Vec::new();
    let mut has_unknown_boxes = false;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases box_ref.payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > box_ref.payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let child = next.map_err(|error| rebase_error(error, base_offset))?;
        let child_type = child.header.box_type();
        if child_type.equals(BoxType::COMPOSITION_OPTIONS) {
            if options.is_some() || !instruction_sets.is_empty() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            if child.payload.len() != 9 {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxLength,
                ));
            }
            let height = read_u32([
                child.payload[0],
                child.payload[1],
                child.payload[2],
                child.payload[3],
            ]);
            let width = read_u32([
                child.payload[4],
                child.payload[5],
                child.payload[6],
                child.payload[7],
            ]);
            validate_dimensions(width, height, limits, base_offset)?;
            options = Some((width, height, child.payload[8]));
        } else if child_type.equals(BoxType::INSTRUCTION_SET) {
            if options.is_none() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let instruction_set = parse_instruction_set(
                child,
                options.ok_or_else(|| {
                    DecodeError::malformed(base_offset, MalformedKind::MissingRequiredStructure)
                })?,
                base_offset,
                limits,
            )?;
            let actual = instruction_sets.len().checked_add(1).ok_or_else(|| {
                DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
            })?;
            if actual as u64 > u64::from(limits.maximum_layers()) {
                return Err(DecodeError::limit(
                    Resource::Layers,
                    u64::from(limits.maximum_layers()),
                    actual as u64,
                ));
            }
            instruction_sets.push(instruction_set);
        } else {
            has_unknown_boxes = true;
        }
    }
    let (width, height, encoded_loop_count) = options.ok_or_else(|| {
        DecodeError::malformed(base_offset, MalformedKind::MissingRequiredStructure)
    })?;
    let loop_count = if encoded_loop_count == u8::MAX {
        0
    } else {
        encoded_loop_count
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?
    };
    Ok(Composition {
        width,
        height,
        loop_count,
        instruction_sets,
        has_unknown_boxes,
    })
}

fn validate_dimensions(
    width: u32,
    height: u32,
    limits: &ResourceLimits,
    offset: u64,
) -> Result<(), DecodeError> {
    if width == 0 || height == 0 {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidImageGeometry,
        ));
    }
    if width > limits.maximum_width() {
        return Err(DecodeError::limit(
            Resource::Width,
            u64::from(limits.maximum_width()),
            u64::from(width),
        ));
    }
    if height > limits.maximum_height() {
        return Err(DecodeError::limit(
            Resource::Height,
            u64::from(limits.maximum_height()),
            u64::from(height),
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    if pixels > limits.maximum_pixels() {
        return Err(DecodeError::limit(
            Resource::Pixels,
            limits.maximum_pixels(),
            pixels,
        ));
    }
    Ok(())
}

fn validate_layer_references(
    layers: &[CompositingLayer<'_>],
    codestream_count: usize,
) -> Result<(), DecodeError> {
    let mut layer_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant layer_index <= layers.len(),
            decreases layers.len() - layer_index,
        )
    )]
    while layer_index < layers.len() {
        let layer = layers
            .get(layer_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if layer.registrations.is_empty() {
            if layer_index >= codestream_count {
                return Err(DecodeError::malformed(
                    layer.header.offset(),
                    MalformedKind::InvalidBoxContents,
                ));
            }
        } else {
            let mut registration_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant registration_index <= layer.registrations.len(),
                    decreases layer.registrations.len() - registration_index,
                )
            )]
            while registration_index < layer.registrations.len() {
                let registration = layer
                    .registrations
                    .get(registration_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
                if usize::from(registration.codestream_index) >= codestream_count {
                    return Err(DecodeError::malformed(
                        layer.header.offset(),
                        MalformedKind::InvalidBoxContents,
                    ));
                }
                registration_index = registration_index
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            }
        }
        layer_index = layer_index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    Ok(())
}

fn parse_codestream_header(
    payload: &[u8],
    defaults: Option<&Jp2Header>,
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<Jp2Header, DecodeError> {
    if let Ok(header) = parse_jp2_header(payload, limits, base_offset) {
        return Ok(header);
    }
    let mut header = defaults.cloned().ok_or_else(|| {
        DecodeError::malformed(base_offset, MalformedKind::MissingRequiredStructure)
    })?;
    let mut reader = BoxReader::new(payload, limits)?;
    let mut image_override = false;
    let mut color_override = false;
    let mut saw_bpcc = false;
    let mut palette_override = false;
    let mut mapping_override = false;
    let mut definition_override = false;
    let mut resolution_override = false;
    let mut nested_box_count = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let box_ref = next.map_err(|error| rebase_error(error, base_offset))?;
        nested_box_count = nested_box_count.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
        let box_type = box_ref.header.box_type();
        if box_type.equals(BoxType::IMAGE_HEADER) {
            if image_override {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            header.image = parse_image_header(box_ref, limits, base_offset)?;
            header.palette = None;
            header.component_mappings.clear();
            header.channel_definition_records.clear();
            header.palette_entries = 0;
            header.palette_columns = 0;
            header.component_mapping_entries = 0;
            header.channel_definitions = 0;
            image_override = true;
        } else if box_type.equals(BoxType::BITS_PER_COMPONENT) {
            if saw_bpcc {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            validate_bpcc(box_ref, header.image, base_offset)?;
            saw_bpcc = true;
        } else if box_type.equals(BoxType::COLOR_SPECIFICATION) {
            let color = parse_color_specification(box_ref, base_offset)?;
            if crate::refinement::first_override_allowed(color_override) {
                header.color = color;
                color_override = true;
            }
        } else if box_type.equals(BoxType::PALETTE) {
            if palette_override || mapping_override || definition_override {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let palette = parse_palette(box_ref, base_offset)?;
            header.palette_entries = palette.entries;
            header.palette_columns = u8::try_from(palette.columns.len()).map_err(|_| {
                DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
            })?;
            header.palette = Some(palette);
            palette_override = true;
        } else if box_type.equals(BoxType::COMPONENT_MAPPING) {
            if mapping_override || definition_override {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let palette_columns = match &header.palette {
                Some(palette) => u8::try_from(palette.columns.len()).map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?,
                None => 0,
            };
            header.component_mappings = parse_component_mapping(
                box_ref,
                header.image.components,
                palette_columns,
                limits,
                base_offset,
            )?;
            header.component_mapping_entries = u16::try_from(header.component_mappings.len())
                .map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?;
            mapping_override = true;
        } else if box_type.equals(BoxType::CHANNEL_DEFINITION) {
            if definition_override {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let output_channels = if header.component_mappings.is_empty() {
                header.image.components
            } else {
                u16::try_from(header.component_mappings.len()).map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?
            };
            header.channel_definition_records =
                parse_channel_definition(box_ref, output_channels, base_offset)?;
            header.channel_definitions = u16::try_from(header.channel_definition_records.len())
                .map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?;
            definition_override = true;
        } else if box_type.equals(BoxType::RESOLUTION) {
            if resolution_override {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let (capture, display) = parse_resolution_superbox(
                box_ref.payload,
                limits,
                add_offset(base_offset, box_ref.header.payload_offset()?)?,
            )?;
            header.capture_resolution = capture;
            header.display_resolution = display;
            resolution_override = true;
        }
    }
    if image_override && matches!(header.image.bit_depth, ComponentBitDepth::Variable) != saw_bpcc {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    if header.palette.is_some() && header.component_mappings.is_empty() {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    header.nested_box_count = nested_box_count;
    Ok(header)
}

fn parse_jp2_header(
    payload: &[u8],
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<Jp2Header, DecodeError> {
    let mut reader = BoxReader::new(payload, limits)?;
    let image_box =
        required_box(&mut reader, 0).map_err(|error| rebase_error(error, base_offset))?;
    if !image_box.header.box_type().equals(BoxType::IMAGE_HEADER) {
        return Err(DecodeError::malformed(
            add_offset(base_offset, image_box.header.offset())?,
            MalformedKind::InvalidBoxOrder,
        ));
    }
    let image = parse_image_header(image_box, limits, base_offset)?;
    let mut color = None;
    let mut saw_bpcc = false;
    let mut palette = None;
    let mut component_mappings = Vec::new();
    let mut channel_definition_records = Vec::new();
    let mut capture_resolution = None;
    let mut display_resolution = None;
    let mut nested_box_count = 1_u32;

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let box_ref = next.map_err(|error| rebase_error(error, base_offset))?;
        nested_box_count = nested_box_count.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
        let box_type = box_ref.header.box_type();
        if box_type.equals(BoxType::IMAGE_HEADER) {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxOrder,
            ));
        }
        if box_type.equals(BoxType::BITS_PER_COMPONENT) {
            if saw_bpcc {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            validate_bpcc(box_ref, image, base_offset)?;
            saw_bpcc = true;
        } else if box_type.equals(BoxType::COLOR_SPECIFICATION) {
            let parsed_color = parse_color_specification(box_ref, base_offset)?;
            if color.is_none() {
                color = Some(parsed_color);
            }
        } else if box_type.equals(BoxType::PALETTE) {
            if palette.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            palette = Some(parse_palette(box_ref, base_offset)?);
        } else if box_type.equals(BoxType::COMPONENT_MAPPING) {
            if !component_mappings.is_empty() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let palette_columns = match &palette {
                Some(parsed) => u8::try_from(parsed.columns.len()).map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?,
                None => 0,
            };
            component_mappings = parse_component_mapping(
                box_ref,
                image.components,
                palette_columns,
                limits,
                base_offset,
            )?;
        } else if box_type.equals(BoxType::CHANNEL_DEFINITION) {
            if !channel_definition_records.is_empty() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            let output_channels = if component_mappings.is_empty() {
                image.components
            } else {
                u16::try_from(component_mappings.len()).map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?
            };
            channel_definition_records =
                parse_channel_definition(box_ref, output_channels, base_offset)?;
        } else if box_type.equals(BoxType::RESOLUTION) {
            let (capture, display) = parse_resolution_superbox(
                box_ref.payload,
                limits,
                add_offset(base_offset, box_ref.header.payload_offset()?)?,
            )?;
            if capture_resolution.is_some() || display_resolution.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            capture_resolution = capture;
            display_resolution = display;
        }
    }

    if matches!(image.bit_depth, ComponentBitDepth::Variable) != saw_bpcc {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    if palette.is_some() && component_mappings.is_empty() {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    let color = color.ok_or_else(|| {
        DecodeError::malformed(base_offset, MalformedKind::MissingRequiredStructure)
    })?;

    let (palette_entries, palette_columns) = match &palette {
        Some(parsed) => (parsed.entries(), parsed.columns.len() as u8),
        None => (0, 0),
    };
    let component_mapping_entries = u16::try_from(component_mappings.len())
        .map_err(|_| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    let channel_definitions = u16::try_from(channel_definition_records.len())
        .map_err(|_| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    Ok(Jp2Header {
        image,
        color,
        nested_box_count,
        palette_entries,
        palette_columns,
        component_mapping_entries,
        channel_definitions,
        palette,
        component_mappings,
        channel_definition_records,
        capture_resolution,
        display_resolution,
    })
}

fn parse_image_header(
    box_ref: BoxRef<'_>,
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<ImageHeader, DecodeError> {
    if box_ref.payload.len() != 14 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let height = read_u32([
        box_ref.payload[0],
        box_ref.payload[1],
        box_ref.payload[2],
        box_ref.payload[3],
    ]);
    let width = read_u32([
        box_ref.payload[4],
        box_ref.payload[5],
        box_ref.payload[6],
        box_ref.payload[7],
    ]);
    let components = read_u16([box_ref.payload[8], box_ref.payload[9]]);
    if width == 0 || height == 0 || components == 0 || box_ref.payload[11] != 7 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxContents,
        ));
    }
    if width > limits.maximum_width() {
        return Err(DecodeError::limit(
            Resource::Width,
            u64::from(limits.maximum_width()),
            u64::from(width),
        ));
    }
    if height > limits.maximum_height() {
        return Err(DecodeError::limit(
            Resource::Height,
            u64::from(limits.maximum_height()),
            u64::from(height),
        ));
    }
    if components > limits.maximum_components() {
        return Err(DecodeError::limit(
            Resource::Components,
            u64::from(limits.maximum_components()),
            u64::from(components),
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    if pixels > limits.maximum_pixels() {
        return Err(DecodeError::limit(
            Resource::Pixels,
            limits.maximum_pixels(),
            pixels,
        ));
    }
    let bit_depth = parse_bit_depth(box_ref.payload[10], base_offset)?;
    let unknown_color_space = match box_ref.payload[12] {
        0 => false,
        1 => true,
        _ => {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
    };
    let intellectual_property = match box_ref.payload[13] {
        0 => false,
        1 => true,
        _ => {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
    };
    Ok(ImageHeader {
        width,
        height,
        components,
        bit_depth,
        unknown_color_space,
        intellectual_property,
    })
}

fn parse_bit_depth(value: u8, offset: u64) -> Result<ComponentBitDepth, DecodeError> {
    if value == 0xff {
        return Ok(ComponentBitDepth::Variable);
    }
    let bits = (value & 0x7f)
        .checked_add(1)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    if bits > 38 {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxContents,
        ));
    }
    Ok(ComponentBitDepth::Uniform {
        bits,
        signed: value & 0x80 != 0,
    })
}

fn validate_bpcc(
    box_ref: BoxRef<'_>,
    image: ImageHeader,
    base_offset: u64,
) -> Result<(), DecodeError> {
    if box_ref.payload.len() != usize::from(image.components) {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= box_ref.payload.len(),
            decreases box_ref.payload.len() - index,
        )
    )]
    while index < box_ref.payload.len() {
        if matches!(
            parse_bit_depth(box_ref.payload[index], base_offset)?,
            ComponentBitDepth::Variable
        ) {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
        index = index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
    }
    Ok(())
}

fn parse_color_specification(
    box_ref: BoxRef<'_>,
    base_offset: u64,
) -> Result<ColorSpecification, DecodeError> {
    if box_ref.payload.len() < 3 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let profile_bytes = u64::try_from(box_ref.payload.len() - 3)
        .map_err(|_| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    let method_value = box_ref.payload[0];
    let mut icc_profile = None;
    let method = if method_value == 1 {
        if box_ref.payload.len() != 7 {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        ColorMethod::Enumerated {
            color_space: read_u32([
                box_ref.payload[3],
                box_ref.payload[4],
                box_ref.payload[5],
                box_ref.payload[6],
            ]),
        }
    } else if method_value == 2 {
        if profile_bytes == 0 {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        icc_profile = Some(copy_profile(&box_ref.payload[3..])?);
        ColorMethod::RestrictedIcc { profile_bytes }
    } else if method_value == 3 {
        if profile_bytes == 0 {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        icc_profile = Some(copy_profile(&box_ref.payload[3..])?);
        ColorMethod::AnyIcc { profile_bytes }
    } else {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidColorSpecification,
        ));
    };
    Ok(ColorSpecification {
        method,
        precedence: box_ref.payload[1],
        approximation: box_ref.payload[2],
        icc_profile,
    })
}

fn copy_profile(source: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut profile = Vec::with_capacity(source.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= source.len(),
            decreases source.len() - index,
        )
    )]
    while index < source.len() {
        profile.push(
            *source
                .get(index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?,
        );
        index = index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    Ok(profile)
}

fn parse_palette(box_ref: BoxRef<'_>, base_offset: u64) -> Result<Palette, DecodeError> {
    if box_ref.payload.len() < 3 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let entries = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let columns = box_ref.payload[2];
    if entries == 0 || columns == 0 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxContents,
        ));
    }
    let descriptor_end = 3_usize
        .checked_add(usize::from(columns))
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    if descriptor_end > box_ref.payload.len() {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let mut columns = Vec::with_capacity(usize::from(columns));
    let mut bytes_per_entry = 0_usize;
    let mut descriptor = 3_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant descriptor <= descriptor_end,
            decreases descriptor_end - descriptor,
        )
    )]
    while descriptor < descriptor_end {
        if descriptor >= box_ref.payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let bit_depth = parse_bit_depth(box_ref.payload[descriptor], base_offset)?;
        let (bits, signed) = match bit_depth {
            ComponentBitDepth::Uniform { bits, signed } => (bits, signed),
            ComponentBitDepth::Variable => {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxContents,
                ));
            }
        };
        let component_bytes = usize::from(bits).checked_add(7).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })? / 8;
        bytes_per_entry = bytes_per_entry
            .checked_add(component_bytes)
            .ok_or_else(|| {
                DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
            })?;
        columns.push(PaletteColumn {
            precision: bits,
            signed,
        });
        descriptor = descriptor.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
    }
    let sample_bytes = usize::from(entries)
        .checked_mul(bytes_per_entry)
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    let expected_length = descriptor_end
        .checked_add(sample_bytes)
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    if box_ref.payload.len() != expected_length {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let value_count = usize::from(entries)
        .checked_mul(columns.len())
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    let mut values = Vec::with_capacity(value_count);
    let mut offset = descriptor_end;
    let mut entry = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant entry <= entries,
            decreases entries - entry,
        )
    )]
    while entry < entries {
        let mut column = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant column <= columns.len(),
                decreases columns.len() - column,
            )
        )]
        while column < columns.len() {
            let descriptor = columns.get(column).ok_or_else(|| {
                DecodeError::malformed(base_offset, MalformedKind::InvalidBoxContents)
            })?;
            let byte_count = usize::from(descriptor.precision)
                .checked_add(7)
                .ok_or_else(|| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?
                / 8;
            let end = offset.checked_add(byte_count).ok_or_else(|| {
                DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
            })?;
            if end > box_ref.payload.len() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxLength,
                ));
            }
            values.push(parse_palette_value(
                &box_ref.payload[offset..end],
                descriptor.precision,
                descriptor.signed,
                base_offset,
            )?);
            offset = end;
            column = column.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
            })?;
        }
        entry = entry.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
    }
    if offset != box_ref.payload.len() || values.len() != value_count {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    Ok(Palette {
        entries,
        columns,
        values,
    })
}

fn parse_component_mapping(
    box_ref: BoxRef<'_>,
    components: u16,
    palette_columns: u8,
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<Vec<ComponentMapping>, DecodeError> {
    if box_ref.payload.is_empty() || !box_ref.payload.len().is_multiple_of(4) {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let entry_count = box_ref.payload.len() / 4;
    let entries = u16::try_from(entry_count)
        .map_err(|_| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    if entries > limits.maximum_components() {
        return Err(DecodeError::limit(
            Resource::Components,
            u64::from(limits.maximum_components()),
            u64::from(entries),
        ));
    }
    let mut mappings = Vec::with_capacity(entry_count);
    let mut offset = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant offset <= box_ref.payload.len(),
            decreases box_ref.payload.len() - offset,
        )
    )]
    while offset < box_ref.payload.len() {
        if box_ref.payload.len() - offset < 4 {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let component = read_u16([box_ref.payload[offset], box_ref.payload[offset + 1]]);
        let mapping_type = box_ref.payload[offset + 2];
        let palette_column = box_ref.payload[offset + 3];
        if !crate::refinement::component_mapping_valid(
            component,
            components,
            mapping_type,
            palette_column,
            palette_columns,
        ) {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
        let mapping_type = if mapping_type == 0 {
            MappingType::Direct
        } else {
            MappingType::Palette {
                column: palette_column,
            }
        };
        mappings.push(ComponentMapping {
            component,
            mapping_type,
        });
        offset = offset.checked_add(4).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
    }
    Ok(mappings)
}

fn parse_channel_definition(
    box_ref: BoxRef<'_>,
    output_channels: u16,
    base_offset: u64,
) -> Result<Vec<ChannelDefinitionRecord>, DecodeError> {
    if box_ref.payload.len() < 2 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let entries = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let entry_bytes = usize::from(entries)
        .checked_mul(6)
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    let expected_length = 2_usize
        .checked_add(entry_bytes)
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
    if entries == 0 || box_ref.payload.len() != expected_length {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let mut records = Vec::with_capacity(usize::from(entries));
    let mut offset = 2_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant offset <= box_ref.payload.len(),
            decreases box_ref.payload.len() - offset,
        )
    )]
    while offset < box_ref.payload.len() {
        if box_ref.payload.len() - offset < 6 {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let channel = read_u16([box_ref.payload[offset], box_ref.payload[offset + 1]]);
        let channel_type = read_u16([box_ref.payload[offset + 2], box_ref.payload[offset + 3]]);
        let association = read_u16([box_ref.payload[offset + 4], box_ref.payload[offset + 5]]);
        if !crate::refinement::channel_definition_valid(
            channel,
            channel_type,
            association,
            output_channels,
        ) {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
        let channel_type = match channel_type {
            0 => ChannelType::Color,
            1 => ChannelType::Opacity,
            2 => ChannelType::PremultipliedOpacity,
            0xffff => ChannelType::Unspecified,
            _ => unreachable!("channel type validated above"),
        };
        let association = if association == 0 {
            ChannelAssociation::WholeImage
        } else if association == 0xffff {
            ChannelAssociation::Unassociated
        } else {
            ChannelAssociation::Color(association)
        };
        records.push(ChannelDefinitionRecord {
            channel,
            definition: ChannelDefinition::new(channel_type, association),
        });
        offset = offset.checked_add(6).ok_or_else(|| {
            DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
        })?;
    }
    Ok(records)
}

fn parse_palette_value(
    bytes: &[u8],
    precision: u8,
    signed: bool,
    offset: u64,
) -> Result<i64, DecodeError> {
    if precision == 0 || precision > 38 || bytes.is_empty() || bytes.len() > 5 {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidBoxContents,
        ));
    }
    let mut raw = 0_u64;
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= bytes.len(),
            decreases bytes.len() - index,
        )
    )]
    while index < bytes.len() {
        raw = raw
            .checked_mul(256)
            .and_then(|value| value.checked_add(u64::from(bytes[index])))
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        index = index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    }
    let mut modulus = 1_u64;
    let mut bit = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= precision,
            decreases precision - bit,
        )
    )]
    while bit < precision {
        modulus = modulus
            .checked_mul(2)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        bit = bit
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    }
    let mask = modulus
        .checked_sub(1)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    let value = raw & mask;
    if signed && value & (modulus >> 1) != 0 {
        let magnitude =
            i64::try_from(modulus.checked_sub(value).ok_or_else(|| {
                DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow)
            })?)
            .map_err(|_| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
        0_i64
            .checked_sub(magnitude)
            .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))
    } else {
        i64::try_from(value)
            .map_err(|_| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))
    }
}

fn parse_resolution_superbox(
    payload: &[u8],
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<(Option<Resolution>, Option<Resolution>), DecodeError> {
    let mut reader = BoxReader::new(payload, limits)?;
    let mut capture = None;
    let mut display = None;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let box_ref = next.map_err(|error| rebase_error(error, base_offset))?;
        let box_type = box_ref.header.box_type();
        if box_type.equals(BoxType::CAPTURE_RESOLUTION) {
            if capture.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            capture = Some(parse_resolution(box_ref, base_offset)?);
        } else if box_type.equals(BoxType::DISPLAY_RESOLUTION) {
            if display.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            display = Some(parse_resolution(box_ref, base_offset)?);
        }
    }
    if capture.is_none() && display.is_none() {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    Ok((capture, display))
}

fn parse_resolution(box_ref: BoxRef<'_>, base_offset: u64) -> Result<Resolution, DecodeError> {
    if box_ref.payload.len() != 10 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let vertical_numerator = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let vertical_denominator = read_u16([box_ref.payload[2], box_ref.payload[3]]);
    let horizontal_numerator = read_u16([box_ref.payload[4], box_ref.payload[5]]);
    let horizontal_denominator = read_u16([box_ref.payload[6], box_ref.payload[7]]);
    if vertical_numerator == 0
        || vertical_denominator == 0
        || horizontal_numerator == 0
        || horizontal_denominator == 0
    {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxContents,
        ));
    }
    Ok(Resolution {
        vertical_numerator,
        vertical_denominator,
        horizontal_numerator,
        horizontal_denominator,
        vertical_exponent: box_ref.payload[8] as i8,
        horizontal_exponent: box_ref.payload[9] as i8,
    })
}

fn add_offset(base: u64, relative: u64) -> Result<u64, DecodeError> {
    base.checked_add(relative)
        .ok_or_else(|| DecodeError::malformed(base, MalformedKind::ArithmeticOverflow))
}

fn rebase_error(error: DecodeError, base_offset: u64) -> DecodeError {
    match error {
        DecodeError::MalformedInput { offset, kind } => match base_offset.checked_add(offset) {
            Some(rebased) => DecodeError::malformed(rebased, kind),
            None => DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow),
        },
        other => other,
    }
}

fn required_box<'a>(
    reader: &mut BoxReader<'a>,
    expected_index: u64,
) -> Result<BoxRef<'a>, DecodeError> {
    match reader.next_box_ref() {
        Some(result) => result,
        None => Err(DecodeError::malformed(
            expected_index,
            MalformedKind::MissingRequiredStructure,
        )),
    }
}

fn parse_file_type(box_ref: BoxRef<'_>, limits: &ResourceLimits) -> Result<FileType, DecodeError> {
    let payload_offset = box_ref.header.payload_offset()?;
    if box_ref.payload.len() < 8 || !box_ref.payload.len().is_multiple_of(4) {
        return Err(DecodeError::malformed(
            payload_offset,
            MalformedKind::InvalidBoxLength,
        ));
    }

    let compatible_brand_count = box_ref
        .payload
        .len()
        .checked_sub(8)
        .and_then(|bytes| bytes.checked_div(4))
        .ok_or_else(|| DecodeError::malformed(payload_offset, MalformedKind::ArithmeticOverflow))?;
    let compatible_brand_count_u64 = u64::try_from(compatible_brand_count)
        .map_err(|_| DecodeError::malformed(payload_offset, MalformedKind::ArithmeticOverflow))?;
    if compatible_brand_count_u64 > u64::from(limits.maximum_compatible_brands()) {
        return Err(DecodeError::limit(
            Resource::CompatibleBrands,
            u64::from(limits.maximum_compatible_brands()),
            compatible_brand_count_u64,
        ));
    }
    let brand = BoxType::new([
        box_ref.payload[0],
        box_ref.payload[1],
        box_ref.payload[2],
        box_ref.payload[3],
    ]);
    let mut compatible_brands: Vec<BoxType> = Vec::new();
    let mut position = 8_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= box_ref.payload.len(),
            decreases box_ref.payload.len() - position,
        )
    )]
    while position < box_ref.payload.len() {
        let compatible = read_box_type_at(box_ref.payload, position, payload_offset)?;
        let mut prior = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant prior <= compatible_brands.len(),
                decreases compatible_brands.len() - prior,
            )
        )]
        while prior < compatible_brands.len() {
            let previous = compatible_brands.get(prior).ok_or_else(|| {
                DecodeError::malformed(box_ref.header.offset(), MalformedKind::ArithmeticOverflow)
            })?;
            if previous.equals(compatible) {
                return Err(DecodeError::malformed(
                    payload_offset,
                    MalformedKind::InvalidBoxContents,
                ));
            }
            prior = prior.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(box_ref.header.offset(), MalformedKind::ArithmeticOverflow)
            })?;
        }
        compatible_brands.push(compatible);
        position = position.checked_add(4).ok_or_else(|| {
            DecodeError::malformed(box_ref.header.offset(), MalformedKind::ArithmeticOverflow)
        })?;
        if position > box_ref.payload.len() {
            return Err(DecodeError::malformed(
                payload_offset,
                MalformedKind::InvalidBoxLength,
            ));
        }
    }
    Ok(FileType {
        brand,
        minor_version: read_u32([
            box_ref.payload[4],
            box_ref.payload[5],
            box_ref.payload[6],
            box_ref.payload[7],
        ]),
        compatible_brands,
    })
}

fn validate_file_type_profile(file_type: &FileType, offset: u64) -> Result<(), DecodeError> {
    if file_type.brand.equals(BoxType::JP2_BRAND) && file_type.is_jpx_compatible() {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidFileType,
        ));
    }
    if !file_type.is_jph_compatible() {
        return Ok(());
    }
    if !crate::refinement::part15_jph_branding(
        file_type.brand.equals(BoxType::JPH_BRAND),
        file_type.minor_version,
        file_type.compatible_brands.len(),
        file_type.compatible_brands.len() == 1
            && file_type.compatible_brands[0].equals(BoxType::JPH_BRAND),
    ) {
        return Err(DecodeError::malformed(
            offset,
            MalformedKind::InvalidProfile,
        ));
    }
    Ok(())
}

fn read_box_type_at(payload: &[u8], position: usize, offset: u64) -> Result<BoxType, DecodeError> {
    let position1 = position
        .checked_add(1)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    let position2 = position
        .checked_add(2)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    let position3 = position
        .checked_add(3)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::ArithmeticOverflow))?;
    let byte0 = *payload
        .get(position)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::InvalidBoxLength))?;
    let byte1 = *payload
        .get(position1)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::InvalidBoxLength))?;
    let byte2 = *payload
        .get(position2)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::InvalidBoxLength))?;
    let byte3 = *payload
        .get(position3)
        .ok_or_else(|| DecodeError::malformed(offset, MalformedKind::InvalidBoxLength))?;
    Ok(BoxType::new([byte0, byte1, byte2, byte3]))
}

fn read_u32(bytes: [u8; 4]) -> u32 {
    (u32::from(bytes[0]) << 24)
        | (u32::from(bytes[1]) << 16)
        | (u32::from(bytes[2]) << 8)
        | u32::from(bytes[3])
}

fn read_u16(bytes: [u8; 2]) -> u16 {
    (u16::from(bytes[0]) << 8) | u16::from(bytes[1])
}

fn read_u64(bytes: [u8; 8]) -> u64 {
    (u64::from(bytes[0]) << 56)
        | (u64::from(bytes[1]) << 48)
        | (u64::from(bytes[2]) << 40)
        | (u64::from(bytes[3]) << 32)
        | (u64::from(bytes[4]) << 24)
        | (u64::from(bytes[5]) << 16)
        | (u64::from(bytes[6]) << 8)
        | u64::from(bytes[7])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box32(box_type: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let length = u32::try_from(8 + payload.len()).unwrap();
        let mut bytes = length.to_be_bytes().to_vec();
        bytes.extend_from_slice(&box_type);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn marker_segment(marker: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = marker.to_be_bytes().to_vec();
        bytes.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn valid_codestream(width: u32, height: u32) -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0_u16.to_be_bytes());
        siz.extend_from_slice(&width.to_be_bytes());
        siz.extend_from_slice(&height.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&width.to_be_bytes());
        siz.extend_from_slice(&height.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1]);

        let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
        bytes.extend(marker_segment(0xff51, &siz));
        bytes.extend(marker_segment(0xff52, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]));
        bytes.extend(marker_segment(0xff5c, &[0, 0]));
        let tile_part_start = bytes.len();
        let mut sot = Vec::new();
        sot.extend_from_slice(&0_u16.to_be_bytes());
        sot.extend_from_slice(&14_u32.to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        bytes.extend(marker_segment(0xff90, &sot));
        bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
        assert_eq!(bytes.len() - tile_part_start, 14);
        bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
        bytes
    }

    fn valid_ht_codestream(width: u32, height: u32, style: u8) -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0x4000_u16.to_be_bytes());
        siz.extend_from_slice(&width.to_be_bytes());
        siz.extend_from_slice(&height.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&width.to_be_bytes());
        siz.extend_from_slice(&height.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1]);

        let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
        bytes.extend(marker_segment(0xff51, &siz));
        bytes.extend(marker_segment(0xff50, &[0, 2, 0, 0, 0, 8]));
        bytes.extend(marker_segment(0xff52, &[0, 0, 0, 1, 0, 0, 2, 2, style, 1]));
        bytes.extend(marker_segment(0xff5c, &[0, 0x40]));
        let tile_part_start = bytes.len();
        let mut sot = Vec::new();
        sot.extend_from_slice(&0_u16.to_be_bytes());
        sot.extend_from_slice(&14_u32.to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        bytes.extend(marker_segment(0xff90, &sot));
        bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
        assert_eq!(bytes.len() - tile_part_start, 14);
        bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
        bytes
    }

    fn boxed_file(file_type: &[u8], codestream: &[u8]) -> Vec<u8> {
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", file_type));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&32_u32.to_be_bytes());
        image_header.extend_from_slice(&64_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut jp2_header = box32(*b"ihdr", &image_header);
        jp2_header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        bytes.extend(box32(*b"jp2h", &jp2_header));
        bytes.extend(box32(*b"jp2c", codestream));
        bytes
    }

    #[test]
    fn routes_canonical_jph_inspection_past_ht_decoder_construction() {
        let bytes = boxed_file(b"jph \0\0\0\0jph ", &valid_ht_codestream(64, 32, 0x40));
        let inspected = inspect_jph(&bytes, &ResourceLimits::default()).unwrap();
        assert!(inspected.file().file_type().is_jph_compatible());
        assert!(
            inspected
                .codestream(0)
                .unwrap()
                .profile
                .uses_high_throughput()
        );
        let decoded = crate::decode::decode_jph(&bytes, &crate::DecodeOptions::default());
        assert!(matches!(
            decoded,
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::Truncated,
                ..
            })
        ));
    }

    #[test]
    fn accepts_canonical_jph_with_all_legacy_htdeclared_codestream() {
        let mut codestream = valid_ht_codestream(64, 32, 0x00);
        let capability = codestream
            .windows(6)
            .position(|window| window == [0x00, 0x02, 0x00, 0x00, 0x00, 0x08])
            .unwrap();
        codestream[capability + 4..capability + 6].copy_from_slice(&0x8008_u16.to_be_bytes());
        let bytes = boxed_file(b"jph \0\0\0\0jph ", &codestream);
        let inspected = inspect_jph(&bytes, &ResourceLimits::default()).unwrap();
        let profile = &inspected.codestream(0).unwrap().profile;
        assert!(profile.is_part15());
        assert!(!profile.uses_high_throughput());
    }

    #[test]
    fn rejects_jph_brand_and_codestream_profile_inconsistencies() {
        for file_type in [
            &b"jph \0\0\0\0jp2 "[..],
            &b"jp2 \0\0\0\0jph "[..],
            &b"jph \0\0\0\x01jph "[..],
        ] {
            let bytes = boxed_file(file_type, &valid_ht_codestream(64, 32, 0x40));
            assert!(matches!(
                parse(&bytes, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidProfile,
                    ..
                })
            ));
        }

        let branded_jph = boxed_file(b"jph \0\0\0\0jph ", &valid_codestream(64, 32));
        assert!(matches!(
            inspect_jph(&branded_jph, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProfile,
                ..
            })
        ));
        let mislabeled_jp2 = boxed_file(b"jp2 \0\0\0\0jp2 ", &valid_ht_codestream(64, 32, 0xc0));
        assert!(matches!(
            parse_and_inspect(&mislabeled_jp2, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProfile,
                ..
            })
        ));
    }

    fn minimal_jp2() -> Vec<u8> {
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&32_u32.to_be_bytes());
        image_header.extend_from_slice(&64_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut jp2_header = box32(*b"ihdr", &image_header);
        jp2_header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        bytes.extend(box32(*b"jp2h", &jp2_header));
        bytes.extend(box32(*b"jp2c", &[0xff, 0x4f, 0xff, 0xd9]));
        bytes
    }

    #[test]
    fn icc_color_specification_preserves_bounded_profile_bytes() {
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut jp2_header = box32(*b"ihdr", &image_header);
        jp2_header.extend(box32(*b"colr", &[2, 0, 0, 0xde, 0xad, 0xbe, 0xef]));
        bytes.extend(box32(*b"jp2h", &jp2_header));
        bytes.extend(box32(*b"jp2c", &[0xff, 0x4f, 0xff, 0xd9]));

        let parsed = parse(&bytes, &ResourceLimits::default()).unwrap();
        assert!(matches!(
            parsed.header().color.method,
            ColorMethod::RestrictedIcc { profile_bytes: 4 }
        ));
        assert_eq!(
            parsed.header().color.icc_profile(),
            Some(&[0xde, 0xad, 0xbe, 0xef][..])
        );
        let component = crate::image::TypedComponent::from_parts(
            1,
            1,
            crate::codestream::ComponentInfo {
                precision: 8,
                signed: false,
                x_subsampling: 1,
                y_subsampling: 1,
                x_registration: 0,
                y_registration: 0,
            },
            crate::image::TypedSamples::Unsigned(vec![0]),
        );
        let image = crate::image::TypedImage::from_components(
            1,
            1,
            vec![component],
            &ResourceLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            crate::color::convert_jp2_color(
                &image,
                &parsed.header().color,
                &ResourceLimits::default()
            ),
            Err(DecodeError::UnsupportedColorConversion {
                reason: crate::UnsupportedColorConversion::IccProfile
            })
        ));
    }

    fn image_header_box(width: u32, height: u32) -> Vec<u8> {
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&height.to_be_bytes());
        image_header.extend_from_slice(&width.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        header
    }

    fn jpx_with_codestreams(
        file_type_payload: &[u8],
        codestreams: &[Vec<u8>],
        codestream_headers: &[Vec<u8>],
    ) -> Vec<u8> {
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", file_type_payload));
        bytes.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        for header in codestream_headers {
            bytes.extend(box32(*b"jpch", header));
        }
        for codestream in codestreams {
            bytes.extend(box32(*b"jp2c", codestream));
        }
        bytes
    }

    #[test]
    fn parses_minimal_file() {
        let bytes = minimal_jp2();
        let file = parse(&bytes, &ResourceLimits::default()).unwrap();
        assert_eq!(file.file_type.brand, BoxType::JP2_BRAND);
        assert_eq!(file.header.image.width, 64);
        assert_eq!(file.header.image.height, 32);
        assert_eq!(file.header.image.components, 1);
        assert_eq!(
            file.header.image.bit_depth,
            ComponentBitDepth::Uniform {
                bits: 8,
                signed: false
            }
        );
        assert_eq!(
            file.header.color.method,
            ColorMethod::Enumerated { color_space: 16 }
        );
        assert_eq!(file.codestream(0).unwrap().data, [0xff, 0x4f, 0xff, 0xd9]);
        assert_eq!(file.box_count, 4);
    }

    #[test]
    fn parses_extended_and_to_eof_boxes() {
        let mut extended = 1_u32.to_be_bytes().to_vec();
        extended.extend_from_slice(b"uuid");
        extended.extend_from_slice(&20_u64.to_be_bytes());
        extended.extend_from_slice(b"data");

        let reader = BoxReader::new(&extended, &ResourceLimits::default()).unwrap();
        let boxes = reader.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(boxes[0].header.header_length(), 16);
        assert_eq!(boxes[0].payload, b"data");

        let mut to_eof = 0_u32.to_be_bytes().to_vec();
        to_eof.extend_from_slice(b"xml ");
        to_eof.extend_from_slice(b"content");
        let boxes = BoxReader::new(&to_eof, &ResourceLimits::default())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(boxes[0].header.total_length(), 15);
        assert_eq!(boxes[0].payload, b"content");
    }

    #[test]
    fn rejects_invalid_length_and_order() {
        let mut invalid = 7_u32.to_be_bytes().to_vec();
        invalid.extend_from_slice(b"free");
        assert!(matches!(
            BoxReader::new(&invalid, &ResourceLimits::default())
                .unwrap()
                .next()
                .unwrap(),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxLength,
                ..
            })
        ));

        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"jp2h", &[]));
        assert!(matches!(
            parse(&bytes, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxOrder,
                ..
            })
        ));
    }

    #[test]
    fn every_prefix_fails_without_panicking() {
        let bytes = minimal_jp2();
        for length in 0..bytes.len() {
            assert!(parse(&bytes[..length], &ResourceLimits::default()).is_err());
        }
    }

    #[test]
    fn enforces_metadata_limit() {
        let bytes = minimal_jp2();
        let limits = ResourceLimits::default()
            .with_maximum_metadata_bytes(31)
            .unwrap();
        assert!(matches!(
            parse(&bytes, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::MetadataBytes,
                ..
            })
        ));
    }

    #[test]
    fn requires_bpcc_for_variable_component_depths() {
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&2_u16.to_be_bytes());
        image_header.extend_from_slice(&[0xff, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        assert!(matches!(
            parse_jp2_header(&header, &ResourceLimits::default(), 0),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::MissingRequiredStructure,
                ..
            })
        ));

        let mut with_bpcc = box32(*b"ihdr", &image_header);
        with_bpcc.extend(box32(*b"bpcc", &[7, 0x87]));
        with_bpcc.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        let parsed = parse_jp2_header(&with_bpcc, &ResourceLimits::default(), 0).unwrap();
        assert_eq!(parsed.image.components, 2);
    }

    #[test]
    fn validates_palette_mapping_relationships() {
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        header.extend(box32(*b"pclr", &[0, 1, 1, 7, 42]));
        assert!(matches!(
            parse_jp2_header(&header, &ResourceLimits::default(), 0),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::MissingRequiredStructure,
                ..
            })
        ));

        header.extend(box32(*b"cmap", &[0, 0, 1, 0]));
        let parsed = parse_jp2_header(&header, &ResourceLimits::default(), 0).unwrap();
        assert_eq!(parsed.palette_entries, 1);
        assert_eq!(parsed.palette_columns, 1);
        assert_eq!(parsed.component_mapping_entries, 1);
        let palette = parsed.palette.as_ref().unwrap();
        assert_eq!(palette.columns()[0].precision, 8);
        assert!(!palette.columns()[0].signed);
        assert_eq!(palette.value(0, 0).unwrap(), 42);
        assert_eq!(
            parsed.component_mappings[0],
            ComponentMapping {
                component: 0,
                mapping_type: MappingType::Palette { column: 0 },
            }
        );
    }

    #[test]
    fn retains_signed_palette_and_channel_definition_records() {
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        header.extend(box32(*b"pclr", &[0, 1, 1, 0x84, 0x1d]));
        header.extend(box32(*b"cmap", &[0, 0, 1, 0]));
        header.extend(box32(
            *b"cdef",
            &[0, 2, 0, 0, 0, 0, 0, 1, 0, 0, 0xff, 0xff, 0xff, 0xff],
        ));
        let parsed = parse_jp2_header(&header, &ResourceLimits::default(), 0).unwrap();
        let palette = parsed.palette.as_ref().unwrap();
        assert_eq!(
            palette.columns()[0],
            PaletteColumn {
                precision: 5,
                signed: true,
            }
        );
        assert_eq!(palette.value(0, 0).unwrap(), -3);
        assert_eq!(parsed.channel_definition_records.len(), 2);
        assert_eq!(
            parsed.channel_definition_records[0]
                .definition
                .channel_type(),
            ChannelType::Color
        );
        assert_eq!(
            parsed.channel_definition_records[1]
                .definition
                .association(),
            ChannelAssociation::Unassociated
        );
    }

    #[test]
    fn rejects_invalid_mapping_type_and_bounds_output_channels() {
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        header.extend(box32(*b"cmap", &[0, 0, 2, 0]));
        assert!(matches!(
            parse_jp2_header(&header, &ResourceLimits::default(), 0),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));

        let mut mappings = Vec::new();
        for _ in 0..65 {
            mappings.extend_from_slice(&[0, 0, 0, 0]);
        }
        let mut bounded = box32(*b"ihdr", &image_header);
        bounded.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        bounded.extend(box32(*b"cmap", &mappings));
        assert!(matches!(
            parse_jp2_header(&bounded, &ResourceLimits::default(), 0),
            Err(DecodeError::LimitExceeded {
                resource: Resource::Components,
                limit: 64,
                actual: 65,
            })
        ));
    }

    #[test]
    fn cross_checks_container_and_codestream_headers() {
        let codestream = valid_codestream(64, 32);
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&32_u32.to_be_bytes());
        image_header.extend_from_slice(&64_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut jp2_header = box32(*b"ihdr", &image_header);
        jp2_header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        bytes.extend(box32(*b"jp2h", &jp2_header));
        bytes.extend(box32(*b"jp2c", &codestream));
        let inspected = parse_and_inspect(&bytes, &ResourceLimits::default()).unwrap();
        assert_eq!(inspected.codestream(0).unwrap().image_size.width, 64);

        let height_offset = 12 + 20 + 8 + 8;
        bytes[height_offset..height_offset + 4].copy_from_slice(&31_u32.to_be_bytes());
        assert!(matches!(
            parse_and_inspect(&bytes, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::ContainerCodestreamMismatch,
                ..
            })
        ));
    }

    #[test]
    fn retains_and_inspects_explicit_jpx_codestreams_and_headers() {
        let first = valid_codestream(64, 32);
        let second = valid_codestream(17, 9);
        let bytes = jpx_with_codestreams(
            b"vend\0\0\0\0jpxb",
            &[first.clone(), second.clone()],
            &[image_header_box(64, 32), image_header_box(17, 9)],
        );
        let inspected = parse_and_inspect(&bytes, &ResourceLimits::default()).unwrap();
        assert!(inspected.file.file_type.is_jpx_compatible());
        assert_eq!(
            inspected.file.file_type.compatible_brands(),
            &[BoxType::JPX_BASELINE_BRAND]
        );
        assert_eq!(inspected.codestream_count(), 2);
        assert_eq!(inspected.codestream(0).unwrap().image_size.width, 64);
        assert_eq!(inspected.codestream(1).unwrap().image_size.height, 9);
        assert_eq!(inspected.file.codestream(0).unwrap().data, first);
        assert_eq!(inspected.file.codestream(1).unwrap().data, second);
        assert!(
            inspected
                .file
                .codestream(1)
                .unwrap()
                .codestream_header()
                .unwrap()
                .parsed_header()
                .is_some()
        );
        assert!(matches!(
            inspected.file.codestream(2),
            Err(DecodeError::CodestreamIndexOutOfRange {
                index: 2,
                codestreams: 2,
            })
        ));

        let mismatched = jpx_with_codestreams(
            b"jpx \0\0\0\0jpx ",
            &[valid_codestream(64, 32), valid_codestream(17, 9)],
            &[image_header_box(64, 32), image_header_box(18, 9)],
        );
        assert!(matches!(
            parse_and_inspect(&mismatched, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::ContainerCodestreamMismatch,
                ..
            })
        ));
    }

    #[test]
    fn bounds_jpx_codestream_count_and_rejects_jp2_duplicates() {
        let codestream = valid_codestream(64, 32);
        let bytes = jpx_with_codestreams(
            b"jpx \0\0\0\0jpx ",
            &[codestream.clone(), codestream.clone()],
            &[],
        );
        let limits = ResourceLimits::default()
            .with_maximum_codestreams(1)
            .unwrap();
        assert!(matches!(
            parse(&bytes, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::Codestreams,
                limit: 1,
                actual: 2,
            })
        ));

        let jp2 = jpx_with_codestreams(b"jp2 \0\0\0\0jp2 ", &[codestream.clone(), codestream], &[]);
        assert!(matches!(
            parse(&jp2, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));
    }

    #[test]
    fn rejects_duplicate_compatible_brands_and_late_codestream_headers() {
        let duplicate =
            jpx_with_codestreams(b"jpx \0\0\0\0jpxbjpxb", &[valid_codestream(64, 32)], &[]);
        assert!(matches!(
            parse(&duplicate, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));

        let brand_bounded =
            jpx_with_codestreams(b"jpx \0\0\0\0jp2 jpxb", &[valid_codestream(64, 32)], &[]);
        let limits = ResourceLimits::default()
            .with_maximum_compatible_brands(1)
            .unwrap();
        assert!(matches!(
            parse(&brand_bounded, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::CompatibleBrands,
                limit: 1,
                actual: 2,
            })
        ));

        let mut late = jpx_with_codestreams(b"jpx \0\0\0\0jpx ", &[valid_codestream(64, 32)], &[]);
        late.extend(box32(*b"jpch", &image_header_box(64, 32)));
        assert!(matches!(
            parse(&late, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxOrder,
                ..
            })
        ));
    }

    #[test]
    fn rejects_contradictory_jp2_primary_and_jpx_compatibility() {
        let contradictory = boxed_file(b"jp2 \0\0\0\0jp2 jpx ", &valid_codestream(64, 32));
        assert!(matches!(
            parse(&contradictory, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidFileType,
                ..
            })
        ));
    }

    #[test]
    fn retains_validated_layer_registration_and_static_composition_options() {
        let codestream = valid_codestream(64, 32);
        let mut layered = box32(*b"jP  ", &SIGNATURE_MAGIC);
        layered.extend(box32(*b"ftyp", b"jpx \0\0\0\0jpx "));
        layered.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        let mut registration = 10_u16.to_be_bytes().to_vec();
        registration.extend_from_slice(&10_u16.to_be_bytes());
        registration.extend_from_slice(&0_u16.to_be_bytes());
        registration.extend_from_slice(&[10, 10, 7, 9]);
        layered.extend(box32(*b"jplh", &box32(*b"creg", &registration)));
        let mut options = 32_u32.to_be_bytes().to_vec();
        options.extend_from_slice(&64_u32.to_be_bytes());
        options.push(0);
        layered.extend(box32(*b"comp", &box32(*b"copt", &options)));
        layered.extend(box32(*b"jp2c", &codestream));
        let parsed = parse(&layered, &ResourceLimits::default()).unwrap();
        let layer = parsed.compositing_layer(0).unwrap();
        assert_eq!(layer.horizontal_grid_size(), 10);
        assert_eq!(
            layer.registrations()[0],
            CodestreamRegistration {
                codestream_index: 0,
                horizontal_subsampling: 10,
                vertical_subsampling: 10,
                horizontal_offset: 7,
                vertical_offset: 9,
            }
        );
        assert_eq!(parsed.composition().unwrap().width, 64);
        assert_eq!(parsed.composition().unwrap().height, 32);

        let mut invalid_reference = layered.clone();
        let reference = invalid_reference
            .windows(4)
            .position(|window| window == *b"creg")
            .unwrap()
            + 8;
        invalid_reference[reference..reference + 2].copy_from_slice(&1_u16.to_be_bytes());
        assert!(matches!(
            parse(&invalid_reference, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));
    }

    #[test]
    fn parses_layer_color_opacity_and_instruction_fields() {
        let codestream = valid_codestream(64, 32);
        let mut layered = box32(*b"jP  ", &SIGNATURE_MAGIC);
        layered.extend(box32(*b"ftyp", b"jpx \0\0\0\0jpx "));
        layered.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        let color_group = box32(*b"cgrp", &box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        let mut layer_header = color_group;
        layer_header.extend(box32(*b"opct", &[2, 1, 7]));
        layered.extend(box32(*b"jplh", &layer_header));
        let mut options = 32_u32.to_be_bytes().to_vec();
        options.extend_from_slice(&64_u32.to_be_bytes());
        options.push(1);
        let mut instruction = 0x27_u16.to_be_bytes().to_vec();
        instruction.extend_from_slice(&1_u16.to_be_bytes());
        instruction.extend_from_slice(&10_u32.to_be_bytes());
        for value in [1_u32, 2, 3, 4, 0x8000_0005, 1, 6, 7, 8, 9] {
            instruction.extend_from_slice(&value.to_be_bytes());
        }
        let mut composition = box32(*b"copt", &options);
        composition.extend(box32(*b"inst", &instruction));
        layered.extend(box32(*b"comp", &composition));
        layered.extend(box32(*b"jp2c", &codestream));
        let parsed = parse(&layered, &ResourceLimits::default()).unwrap();
        let layer = parsed.compositing_layer(0).unwrap();
        assert!(layer.has_color_group());
        assert!(matches!(
            layer.opacity(),
            Some(LayerOpacity::ChromaKey {
                channels: 1,
                values,
            }) if values == &[7]
        ));
        let composition = parsed.composition().unwrap();
        assert_eq!(composition.loop_count, 2);
        assert_eq!(composition.instruction_sets.len(), 1);
        let parsed_instruction = composition.instruction_sets[0].instructions[0];
        assert_eq!(parsed_instruction.target_x, 1);
        assert_eq!(parsed_instruction.life, 50);
        assert!(parsed_instruction.persistent);
        assert_eq!(parsed_instruction.source_width, 8);
    }

    #[test]
    fn rejects_reserved_opacity_and_instruction_flags() {
        let codestream = valid_codestream(64, 32);
        let mut invalid_opacity = box32(*b"jP  ", &SIGNATURE_MAGIC);
        invalid_opacity.extend(box32(*b"ftyp", b"jpx \0\0\0\0jpx "));
        invalid_opacity.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        invalid_opacity.extend(box32(*b"jplh", &box32(*b"opct", &[3])));
        invalid_opacity.extend(box32(*b"jp2c", &codestream));
        assert!(matches!(
            parse(&invalid_opacity, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));

        let mut invalid_instruction = box32(*b"jP  ", &SIGNATURE_MAGIC);
        invalid_instruction.extend(box32(*b"ftyp", b"jpx \0\0\0\0jpx "));
        invalid_instruction.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        invalid_instruction.extend(box32(*b"jplh", &[]));
        let mut options = 32_u32.to_be_bytes().to_vec();
        options.extend_from_slice(&64_u32.to_be_bytes());
        options.push(0);
        let mut reserved = 8_u16.to_be_bytes().to_vec();
        reserved.extend_from_slice(&0_u16.to_be_bytes());
        reserved.extend_from_slice(&1_u32.to_be_bytes());
        let mut composition = box32(*b"copt", &options);
        composition.extend(box32(*b"inst", &reserved));
        invalid_instruction.extend(box32(*b"comp", &composition));
        invalid_instruction.extend(box32(*b"jp2c", &codestream));
        assert!(matches!(
            parse(&invalid_instruction, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents,
                ..
            })
        ));
    }

    #[test]
    fn inherits_partial_jpx_codestream_header_color_metadata() {
        let codestream = valid_codestream(64, 32);
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", b"jpx \0\0\0\0jpx "));
        bytes.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        let full_header = image_header_box(64, 32);
        let first_box_length = u32::from_be_bytes([
            full_header[0],
            full_header[1],
            full_header[2],
            full_header[3],
        ]) as usize;
        bytes.extend(box32(*b"jpch", &full_header[..first_box_length]));
        bytes.extend(box32(*b"jp2c", &codestream));
        let parsed = parse_and_inspect(&bytes, &ResourceLimits::default()).unwrap();
        let inherited = parsed
            .file()
            .codestream(0)
            .unwrap()
            .codestream_header()
            .unwrap()
            .parsed_header()
            .unwrap();
        assert!(matches!(
            inherited.color.method(),
            ColorMethod::Enumerated {
                color_space: ENUMERATED_SRGB
            }
        ));
    }

    #[test]
    fn applies_palette_mapping_and_channel_overrides_in_partial_jpch() {
        let codestream = valid_codestream(64, 32);
        let mut override_boxes = box32(*b"pclr", &[0, 2, 1, 7, 3, 11]);
        override_boxes.extend(box32(*b"cmap", &[0, 0, 1, 0]));
        override_boxes.extend(box32(*b"cdef", &[0, 1, 0, 0, 0, 0, 0, 1]));
        let bytes = jpx_with_codestreams(b"jpx \0\0\0\0jpx ", &[codestream], &[override_boxes]);
        let parsed = parse(&bytes, &ResourceLimits::default()).unwrap();
        let header = parsed
            .codestream(0)
            .unwrap()
            .codestream_header()
            .unwrap()
            .parsed_header()
            .unwrap();
        assert_eq!(header.palette_entries, 2);
        assert_eq!(header.palette.as_ref().unwrap().value(1, 0).unwrap(), 11);
        assert_eq!(header.component_mapping_entries, 1);
        assert_eq!(header.channel_definitions, 1);
    }

    #[test]
    fn retains_general_layer_cdef_and_nested_metadata() {
        let codestream = valid_codestream(64, 32);
        let mut bytes = box32(*b"jP  ", &SIGNATURE_MAGIC);
        bytes.extend(box32(*b"ftyp", b"jpx \0\0\0\0jpx "));
        bytes.extend(box32(*b"jp2h", &image_header_box(64, 32)));
        let cdef = box32(*b"cdef", &[0, 2, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0]);
        let nested = box32(*b"asoc", &box32(*b"lbl ", b"layer"));
        let mut layer = cdef;
        layer.extend(nested);
        bytes.extend(box32(*b"jplh", &layer));
        bytes.extend(box32(*b"asoc", &box32(*b"asoc", &box32(*b"xml ", b"<x/>"))));
        bytes.extend(box32(*b"jp2c", &codestream));
        let parsed = parse(&bytes, &ResourceLimits::default()).unwrap();
        let layer = parsed.compositing_layer(0).unwrap();
        assert_eq!(layer.channel_definition_records().len(), 2);
        assert_eq!(layer.nested_metadata_boxes(), 2);
        assert_eq!(parsed.metadata_boxes(), 3);
        assert_eq!(parsed.metadata_depth(), 3);
    }
}
