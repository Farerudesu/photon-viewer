use super::jpeg::DecodedImage;
use std::time::Instant;

pub fn decode_psd(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    if data.len() < 26 || !data.starts_with(b"8BPS") {
        return Err("Invalid PSD signature".to_string());
    }

    // Read PSD header (big-endian)
    let channels = u16::from_be_bytes([data[12], data[13]]) as usize;
    let height = u32::from_be_bytes([data[14], data[15], data[16], data[17]]) as usize;
    let width = u32::from_be_bytes([data[18], data[19], data[20], data[21]]) as usize;
    let depth = u16::from_be_bytes([data[22], data[23]]) as usize;
    let color_mode = u16::from_be_bytes([data[24], data[25]]);

    if width == 0 || height == 0 || width > 65536 || height > 65536 {
        return Err(format!("Invalid PSD dimensions: {}x{}", width, height));
    }

    // Color mode 3 = RGB, 1 = Grayscale
    if color_mode != 3 && color_mode != 1 {
        // Fallback to image crate if other modes
        let img = image::load_from_memory(data)
            .map_err(|e| format!("PSD load error: {}", e))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        return Ok(DecodedImage {
            width: w,
            height: h,
            rgba: rgba.into_raw(),
            decode_duration_us: start.elapsed().as_micros(),
            orientation: 1,
            is_hdr: false,
        });
    }

    // Skip Color Mode Data
    let mut offset = 26;
    if offset + 4 > data.len() {
        return Err("Truncated PSD".to_string());
    }
    let color_data_len = u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
    offset += 4 + color_data_len;

    // Skip Image Resources
    if offset + 4 > data.len() {
        return Err("Truncated PSD".to_string());
    }
    let img_res_len = u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
    offset += 4 + img_res_len;

    // Skip Layer and Mask Data
    if offset + 4 > data.len() {
        return Err("Truncated PSD".to_string());
    }
    let layer_mask_len = u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
    offset += 4 + layer_mask_len;

    // Image Data Section
    if offset + 2 > data.len() {
        return Err("Truncated PSD image data".to_string());
    }
    let compression = u16::from_be_bytes([data[offset], data[offset+1]]);
    offset += 2;

    let num_pixels = width * height;
    let mut channel_data = vec![vec![0u8; num_pixels]; channels];

    match compression {
        0 => {
            // Raw uncompressed data
            if depth == 8 {
                for ch in 0..channels {
                    let end = offset + num_pixels;
                    if end <= data.len() {
                        channel_data[ch].copy_from_slice(&data[offset..end]);
                        offset = end;
                    }
                }
            } else {
                // 16-bit raw: take high byte
                let bytes_per_ch = num_pixels * 2;
                for ch in 0..channels {
                    let end = offset + bytes_per_ch;
                    if end <= data.len() {
                        for (i, p) in channel_data[ch].iter_mut().enumerate() {
                            *p = data[offset + i * 2];
                        }
                        offset = end;
                    }
                }
            }
        }
        1 => {
            // RLE (PackBits) compression
            let scanline_count = height * channels;
            let byte_counts_len = scanline_count * 2;
            if offset + byte_counts_len > data.len() {
                return Err("Truncated PSD RLE byte counts".to_string());
            }

            let mut rle_offset = offset + byte_counts_len;

            for ch in 0..channels {
                let mut pixel_idx = 0;
                for _row in 0..height {
                    while pixel_idx < num_pixels && rle_offset < data.len() {
                        let header = data[rle_offset] as i8;
                        rle_offset += 1;

                        if header >= 0 {
                            // Copy next (header + 1) literal bytes
                            let count = (header as usize) + 1;
                            let end = (rle_offset + count).min(data.len());
                            for &b in &data[rle_offset..end] {
                                if pixel_idx < num_pixels {
                                    channel_data[ch][pixel_idx] = b;
                                    pixel_idx += 1;
                                }
                            }
                            rle_offset += count;
                        } else if header != -128 {
                            // Repeat next byte (1 - header) times
                            let count = (1 - header as isize) as usize;
                            if rle_offset < data.len() {
                                let val = data[rle_offset];
                                rle_offset += 1;
                                for _ in 0..count {
                                    if pixel_idx < num_pixels {
                                        channel_data[ch][pixel_idx] = val;
                                        pixel_idx += 1;
                                    }
                                }
                            }
                        }
                        if pixel_idx % width == 0 {
                            break; // Done with this row
                        }
                    }
                }
            }
        }
        _ => return Err(format!("Unsupported PSD compression: {}", compression)),
    }

    // Interleave channels to RGBA8
    let mut rgba = Vec::with_capacity(num_pixels * 4);
    if color_mode == 3 && channels >= 3 {
        let r_ch = &channel_data[0];
        let g_ch = &channel_data[1];
        let b_ch = &channel_data[2];
        let a_ch = if channels >= 4 { Some(&channel_data[3]) } else { None };

        for i in 0..num_pixels {
            rgba.push(r_ch[i]);
            rgba.push(g_ch[i]);
            rgba.push(b_ch[i]);
            rgba.push(a_ch.map(|a| a[i]).unwrap_or(255));
        }
    } else {
        // Grayscale
        let gray_ch = &channel_data[0];
        let a_ch = if channels >= 2 { Some(&channel_data[1]) } else { None };
        for i in 0..num_pixels {
            let g = gray_ch[i];
            rgba.push(g);
            rgba.push(g);
            rgba.push(g);
            rgba.push(a_ch.map(|a| a[i]).unwrap_or(255));
        }
    }

    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width: width as u32,
        height: height as u32,
        rgba,
        decode_duration_us: duration,
        orientation: 1,
        is_hdr: false,
    })
}
