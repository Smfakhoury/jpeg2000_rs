use crate::ResourceLimits;

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecodeOptions {
    limits: ResourceLimits,
}

impl DecodeOptions {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    pub const fn limits(&self) -> &ResourceLimits {
        &self.limits
    }
}

/// A half-open image-reference-grid rectangle.
///
/// `x` and `y` are absolute JPEG 2000 image-space coordinates (they include
/// the SIZ image origin), and the selected interval is
/// `[x, x + width) × [y, y + height)`.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ImageRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ImageRegion {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum TileSelection {
    #[default]
    All,
    Index(u16),
    Set(Vec<u16>),
}

/// Spatial and resolution selection for the `*_selected` decode APIs.
///
/// `discard_levels` selects the native JPEG 2000 resolution obtained by
/// omitting that many highest DWT resolution levels. It is not a post-decode
/// resize operation.
#[derive(Clone, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecodeSelection {
    tiles: TileSelection,
    region: Option<ImageRegion>,
    discard_levels: u8,
}

impl DecodeSelection {
    pub const fn full_image() -> Self {
        Self {
            tiles: TileSelection::All,
            region: None,
            discard_levels: 0,
        }
    }

    pub const fn tile(tile_index: u16) -> Self {
        Self {
            tiles: TileSelection::Index(tile_index),
            region: None,
            discard_levels: 0,
        }
    }

    pub fn tiles(tile_indices: Vec<u16>) -> Self {
        Self {
            tiles: TileSelection::Set(tile_indices),
            region: None,
            discard_levels: 0,
        }
    }

    pub fn with_region(self, region: ImageRegion) -> Self {
        Self {
            tiles: self.tiles,
            region: Some(region),
            discard_levels: self.discard_levels,
        }
    }

    pub fn with_discard_levels(self, discard_levels: u8) -> Self {
        Self {
            tiles: self.tiles,
            region: self.region,
            discard_levels,
        }
    }

    pub const fn tile_selection(&self) -> &TileSelection {
        &self.tiles
    }

    pub const fn region(&self) -> Option<ImageRegion> {
        self.region
    }

    pub const fn discard_levels(&self) -> u8 {
        self.discard_levels
    }
}
