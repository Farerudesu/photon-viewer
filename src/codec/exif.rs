/// Reads the EXIF orientation tag (0x0112) from raw bytes.
/// Supports JPEG (APP1 Exif), TIFF, Camera RAW, and WebP EXIF chunks.
/// Returns 1..=8 (defaults to 1 = Normal/Unrotated if not present).
pub fn read_exif_orientation(data: &[u8]) -> u16 {
    // 1. JPEG: Scan for APP1 (0xFFE1) with "Exif\0\0"
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut idx = 2;
        while idx + 4 < data.len() {
            if data[idx] == 0xFF {
                let marker = data[idx + 1];
                if marker == 0xDA || marker == 0xD9 {
                    // Start of Scan or End of Image
                    break;
                }
                let len = u16::from_be_bytes([data[idx + 2], data[idx + 3]]) as usize;
                if marker == 0xE1 && idx + 2 + len <= data.len() {
                    let segment = &data[idx + 4..idx + 2 + len];
                    if segment.starts_with(b"Exif\0\0") && segment.len() > 14 {
                        return read_tiff_orientation(&segment[6..]);
                    }
                }
                idx += 2 + len;
            } else {
                idx += 1;
            }
        }
    }

    // 2. TIFF / Camera RAW / DNG: Starts directly with TIFF header
    if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        return read_tiff_orientation(data);
    }

    // 3. WebP: RIFF chunk with fourcc "EXIF"
    if data.len() >= 20 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        let mut idx = 12;
        while idx + 8 < data.len() {
            let chunk_id = &data[idx..idx + 4];
            let chunk_len = u32::from_le_bytes([data[idx + 4], data[idx + 5], data[idx + 6], data[idx + 7]]) as usize;
            if chunk_id == b"EXIF" && idx + 8 + chunk_len <= data.len() {
                let chunk_data = &data[idx + 8..idx + 8 + chunk_len];
                return read_tiff_orientation(chunk_data);
            }
            idx += 8 + (chunk_len + 1) & !1; // 2-byte aligned
        }
    }

    1
}

/// Parses TIFF IFD0 looking for orientation tag 0x0112
fn read_tiff_orientation(tiff: &[u8]) -> u16 {
    if tiff.len() < 14 {
        return 1;
    }

    let is_le = if tiff.starts_with(b"II*\0") {
        true
    } else if tiff.starts_with(b"MM\0*") {
        false
    } else {
        return 1;
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

    let ifd0_offset = match read_u32(tiff, 4) {
        Some(off) => off as usize,
        None => return 1,
    };

    if ifd0_offset + 2 > tiff.len() {
        return 1;
    }

    let num_entries = match read_u16(tiff, ifd0_offset) {
        Some(n) => n as usize,
        None => return 1,
    };

    let mut entry_pos = ifd0_offset + 2;

    for _ in 0..num_entries {
        if entry_pos + 12 > tiff.len() {
            break;
        }

        if let Some(tag) = read_u16(tiff, entry_pos) {
            if tag == 0x0112 {
                // Orientation tag
                if let Some(val) = read_u16(tiff, entry_pos + 8) {
                    if (1..=8).contains(&val) {
                        return val;
                    }
                }
            }
        }
        entry_pos += 12;
    }

    1
}

/// Convert EXIF orientation tag 1..=8 into rotation angle and horizontal/vertical flip flags.
/// Evaluated at zero CPU memory cost via the GPU transformation matrix.
pub fn orientation_to_transform(orientation: u16) -> (f32, bool, bool) {
    match orientation {
        1 => (0.0, false, false),     // Top-Left: Normal
        2 => (0.0, true, false),      // Top-Right: Flip Horizontal
        3 => (180.0, false, false),   // Bottom-Right: Rotate 180°
        4 => (0.0, false, true),      // Bottom-Left: Flip Vertical
        5 => (90.0, true, false),     // Left-Top: Rotate 90° CW + Flip Horizontal
        6 => (90.0, false, false),    // Right-Top: Rotate 90° CW
        7 => (270.0, true, false),    // Right-Bottom: Rotate 270° CW + Flip Horizontal
        8 => (270.0, false, false),   // Left-Bottom: Rotate 270° CW (90° CCW)
        _ => (0.0, false, false),
    }
}
