pub mod grid;
pub mod manager;

#[allow(unused_imports)]
pub use grid::{TileGrid, TileKey, TILE_SIZE};
pub use manager::TileManager;

/// Threshold for activating tile-based virtual texture rendering
pub const HUGE_IMAGE_DIMENSION_LIMIT: u32 = 8192;
pub const HUGE_IMAGE_PIXEL_LIMIT: u64 = 64_000_000; // 64 Megapixels

pub fn is_huge_image(width: u32, height: u32) -> bool {
    width > HUGE_IMAGE_DIMENSION_LIMIT
        || height > HUGE_IMAGE_DIMENSION_LIMIT
        || (width as u64 * height as u64) > HUGE_IMAGE_PIXEL_LIMIT
}
