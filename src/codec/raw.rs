use std::io::Cursor;
use std::time::Instant;
use rayon::prelude::*;
use crate::codec::jpeg::{decode_jpeg, DecodedImage};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RawDecodeResult {
    pub image: DecodedImage,
    pub is_preview: bool,
    pub camera_model: String,
}

/// Extract an embedded full/large JPEG preview from camera raw file bytes.
/// Inspects Fuji RAF header offsets, TIFF IFD JPEG interchange tags, and fast stream scanning.
pub fn extract_embedded_jpeg(data: &[u8]) -> Option<DecodedImage> {
    let start = Instant::now();

    // 1. Fujifilm RAF dedicated header
    // Offset 84: 4-byte BE JPEG offset, Offset 88: 4-byte BE JPEG length
    if data.starts_with(b"FUJIFILM") && data.len() >= 92 {
        let offset = u32::from_be_bytes([data[84], data[85], data[86], data[87]]) as usize;
        let len = u32::from_be_bytes([data[88], data[89], data[90], data[91]]) as usize;
        if offset + len <= data.len() && len > 64 {
            if let Ok(mut img) = decode_jpeg(&data[offset..offset + len]) {
                img.decode_duration_us = start.elapsed().as_micros();
                return Some(img);
            }
        }
    }

    // 2. TIFF-based RAW files (Canon CR2, Nikon NEF, Sony ARW, Adobe DNG, Olympus ORF, Panasonic RW2)
    // Inspect standard EXIF JPEGInterchangeFormat tags (0x0201 / 0x0202) in IFDs and SubIFDs (0x014A)
    let mut candidates = extract_tiff_jpeg_candidates(data);

    // If TIFF tags didn't provide any high-res candidates (> 500KB), complement with stream scan
    let has_large_tiff_candidate = candidates.iter().any(|&(_, l)| l > 500_000);
    if !has_large_tiff_candidate {
        let stream_candidates = find_jpeg_stream_candidates(data);
        for cand in stream_candidates {
            if !candidates.iter().any(|&(o, _)| o == cand.0) {
                candidates.push(cand);
            }
        }
    }

    // Sort descending by byte length so largest full-res previews are evaluated first
    candidates.sort_by_key(|&(_, l)| std::cmp::Reverse(l));

    let mut best_img: Option<DecodedImage> = None;
    let mut best_pixels: u64 = 0;

    for (offset, len) in candidates {
        if offset + len <= data.len() && len > 64 {
            if let Ok(mut img) = decode_jpeg(&data[offset..offset + len]) {
                let pixels = img.width as u64 * img.height as u64;
                if pixels > best_pixels {
                    best_pixels = pixels;
                    img.decode_duration_us = start.elapsed().as_micros();
                    best_img = Some(img);
                }
                // If high-resolution preview (>= 4K or >= 8 Megapixels), return immediately
                if best_pixels >= 8_000_000
                    || best_img.as_ref().map_or(false, |im| im.width >= 3840)
                {
                    break;
                }
            }
        }
    }

    best_img
}

/// Traverses TIFF header, chained IFDs, and SubIFDs looking for JPEG previews (tag 0x0201)
fn extract_tiff_jpeg_candidates(data: &[u8]) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();
    if data.len() < 16 {
        return candidates;
    }

    let is_le = if data.starts_with(b"II*\0") {
        true
    } else if data.starts_with(b"MM\0*") {
        false
    } else {
        return candidates;
    };

    let read_u16 = |buf: &[u8], offset: usize| -> Option<u16> {
        if offset + 2 > buf.len() {
            return None;
        }
        Some(if is_le {
            u16::from_le_bytes([buf[offset], buf[offset + 1]])
        } else {
            u16::from_be_bytes([buf[offset], buf[offset + 1]])
        })
    };

    let read_u32 = |buf: &[u8], offset: usize| -> Option<u32> {
        if offset + 4 > buf.len() {
            return None;
        }
        Some(if is_le {
            u32::from_le_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]])
        } else {
            u32::from_be_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]])
        })
    };

    let mut ifd_offset = match read_u32(data, 4) {
        Some(o) => o as usize,
        None => return candidates,
    };

    // Traverse up to 8 chained IFDs
    for _ in 0..8 {
        if ifd_offset == 0 || ifd_offset + 2 > data.len() {
            break;
        }

        let num_entries = match read_u16(data, ifd_offset) {
            Some(n) => n as usize,
            None => break,
        };
        let mut entry_pos = ifd_offset + 2;

        let mut jpeg_offset = None;
        let mut jpeg_len = None;

        for _ in 0..num_entries {
            if entry_pos + 12 > data.len() {
                break;
            }
            let tag = match read_u16(data, entry_pos) {
                Some(t) => t,
                None => break,
            };
            let count = match read_u32(data, entry_pos + 4) {
                Some(c) => c as usize,
                None => break,
            };
            let val = match read_u32(data, entry_pos + 8) {
                Some(v) => v as usize,
                None => break,
            };

            if tag == 0x0201 {
                // JPEGInterchangeFormat
                jpeg_offset = Some(val);
            } else if tag == 0x0202 {
                // JPEGInterchangeFormatLength
                jpeg_len = Some(val);
            } else if tag == 0x014A {
                // SubIFDs tag (contains array of offsets to child IFDs)
                for s in 0..count.min(16) {
                    let sub_pos = if count == 1 {
                        val
                    } else if val + (s + 1) * 4 <= data.len() {
                        read_u32(data, val + s * 4).unwrap_or(0) as usize
                    } else {
                        break;
                    };

                    if sub_pos > 0 && sub_pos + 2 <= data.len() {
                        if let Some(sub_entries) = read_u16(data, sub_pos) {
                            let mut sub_p = sub_pos + 2;
                            let mut s_off = None;
                            let mut s_len = None;
                            for _ in 0..sub_entries as usize {
                                if sub_p + 12 > data.len() {
                                    break;
                                }
                                let stag = read_u16(data, sub_p).unwrap_or(0);
                                let sval = read_u32(data, sub_p + 8).unwrap_or(0) as usize;
                                if stag == 0x0201 {
                                    s_off = Some(sval);
                                } else if stag == 0x0202 {
                                    s_len = Some(sval);
                                }
                                sub_p += 12;
                            }
                            if let (Some(o), Some(l)) = (s_off, s_len) {
                                if o + l <= data.len() && l > 64 {
                                    candidates.push((o, l));
                                }
                            }
                        }
                    }
                }
            }

            entry_pos += 12;
        }

        if let (Some(off), Some(len)) = (jpeg_offset, jpeg_len) {
            if off + len <= data.len() && len > 64 {
                candidates.push((off, len));
            }
        }

        // Next IFD offset is right after entries
        if entry_pos + 4 <= data.len() {
            ifd_offset = read_u32(data, entry_pos).unwrap_or(0) as usize;
        } else {
            break;
        }
    }

    candidates
}

/// Fast SOI/EOI Stream Search for embedded JPEG streams (\xFF\xD8\xFF ... \xFF\xD9)
fn find_jpeg_stream_candidates(data: &[u8]) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();
    let len = data.len();
    if len < 64 {
        return candidates;
    }

    let mut i = 0;
    while i + 3 < len {
        if data[i] == 0xFF && data[i + 1] == 0xD8 && data[i + 2] == 0xFF {
            let start_idx = i;
            let mut j = start_idx + 4;
            let mut found_eoi = None;
            while j + 1 < len {
                if data[j] == 0xFF && data[j + 1] == 0xD9 {
                    found_eoi = Some(j + 2);
                    break;
                }
                j += 1;
            }

            if let Some(end_idx) = found_eoi {
                let stream_len = end_idx - start_idx;
                if stream_len > 64 {
                    candidates.push((start_idx, stream_len));
                }
                i = end_idx;
                continue;
            }
        }
        i += 1;
    }

    candidates
}

/// Full Bayer demosaicing pipeline using rawloader with Rayon parallel processing
pub fn decode_full_raw(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();
    let mut cursor = Cursor::new(data);
    let raw = rawloader::decode(&mut cursor).map_err(|e| format!("RAW decode error: {:?}", e))?;

    let width = raw.width;
    let height = raw.height;

    if width == 0 || height == 0 {
        return Err("RAW image dimensions are zero".to_string());
    }

    let mut rgba = vec![0u8; width * height * 4];

    match raw.data {
        rawloader::RawImageData::Integer(ref pixels) => {
            demosaic_bayer_parallel(&raw, pixels, &mut rgba);
        }
        rawloader::RawImageData::Float(ref pixels) => {
            demosaic_float_parallel(&raw, pixels, &mut rgba);
        }
    }

    Ok(DecodedImage {
        width: width as u32,
        height: height as u32,
        rgba,
        decode_duration_us: start.elapsed().as_micros(),
        orientation: 1,
        is_hdr: false,
    })
}

/// High-speed Rayon multi-threaded bilinear demosaicing for Bayer CFA sensor arrays
fn demosaic_bayer_parallel(raw: &rawloader::RawImage, pixels: &[u16], out_rgba: &mut [u8]) {
    let width = raw.width;
    let height = raw.height;

    let black = raw.blacklevels[0] as f32;
    let white = (raw.whitelevels[0].max(raw.blacklevels[0] + 1)) as f32;
    let inv_range = 1.0 / (white - black);

    // Camera white-balance multipliers (normalized relative to green channel)
    let (wb_r, wb_g, wb_b) = if raw.wb_coeffs[1] > 0.0 && !raw.wb_coeffs[1].is_nan() {
        let base = raw.wb_coeffs[1];
        let r = if raw.wb_coeffs[0] > 0.0 && !raw.wb_coeffs[0].is_nan() {
            (raw.wb_coeffs[0] / base).clamp(0.2, 5.0)
        } else {
            1.0
        };
        let b = if raw.wb_coeffs[2] > 0.0 && !raw.wb_coeffs[2].is_nan() {
            (raw.wb_coeffs[2] / base).clamp(0.2, 5.0)
        } else {
            1.0
        };
        (r, 1.0, b)
    } else {
        (1.0, 1.0, 1.0)
    };

    // Camera-to-sRGB color matrix via cam_to_xyz_normalized and CIE D65 XYZ-to-sRGB
    let cam_to_xyz = raw.cam_to_xyz_normalized();
    let xyz_to_srgb = [
        [3.2404542f32, -1.5371385, -0.4985314],
        [-0.9692660, 1.8760108, 0.0415560],
        [0.0556434, -0.2040259, 1.0572252],
    ];

    let mut matrix = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            matrix[i][j] = xyz_to_srgb[i][0] * cam_to_xyz[0][j]
                + xyz_to_srgb[i][1] * cam_to_xyz[1][j]
                + xyz_to_srgb[i][2] * cam_to_xyz[2][j];
        }
    }
    let has_matrix = matrix[0][0] != 0.0 || matrix[0][1] != 0.0;

    // Parallelize per scanline
    out_rgba
        .par_chunks_exact_mut(width * 4)
        .enumerate()
        .for_each(|(y, row_bytes)| {
            for x in 0..width {
                let color = raw.cfa.color_at(y, x);

                // Bilinear interpolation of sensor pixels
                let (r, g, b) = match color {
                    0 => {
                        // Current pixel is Red
                        let r_val = get_pixel_normalized(pixels, width, height, x, y, black, inv_range);

                        // Green is average of cross neighbors
                        let g_sum = get_pixel_normalized(pixels, width, height, x.saturating_sub(1), y, black, inv_range)
                            + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), y, black, inv_range)
                            + get_pixel_normalized(pixels, width, height, x, y.saturating_sub(1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, x, (y + 1).min(height - 1), black, inv_range);
                        let g_val = g_sum * 0.25;

                        // Blue is average of 4 diagonal neighbors
                        let b_sum = get_pixel_normalized(pixels, width, height, x.saturating_sub(1), y.saturating_sub(1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), y.saturating_sub(1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, x.saturating_sub(1), (y + 1).min(height - 1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), (y + 1).min(height - 1), black, inv_range);
                        let b_val = b_sum * 0.25;

                        (r_val, g_val, b_val)
                    }
                    1 => {
                        // Current pixel is Green
                        let g_val = get_pixel_normalized(pixels, width, height, x, y, black, inv_range);

                        // Neighboring Red / Blue horizontal or vertical
                        let h1 = raw.cfa.color_at(y, x.saturating_sub(1));
                        let (r_val, b_val) = if h1 == 0 {
                            let r_avg = (get_pixel_normalized(pixels, width, height, x.saturating_sub(1), y, black, inv_range)
                                + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), y, black, inv_range))
                                * 0.5;
                            let b_avg = (get_pixel_normalized(pixels, width, height, x, y.saturating_sub(1), black, inv_range)
                                + get_pixel_normalized(pixels, width, height, x, (y + 1).min(height - 1), black, inv_range))
                                * 0.5;
                            (r_avg, b_avg)
                        } else {
                            let b_avg = (get_pixel_normalized(pixels, width, height, x.saturating_sub(1), y, black, inv_range)
                                + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), y, black, inv_range))
                                * 0.5;
                            let r_avg = (get_pixel_normalized(pixels, width, height, x, y.saturating_sub(1), black, inv_range)
                                + get_pixel_normalized(pixels, width, height, x, (y + 1).min(height - 1), black, inv_range))
                                * 0.5;
                            (r_avg, b_avg)
                        };

                        (r_val, g_val, b_val)
                    }
                    _ => {
                        // Current pixel is Blue
                        let b_val = get_pixel_normalized(pixels, width, height, x, y, black, inv_range);

                        // Green is average of cross neighbors
                        let g_sum = get_pixel_normalized(pixels, width, height, x.saturating_sub(1), y, black, inv_range)
                            + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), y, black, inv_range)
                            + get_pixel_normalized(pixels, width, height, x, y.saturating_sub(1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, x, (y + 1).min(height - 1), black, inv_range);
                        let g_val = g_sum * 0.25;

                        // Red is average of 4 diagonal neighbors
                        let r_sum = get_pixel_normalized(pixels, width, height, x.saturating_sub(1), y.saturating_sub(1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), y.saturating_sub(1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, x.saturating_sub(1), (y + 1).min(height - 1), black, inv_range)
                            + get_pixel_normalized(pixels, width, height, (x + 1).min(width - 1), (y + 1).min(height - 1), black, inv_range);
                        let r_val = r_sum * 0.25;

                        (r_val, g_val, b_val)
                    }
                };

                // White balance
                let rw = r * wb_r;
                let gw = g * wb_g;
                let bw = b * wb_b;

                // Camera matrix transformation to calibrated linear sRGB
                let (r_lin, g_lin, b_lin) = if has_matrix {
                    let rc = matrix[0][0] * rw + matrix[0][1] * gw + matrix[0][2] * bw;
                    let gc = matrix[1][0] * rw + matrix[1][1] * gw + matrix[1][2] * bw;
                    let bc = matrix[2][0] * rw + matrix[2][1] * gw + matrix[2][2] * bw;
                    (rc, gc, bc)
                } else {
                    (rw, gw, bw)
                };

                let r_out = to_srgb_u8(r_lin);
                let g_out = to_srgb_u8(g_lin);
                let b_out = to_srgb_u8(b_lin);

                let pixel_idx = x * 4;
                row_bytes[pixel_idx] = r_out;
                row_bytes[pixel_idx + 1] = g_out;
                row_bytes[pixel_idx + 2] = b_out;
                row_bytes[pixel_idx + 3] = 255;
            }
        });
}

#[inline(always)]
fn get_pixel_normalized(
    pixels: &[u16],
    width: usize,
    _height: usize,
    x: usize,
    y: usize,
    black: f32,
    inv_range: f32,
) -> f32 {
    let raw_val = pixels[y * width + x] as f32;
    ((raw_val - black) * inv_range).clamp(0.0, 1.0)
}

#[inline(always)]
fn to_srgb_u8(linear: f32) -> u8 {
    let clamped = linear.clamp(0.0, 1.0);
    // Standard IEC 61966-2-1 piecewise sRGB transfer function
    let srgb = if clamped <= 0.0031308 {
        12.92 * clamped
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

fn demosaic_float_parallel(raw: &rawloader::RawImage, pixels: &[f32], out_rgba: &mut [u8]) {
    let width = raw.width;

    out_rgba
        .par_chunks_exact_mut(width * 4)
        .enumerate()
        .for_each(|(y, row_bytes)| {
            for x in 0..width {
                let val = pixels[y * width + x];
                let srgb = to_srgb_u8(val);
                let pixel_idx = x * 4;
                row_bytes[pixel_idx] = srgb;
                row_bytes[pixel_idx + 1] = srgb;
                row_bytes[pixel_idx + 2] = srgb;
                row_bytes[pixel_idx + 3] = 255;
            }
        });
}
