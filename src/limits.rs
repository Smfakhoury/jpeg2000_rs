use crate::{DecodeError, Resource};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ResourceLimits {
    maximum_input_bytes: u64,
    maximum_metadata_bytes: u64,
    maximum_box_depth: u32,
    maximum_compatible_brands: u32,
    maximum_codestreams: u32,
    maximum_width: u32,
    maximum_height: u32,
    maximum_pixels: u64,
    maximum_components: u16,
    maximum_tiles: u32,
    maximum_tile_parts: u32,
    maximum_resolutions_per_component: u8,
    maximum_precincts: u64,
    maximum_code_blocks: u64,
    maximum_code_block_passes: u32,
    maximum_code_block_bytes: u64,
    maximum_layers: u16,
    maximum_frames: u32,
    maximum_packets: u64,
    maximum_packet_header_bytes: u64,
    maximum_decoded_bytes: u64,
    maximum_jpip_fragments: u64,
    maximum_tag_tree_nodes: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn resource_input_byte_limit(limits: &ResourceLimits) -> u64 {
        limits.maximum_input_bytes
    }

    pub closed spec fn resource_work_limit(limits: &ResourceLimits) -> u64 {
        limits.maximum_work_units
    }

    pub closed spec fn resource_code_block_byte_limit(limits: &ResourceLimits) -> u64 {
        limits.maximum_code_block_bytes
    }

    pub closed spec fn resource_code_block_pass_limit(limits: &ResourceLimits) -> u32 {
        limits.maximum_code_block_passes
    }

    pub closed spec fn resource_decoded_bytes_limit(limits: &ResourceLimits) -> u64 {
        limits.maximum_decoded_bytes
    }

    pub closed spec fn resource_component_limit(limits: &ResourceLimits) -> u16 {
        limits.maximum_components
    }
}

impl ResourceLimits {
    pub const HARDENED: Self = Self {
        maximum_input_bytes: 2_147_483_648,
        maximum_metadata_bytes: 16_777_216,
        maximum_box_depth: 32,
        maximum_compatible_brands: 64,
        maximum_codestreams: 64,
        maximum_width: 1_000_000,
        maximum_height: 1_000_000,
        maximum_pixels: 100_000_000,
        maximum_components: 64,
        maximum_tiles: 16_384,
        maximum_tile_parts: 16_384,
        maximum_resolutions_per_component: 33,
        maximum_precincts: 16_777_216,
        maximum_code_blocks: 16_777_216,
        maximum_code_block_passes: crate::tier1::MAX_TIER1_PASSES,
        maximum_code_block_bytes: 67_580,
        maximum_layers: 256,
        maximum_frames: 4_096,
        maximum_packets: 67_108_864,
        maximum_packet_header_bytes: 268_435_456,
        maximum_decoded_bytes: 2_147_483_648,
        maximum_jpip_fragments: 1_048_576,
        maximum_tag_tree_nodes: 16_777_216,
        maximum_work_units: 1_000_000_000,
    };

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == resource_input_byte_limit(self),)
    )]
    pub const fn maximum_input_bytes(&self) -> u64 {
        self.maximum_input_bytes
    }

    pub const fn maximum_metadata_bytes(&self) -> u64 {
        self.maximum_metadata_bytes
    }

    pub const fn maximum_box_depth(&self) -> u32 {
        self.maximum_box_depth
    }

    pub const fn maximum_compatible_brands(&self) -> u32 {
        self.maximum_compatible_brands
    }

    pub const fn maximum_codestreams(&self) -> u32 {
        self.maximum_codestreams
    }

    pub const fn maximum_width(&self) -> u32 {
        self.maximum_width
    }

    pub const fn maximum_height(&self) -> u32 {
        self.maximum_height
    }

    pub const fn maximum_pixels(&self) -> u64 {
        self.maximum_pixels
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value =>
        ensures value == resource_component_limit(self),
    ))]
    pub const fn maximum_components(&self) -> u16 {
        self.maximum_components
    }

    pub const fn maximum_tiles(&self) -> u32 {
        self.maximum_tiles
    }

    pub const fn maximum_tile_parts(&self) -> u32 {
        self.maximum_tile_parts
    }

    pub const fn maximum_resolutions_per_component(&self) -> u8 {
        self.maximum_resolutions_per_component
    }

    pub const fn maximum_precincts(&self) -> u64 {
        self.maximum_precincts
    }

    pub const fn maximum_code_blocks(&self) -> u64 {
        self.maximum_code_blocks
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == resource_code_block_pass_limit(self),)
    )]
    pub const fn maximum_code_block_passes(&self) -> u32 {
        self.maximum_code_block_passes
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == resource_code_block_byte_limit(self),)
    )]
    pub const fn maximum_code_block_bytes(&self) -> u64 {
        self.maximum_code_block_bytes
    }

    pub const fn maximum_layers(&self) -> u16 {
        self.maximum_layers
    }

    pub const fn maximum_frames(&self) -> u32 {
        self.maximum_frames
    }

    pub const fn maximum_packets(&self) -> u64 {
        self.maximum_packets
    }

    pub const fn maximum_packet_header_bytes(&self) -> u64 {
        self.maximum_packet_header_bytes
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == resource_decoded_bytes_limit(self),)
    )]
    pub const fn maximum_decoded_bytes(&self) -> u64 {
        self.maximum_decoded_bytes
    }

    pub const fn maximum_jpip_fragments(&self) -> u64 {
        self.maximum_jpip_fragments
    }

    pub const fn maximum_tag_tree_nodes(&self) -> u64 {
        self.maximum_tag_tree_nodes
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == resource_work_limit(self),)
    )]
    pub const fn maximum_work_units(&self) -> u64 {
        self.maximum_work_units
    }

    pub fn with_maximum_input_bytes(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::InputBytes,
            Self::HARDENED.maximum_input_bytes,
            value,
        )?;
        let mut updated = self;
        updated.maximum_input_bytes = value;
        Ok(updated)
    }

    pub fn with_maximum_metadata_bytes(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::MetadataBytes,
            Self::HARDENED.maximum_metadata_bytes,
            value,
        )?;
        let mut updated = self;
        updated.maximum_metadata_bytes = value;
        Ok(updated)
    }

    pub fn with_maximum_codestreams(self, value: u32) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::Codestreams,
            u64::from(Self::HARDENED.maximum_codestreams),
            u64::from(value),
        )?;
        let mut updated = self;
        updated.maximum_codestreams = value;
        Ok(updated)
    }

    pub fn with_maximum_compatible_brands(self, value: u32) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::CompatibleBrands,
            u64::from(Self::HARDENED.maximum_compatible_brands),
            u64::from(value),
        )?;
        let mut updated = self;
        updated.maximum_compatible_brands = value;
        Ok(updated)
    }

    pub fn with_maximum_tile_parts(self, value: u32) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::TileParts,
            u64::from(Self::HARDENED.maximum_tile_parts),
            u64::from(value),
        )?;
        let mut updated = self;
        updated.maximum_tile_parts = value;
        Ok(updated)
    }

    pub fn with_maximum_packet_header_bytes(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::PacketHeaderBytes,
            Self::HARDENED.maximum_packet_header_bytes,
            value,
        )?;
        let mut updated = self;
        updated.maximum_packet_header_bytes = value;
        Ok(updated)
    }

    pub fn with_maximum_packets(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(Resource::Packets, Self::HARDENED.maximum_packets, value)?;
        let mut updated = self;
        updated.maximum_packets = value;
        Ok(updated)
    }

    pub fn with_maximum_code_blocks(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::CodeBlocks,
            Self::HARDENED.maximum_code_blocks,
            value,
        )?;
        let mut updated = self;
        updated.maximum_code_blocks = value;
        Ok(updated)
    }

    pub fn with_maximum_code_block_passes(self, value: u32) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::CodeBlockPasses,
            u64::from(Self::HARDENED.maximum_code_block_passes),
            u64::from(value),
        )?;
        let mut updated = self;
        updated.maximum_code_block_passes = value;
        Ok(updated)
    }

    pub fn with_maximum_code_block_bytes(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::CodeBlockBytes,
            Self::HARDENED.maximum_code_block_bytes,
            value,
        )?;
        let mut updated = self;
        updated.maximum_code_block_bytes = value;
        Ok(updated)
    }

    pub fn with_maximum_decoded_bytes(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::DecodedBytes,
            Self::HARDENED.maximum_decoded_bytes,
            value,
        )?;
        let mut updated = self;
        updated.maximum_decoded_bytes = value;
        Ok(updated)
    }

    pub fn with_maximum_frames(self, value: u32) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::Frames,
            u64::from(Self::HARDENED.maximum_frames),
            u64::from(value),
        )?;
        let mut updated = self;
        updated.maximum_frames = value;
        Ok(updated)
    }

    pub fn with_maximum_jpip_fragments(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::JpipFragments,
            Self::HARDENED.maximum_jpip_fragments,
            value,
        )?;
        let mut updated = self;
        updated.maximum_jpip_fragments = value;
        Ok(updated)
    }

    pub fn with_maximum_work_units(self, value: u64) -> Result<Self, DecodeError> {
        Self::ensure_tightened(
            Resource::WorkUnits,
            Self::HARDENED.maximum_work_units,
            value,
        )?;
        let mut updated = self;
        updated.maximum_work_units = value;
        Ok(updated)
    }

    fn ensure_tightened(resource: Resource, hard: u64, requested: u64) -> Result<(), DecodeError> {
        if requested <= hard {
            Ok(())
        } else {
            Err(DecodeError::limit(resource, hard, requested))
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::HARDENED
    }
}
