#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
    Bmp,
    Tiff,
    RawCanonCr2,
    RawCanonCr3,
    RawNikonNef,
    RawSonyArw,
    RawFujiRaf,
    RawOlympusOrf,
    RawPanasonicRw2,
    RawDng,
    Avif,
    Heif,
    JpegXl,
    Hdr,
    OpenExr,
    Psd,
    Ico,
    Tga,
    Svg,
    Unknown,
}

impl ImageFormat {
    pub fn name(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "JPEG",
            ImageFormat::Png => "PNG",
            ImageFormat::Gif => "GIF",
            ImageFormat::WebP => "WebP",
            ImageFormat::Bmp => "BMP",
            ImageFormat::Tiff => "TIFF",
            ImageFormat::RawCanonCr2 => "Canon CR2 RAW",
            ImageFormat::RawCanonCr3 => "Canon CR3 RAW",
            ImageFormat::RawNikonNef => "Nikon NEF RAW",
            ImageFormat::RawSonyArw => "Sony ARW RAW",
            ImageFormat::RawFujiRaf => "Fujifilm RAF RAW",
            ImageFormat::RawOlympusOrf => "Olympus ORF RAW",
            ImageFormat::RawPanasonicRw2 => "Panasonic RW2 RAW",
            ImageFormat::RawDng => "Adobe DNG RAW",
            ImageFormat::Avif => "AVIF",
            ImageFormat::Heif => "HEIC/HEIF",
            ImageFormat::JpegXl => "JPEG XL",
            ImageFormat::Hdr => "Radiance HDR",
            ImageFormat::OpenExr => "OpenEXR",
            ImageFormat::Psd => "Photoshop PSD",
            ImageFormat::Ico => "ICO",
            ImageFormat::Tga => "TGA",
            ImageFormat::Svg => "SVG",
            ImageFormat::Unknown => "Unknown",
        }
    }

    pub fn is_raw(&self) -> bool {
        matches!(
            self,
            ImageFormat::RawCanonCr2
                | ImageFormat::RawCanonCr3
                | ImageFormat::RawNikonNef
                | ImageFormat::RawSonyArw
                | ImageFormat::RawFujiRaf
                | ImageFormat::RawOlympusOrf
                | ImageFormat::RawPanasonicRw2
                | ImageFormat::RawDng
        )
    }
}

pub fn detect_format(bytes: &[u8]) -> ImageFormat {
    if bytes.len() < 4 {
        return ImageFormat::Unknown;
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return ImageFormat::Jpeg;
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return ImageFormat::Png;
    }

    // GIF: GIF87a or GIF89a
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return ImageFormat::Gif;
    }

    // BMP: 'BM'
    if bytes.starts_with(b"BM") {
        return ImageFormat::Bmp;
    }

    // WebP: RIFF .... WEBP
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return ImageFormat::WebP;
    }

    // Radiance HDR: #?RADIANCE or #?RGBE
    if bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE") {
        return ImageFormat::Hdr;
    }

    // OpenEXR: 76 2F 31 01
    if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
        return ImageFormat::OpenExr;
    }

    // Photoshop PSD: 8BPS
    if bytes.starts_with(b"8BPS") {
        return ImageFormat::Psd;
    }

    // ICO: 00 00 01 00
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return ImageFormat::Ico;
    }

    // Fujifilm RAF: FUJIFILMCCD-RAW
    if bytes.starts_with(b"FUJIFILM") {
        return ImageFormat::RawFujiRaf;
    }

    // Canon CR3 / ISOBMFF formats (ftypcrx, ftypavif, ftypheic)
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"crx " {
            return ImageFormat::RawCanonCr3;
        } else if brand == b"avif" || brand == b"avis" {
            return ImageFormat::Avif;
        } else if brand == b"heic" || brand == b"heix" || brand == b"mif1" || brand == b"msf1" {
            return ImageFormat::Heif;
        }
    }

    // JPEG XL: direct codestream FF 0A or ISOBMFF container
    if bytes.starts_with(&[0xFF, 0x0A]) {
        return ImageFormat::JpegXl;
    }
    if bytes.len() >= 12 && bytes.starts_with(&[0x00, 0x00, 0x00, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A]) {
        return ImageFormat::JpegXl;
    }

    // TIFF and TIFF-based RAW (Canon CR2, Nikon NEF, Sony ARW, Olympus ORF, Panasonic RW2, DNG)
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") || bytes.starts_with(b"IIRO") || bytes.starts_with(b"IIRS") {
        // Canon CR2 has magic bytes at offset 8: 'CR\x02\x00'
        if bytes.len() >= 12 && &bytes[8..12] == b"CR\x02\x00" {
            return ImageFormat::RawCanonCr2;
        }
        // Olympus ORF has magic at offset 2: 'IIRO'
        if bytes.starts_with(b"IIRO") {
            return ImageFormat::RawOlympusOrf;
        }
        // Panasonic RW2 has magic at offset 8: 'IIU\0'
        if bytes.len() >= 12 && &bytes[8..12] == b"IIU\0" {
            return ImageFormat::RawPanasonicRw2;
        }
        // Default TIFF or other TIFF-derived RAW
        return ImageFormat::Tiff;
    }

    // SVG: <?xml ... <svg or simply <svg
    let preview_len = bytes.len().min(512);
    let sample = &bytes[..preview_len];
    if let Ok(text) = std::str::from_utf8(sample) {
        let trimmed = text.trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && text.contains("<svg")) {
            return ImageFormat::Svg;
        }
    }

    ImageFormat::Unknown
}

pub fn detect_format_with_path(bytes: &[u8], path: Option<&std::path::Path>) -> ImageFormat {
    let fmt = detect_format(bytes);

    if let Some(p) = path {
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_ascii_lowercase();
            match ext_lower.as_str() {
                "cr2" => return ImageFormat::RawCanonCr2,
                "cr3" => return ImageFormat::RawCanonCr3,
                "nef" => return ImageFormat::RawNikonNef,
                "arw" => return ImageFormat::RawSonyArw,
                "raf" => return ImageFormat::RawFujiRaf,
                "orf" => return ImageFormat::RawOlympusOrf,
                "rw2" => return ImageFormat::RawPanasonicRw2,
                "dng" => return ImageFormat::RawDng,
                _ => {}
            }
        }
    }

    if fmt == ImageFormat::Unknown {
        if let Some(p) = path {
            if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                match ext_lower.as_str() {
                    "jpg" | "jpeg" => return ImageFormat::Jpeg,
                    "png" => return ImageFormat::Png,
                    "gif" => return ImageFormat::Gif,
                    "webp" => return ImageFormat::WebP,
                    "bmp" => return ImageFormat::Bmp,
                    "tif" | "tiff" => return ImageFormat::Tiff,
                    "hdr" => return ImageFormat::Hdr,
                    "exr" => return ImageFormat::OpenExr,
                    "psd" => return ImageFormat::Psd,
                    "svg" => return ImageFormat::Svg,
                    "ico" => return ImageFormat::Ico,
                    "tga" => return ImageFormat::Tga,
                    _ => {}
                }
            }
        }
    }

    fmt
}
