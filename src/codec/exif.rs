/// Reads the EXIF orientation tag (0x0112) from raw bytes.
/// Supports JPEG (APP1 Exif), TIFF, Camera RAW, and WebP EXIF chunks.
/// Returns 1..=8 (defaults to 1 = Normal/Unrotated if not present).
pub fn read_exif_orientation(data: &[u8]) -> u16 {
    // 1. JPEG: Scan for APP1 (0xFFE1) with "Exif\0\0"
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut idx: usize = 2;
        while let Some(&b) = data.get(idx) {
            if b == 0xFF {
                let Some(&marker) = data.get(idx + 1) else { break; };
                if marker == 0xDA || marker == 0xD9 {
                    // Start of Scan or End of Image
                    break;
                }
                let Some(&b2) = data.get(idx + 2) else { break; };
                let Some(&b3) = data.get(idx + 3) else { break; };
                let len = u16::from_be_bytes([b2, b3]) as usize;
                if marker == 0xE1 {
                    let start = idx.saturating_add(4);
                    let end = idx.saturating_add(2).saturating_add(len);
                    if let Some(segment) = data.get(start..end) {
                        if segment.starts_with(b"Exif\0\0") && segment.len() > 14 {
                            return read_tiff_orientation(&segment[6..]);
                        }
                    }
                }
                idx = idx.saturating_add(2).saturating_add(len);
            } else {
                idx = idx.saturating_add(1);
            }
        }
    }

    // 2. TIFF / Camera RAW / DNG: Starts directly with TIFF header
    if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        return read_tiff_orientation(data);
    }

    // 3. WebP: RIFF chunk with fourcc "EXIF"
    if data.len() >= 20 && data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        let mut idx: usize = 12;
        while idx.saturating_add(8) <= data.len() {
            let Some(chunk_id) = data.get(idx..idx + 4) else { break; };
            let Some(len_bytes) = data.get(idx + 4..idx + 8) else { break; };
            let chunk_len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
            let start = idx.saturating_add(8);
            let end = start.saturating_add(chunk_len);
            if chunk_id == b"EXIF" {
                if let Some(chunk_data) = data.get(start..end) {
                    return read_tiff_orientation(chunk_data);
                }
            }
            let step = (8usize.saturating_add(chunk_len).saturating_add(1)) & !1;
            idx = idx.saturating_add(step);
        }
    }

    // 4. PNG: Scan chunks for "eXIf"
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        let mut idx: usize = 8;
        while idx.saturating_add(8) <= data.len() {
            let Some(len_bytes) = data.get(idx..idx + 4) else { break; };
            let chunk_len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
            let Some(chunk_type) = data.get(idx + 4..idx + 8) else { break; };
            if chunk_type == b"eXIf" {
                let start = idx.saturating_add(8);
                let end = start.saturating_add(chunk_len);
                if let Some(chunk_data) = data.get(start..end) {
                    return read_tiff_orientation(chunk_data);
                }
            }
            if chunk_type == b"IEND" {
                break;
            }
            let step = 12usize.saturating_add(chunk_len);
            idx = idx.saturating_add(step);
        }
    }

    1
}

/// Detects whether an image is sRGB (0), Display P3 (1), or Adobe RGB (2)
/// by inspecting ICC profiles and EXIF ColorSpace metadata tags.
pub fn detect_color_space(data: &[u8]) -> u32 {
    // 1. JPEG: Inspect APP2 (ICC_PROFILE) and APP1 (Exif ColorSpace tag 0xA001)
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut idx: usize = 2;
        while let Some(&b) = data.get(idx) {
            if b == 0xFF {
                let Some(&marker) = data.get(idx + 1) else { break; };
                if marker == 0xDA || marker == 0xD9 {
                    break;
                }
                let Some(&b2) = data.get(idx + 2) else { break; };
                let Some(&b3) = data.get(idx + 3) else { break; };
                let len = u16::from_be_bytes([b2, b3]) as usize;

                if marker == 0xE2 {
                    // APP2: ICC Profile
                    let start = idx.saturating_add(4);
                    let end = idx.saturating_add(2).saturating_add(len);
                    if let Some(segment) = data.get(start..end) {
                        if segment.starts_with(b"ICC_PROFILE\0") {
                            if contains_subslice(segment, b"Display P3") || contains_subslice(segment, b"DCI-P3") {
                                return 1;
                            }
                            if contains_subslice(segment, b"Adobe RGB") || contains_subslice(segment, b"AdobeRGB") {
                                return 2;
                            }
                        }
                    }
                } else if marker == 0xE1 {
                    // APP1: EXIF ColorSpace tag 0xA001
                    let start = idx.saturating_add(4);
                    let end = idx.saturating_add(2).saturating_add(len);
                    if let Some(segment) = data.get(start..end) {
                        if segment.starts_with(b"Exif\0\0") && segment.len() > 14 {
                            let cs = read_tiff_color_space(&segment[6..]);
                            if cs != 0 {
                                return cs;
                            }
                        }
                    }
                }
                idx = idx.saturating_add(2).saturating_add(len);
            } else {
                idx = idx.saturating_add(1);
            }
        }
    }

    // 2. PNG: Inspect iCCP chunk and eXIf chunk
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        let mut idx: usize = 8;
        while idx.saturating_add(8) <= data.len() {
            let Some(len_bytes) = data.get(idx..idx + 4) else { break; };
            let chunk_len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
            let Some(chunk_type) = data.get(idx + 4..idx + 8) else { break; };
            let start = idx.saturating_add(8);
            let end = start.saturating_add(chunk_len);

            if chunk_type == b"iCCP" {
                if let Some(chunk_data) = data.get(start..end) {
                    if contains_subslice(chunk_data, b"Display P3") || contains_subslice(chunk_data, b"DCI-P3") {
                        return 1;
                    }
                    if contains_subslice(chunk_data, b"Adobe RGB") || contains_subslice(chunk_data, b"AdobeRGB") {
                        return 2;
                    }
                }
            } else if chunk_type == b"eXIf" {
                if let Some(chunk_data) = data.get(start..end) {
                    let cs = read_tiff_color_space(chunk_data);
                    if cs != 0 {
                        return cs;
                    }
                }
            } else if chunk_type == b"IEND" {
                break;
            }
            let step = 12usize.saturating_add(chunk_len);
            idx = idx.saturating_add(step);
        }
    }

    // 3. TIFF / Camera RAW / DNG: Direct TIFF IFD tags
    if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        return read_tiff_color_space(data);
    }

    0 // Default to sRGB
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn read_tiff_color_space(tiff: &[u8]) -> u32 {
    if tiff.len() < 14 {
        return 0;
    }

    let is_le = if tiff.starts_with(b"II*\0") {
        true
    } else if tiff.starts_with(b"MM\0*") {
        false
    } else {
        return 0;
    };

    let read_u16 = |buf: &[u8], offset: usize| -> Option<u16> {
        let s = buf.get(offset..offset.checked_add(2)?)?;
        Some(if is_le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    };

    let read_u32 = |buf: &[u8], offset: usize| -> Option<u32> {
        let s = buf.get(offset..offset.checked_add(4)?)?;
        Some(if is_le {
            u32::from_le_bytes([s[0], s[1], s[2], s[3]])
        } else {
            u32::from_be_bytes([s[0], s[1], s[2], s[3]])
        })
    };

    let ifd0_offset = match read_u32(tiff, 4) {
        Some(off) => off as usize,
        None => return 0,
    };

    let num_entries = match read_u16(tiff, ifd0_offset) {
        Some(n) => n as usize,
        None => return 0,
    };

    let mut entry_pos = ifd0_offset.saturating_add(2);

    for _ in 0..num_entries {
        if entry_pos.saturating_add(12) > tiff.len() {
            break;
        }

        if let Some(tag) = read_u16(tiff, entry_pos) {
            if tag == 0xA001 {
                // ColorSpace tag: 1 = sRGB, 2 or 65535 = Adobe RGB / Uncalibrated
                if let Some(val) = read_u16(tiff, entry_pos + 8) {
                    if val == 2 || val == 0xFFFF {
                        return 2; // Adobe RGB
                    } else if val == 1 {
                        return 0; // sRGB
                    }
                }
            } else if tag == 0x8773 {
                // InterColorProfile tag: offset points to raw ICC profile
                if let Some(icc_offset) = read_u32(tiff, entry_pos + 8) {
                    let off = icc_offset as usize;
                    if let Some(icc_data) = tiff.get(off..off.saturating_add(512)) {
                        if contains_subslice(icc_data, b"Display P3") || contains_subslice(icc_data, b"DCI-P3") {
                            return 1;
                        }
                        if contains_subslice(icc_data, b"Adobe RGB") || contains_subslice(icc_data, b"AdobeRGB") {
                            return 2;
                        }
                    }
                }
            }
        }
        entry_pos = entry_pos.saturating_add(12);
    }

    0
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
        let s = buf.get(offset..offset.checked_add(2)?)?;
        Some(if is_le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    };

    let read_u32 = |buf: &[u8], offset: usize| -> Option<u32> {
        let s = buf.get(offset..offset.checked_add(4)?)?;
        Some(if is_le {
            u32::from_le_bytes([s[0], s[1], s[2], s[3]])
        } else {
            u32::from_be_bytes([s[0], s[1], s[2], s[3]])
        })
    };

    let ifd0_offset = match read_u32(tiff, 4) {
        Some(off) => off as usize,
        None => return 1,
    };

    let num_entries = match read_u16(tiff, ifd0_offset) {
        Some(n) => n as usize,
        None => return 1,
    };

    let mut entry_pos = ifd0_offset.saturating_add(2);

    for _ in 0..num_entries {
        if entry_pos.saturating_add(12) > tiff.len() {
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
        entry_pos = entry_pos.saturating_add(12);
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
