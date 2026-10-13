//! Hard-coded tuning. Planned to become CLI flags.

/// Number of mosaic columns; rows follow from the target aspect ratio.
pub const GRID_COLS: u32 = 128;
/// Rendered size of one square tile in the output image.
pub const TILE_PX: u32 = 32;
/// Blend strength toward the target cell colour (0 = pure tiles).
pub const BLEND_ALPHA: f32 = 0.2;
/// A tile may not repeat within this Chebyshev radius of cells.
pub const EXCLUSION_RADIUS: i32 = 2;
/// Reuse penalty = PENALTY_FACTOR * median nearest-neighbour dist² * uses.
pub const PENALTY_FACTOR: f32 = 0.5;
pub const SEED: u64 = 42;
/// Weight of the L (lightness) channel in the Oklab distance.
pub const LUMA_WEIGHT: f32 = 2.0;
/// Tiles closer than this (rms Oklab distance) to an already-kept tile are dropped.
pub const DUP_EPS: f32 = 0.02;
/// A cell whose nearest tile is farther than this is "poorly matched".
pub const GAP_THRESHOLD: f32 = 0.12;
pub const MIN_DISTINCT_TILES: usize = 50;
pub const GAP_NUDGE_FRACTION: f32 = 0.2;
pub const INDEX_FILE: &str = ".mosaicer-index.json";
pub const MAX_OUTPUT_PX: u32 = 8192;
