use crate::codec::jpeg::DecodedImage;
use rayon::prelude::*;
use std::sync::Arc;

pub const TILE_SIZE: u32 = 512;

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub struct TileKey {
    pub col: u32,
    pub row: u32,
}

pub struct DecodedTile {
    pub key: TileKey,
    pub width: u32,
    pub height: u32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub rgba: Vec<u8>,
}

#[derive(Clone)]
pub struct TileGrid {
    pub full_width: u32,
    pub full_height: u32,
    pub tile_size: u32,
    pub cols: u32,
    pub rows: u32,
    pub overview: Arc<DecodedImage>,
    source_pixels: Arc<Vec<u8>>,
}

impl TileGrid {
    pub fn new(full_width: u32, full_height: u32, rgba_pixels: Arc<Vec<u8>>) -> Self {
        let cols = (full_width + TILE_SIZE - 1) / TILE_SIZE;
        let rows = (full_height + TILE_SIZE - 1) / TILE_SIZE;

        let overview = Arc::new(create_overview(full_width, full_height, &rgba_pixels, 4096));

        Self {
            full_width,
            full_height,
            tile_size: TILE_SIZE,
            cols,
            rows,
            overview,
            source_pixels: rgba_pixels,
        }
    }

    /// Extract a specific high-resolution tile at (col, row)
    pub fn extract_tile(&self, key: TileKey) -> Option<DecodedTile> {
        if key.col >= self.cols || key.row >= self.rows {
            return None;
        }

        let x0 = key.col * self.tile_size;
        let y0 = key.row * self.tile_size;
        let x1 = ((key.col + 1) * self.tile_size).min(self.full_width);
        let y1 = ((key.row + 1) * self.tile_size).min(self.full_height);

        let tile_w = x1 - x0;
        let tile_h = y1 - y0;

        let mut rgba = vec![0u8; (tile_w * tile_h * 4) as usize];
        let full_stride = (self.full_width * 4) as usize;
        let tile_stride = (tile_w * 4) as usize;

        for ty in 0..tile_h {
            let src_y = (y0 + ty) as usize;
            let src_offset = src_y * full_stride + (x0 * 4) as usize;
            let dst_offset = (ty as usize) * tile_stride;

            rgba[dst_offset..dst_offset + tile_stride]
                .copy_from_slice(&self.source_pixels[src_offset..src_offset + tile_stride]);
        }

        let u0 = x0 as f32 / self.full_width as f32;
        let v0 = y0 as f32 / self.full_height as f32;
        let u1 = x1 as f32 / self.full_width as f32;
        let v1 = y1 as f32 / self.full_height as f32;

        Some(DecodedTile {
            key,
            width: tile_w,
            height: tile_h,
            u0,
            v0,
            u1,
            v1,
            rgba,
        })
    }
}

/// Creates a downsampled base overview proxy fitting within max_dimension using box averaging
fn create_overview(full_w: u32, full_h: u32, pixels: &[u8], max_dim: u32) -> DecodedImage {
    let scale = ((full_w.max(full_h) + max_dim - 1) / max_dim).max(1);

    let out_w = full_w / scale;
    let out_h = full_h / scale;

    let mut out_rgba = vec![0u8; (out_w * out_h * 4) as usize];
    let full_stride = (full_w * 4) as usize;
    let out_stride = (out_w * 4) as usize;

    out_rgba
        .par_chunks_exact_mut(out_stride)
        .enumerate()
        .for_each(|(oy, row)| {
            let sy_start = oy as u32 * scale;
            let sy_end = ((oy as u32 + 1) * scale).min(full_h);
            let box_h = (sy_end - sy_start).max(1);

            for ox in 0..out_w {
                let sx_start = ox * scale;
                let sx_end = ((ox + 1) * scale).min(full_w);
                let box_w = (sx_end - sx_start).max(1);
                let count = (box_w * box_h) as u64;

                let mut sum_r = 0u64;
                let mut sum_g = 0u64;
                let mut sum_b = 0u64;
                let mut sum_a = 0u64;

                for sy in sy_start..sy_end {
                    let row_start = (sy as usize) * full_stride;
                    for sx in sx_start..sx_end {
                        let idx = row_start + (sx as usize) * 4;
                        sum_r += pixels[idx] as u64;
                        sum_g += pixels[idx + 1] as u64;
                        sum_b += pixels[idx + 2] as u64;
                        sum_a += pixels[idx + 3] as u64;
                    }
                }

                let dst_idx = (ox * 4) as usize;
                row[dst_idx] = (sum_r / count) as u8;
                row[dst_idx + 1] = (sum_g / count) as u8;
                row[dst_idx + 2] = (sum_b / count) as u8;
                row[dst_idx + 3] = (sum_a / count) as u8;
            }
        });

    DecodedImage {
        width: out_w,
        height: out_h,
        rgba: out_rgba,
        decode_duration_us: 0,
        orientation: 1,
        is_hdr: false,
    }
}
