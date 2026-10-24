use photon_viewer::codec::detector::{detect_format, detect_format_with_path, ImageFormat};
use photon_viewer::codec::raw::extract_embedded_jpeg;
use photon_viewer::codec::LoadedFile;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Generate a valid minimal 8x8 JPEG byte buffer
fn create_test_jpeg(r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut img = image::RgbImage::new(8, 8);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([r, g, b]);
    }
    let mut buf = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut buf);
    encoder.encode(&img, 8, 8, image::ExtendedColorType::Rgb8).unwrap();
    buf
}

#[test]
fn test_raw_format_detection() {
    // 1. Fuji RAF
    let mut raf_header = b"FUJIFILMCCD-RAW \0\0\0\0".to_vec();
    raf_header.resize(120, 0);
    assert_eq!(detect_format(&raf_header), ImageFormat::RawFujiRaf);

    // 2. Canon CR2
    let mut cr2_header = b"II*\0\x10\0\0\0CR\x02\0".to_vec();
    cr2_header.resize(64, 0);
    assert_eq!(detect_format(&cr2_header), ImageFormat::RawCanonCr2);

    // 3. Canon CR3
    let mut cr3_header = b"\0\0\0\x18ftypcrx \0\0\0\0".to_vec();
    cr3_header.resize(64, 0);
    assert_eq!(detect_format(&cr3_header), ImageFormat::RawCanonCr3);

    // 4. Olympus ORF
    let mut orf_header = b"IIRO\x08\0\0\0".to_vec();
    orf_header.resize(64, 0);
    assert_eq!(detect_format(&orf_header), ImageFormat::RawOlympusOrf);

    // 5. Panasonic RW2
    let mut rw2_header = b"II*\0\x10\0\0\0IIU\0".to_vec();
    rw2_header.resize(64, 0);
    assert_eq!(detect_format(&rw2_header), ImageFormat::RawPanasonicRw2);

    // 6. Path-based disambiguation for TIFF-derived RAW
    let tiff_header = b"II*\0\x08\0\0\0";
    assert_eq!(
        detect_format_with_path(tiff_header, Some(Path::new("DSC_0001.NEF"))),
        ImageFormat::RawNikonNef
    );
    assert_eq!(
        detect_format_with_path(tiff_header, Some(Path::new("ILCE_7M4.ARW"))),
        ImageFormat::RawSonyArw
    );
    assert_eq!(
        detect_format_with_path(tiff_header, Some(Path::new("SAMPLE.DNG"))),
        ImageFormat::RawDng
    );
}

#[test]
fn test_fuji_raf_embedded_preview_extraction() {
    let jpeg_bytes = create_test_jpeg(220, 110, 40);

    // Fuji RAF has:
    // Offset 84: 4-byte BE offset
    // Offset 88: 4-byte BE length
    let mut raf = vec![0u8; 128];
    raf[0..8].copy_from_slice(b"FUJIFILM");

    let offset = 128u32;
    let len = jpeg_bytes.len() as u32;

    raf[84..88].copy_from_slice(&offset.to_be_bytes());
    raf[88..92].copy_from_slice(&len.to_be_bytes());
    raf.extend_from_slice(&jpeg_bytes);

    let extracted = extract_embedded_jpeg(&raf);
    assert!(extracted.is_some(), "Failed to extract Fuji RAF preview");
    let img = extracted.unwrap();
    assert_eq!(img.width, 8);
    assert_eq!(img.height, 8);
    assert_eq!(img.rgba.len(), 8 * 8 * 4);
}

#[test]
fn test_tiff_raw_embedded_preview_extraction() {
    let jpeg_bytes = create_test_jpeg(10, 150, 240);

    // Build minimal TIFF header:
    // 0..4: "II*\0" (Little Endian TIFF)
    // 4..8: IFD offset = 8
    // 8..10: Entry count = 1
    // 10..22: Entry 0 (Tag 0x0201 = JPEGInterchangeFormat, type 4 = LONG, count 1, value = offset)
    // 22..34: Entry 1 (Tag 0x0202 = JPEGInterchangeFormatLength, type 4 = LONG, count 1, value = length)
    // 34..38: Next IFD offset = 0
    let mut tiff = vec![0u8; 64];
    tiff[0..4].copy_from_slice(b"II*\0");
    tiff[4..8].copy_from_slice(&8u32.to_le_bytes()); // IFD at 8
    tiff[8..10].copy_from_slice(&2u16.to_le_bytes()); // 2 entries

    let jpeg_offset = 64u32;
    let jpeg_len = jpeg_bytes.len() as u32;

    // Entry 0: Tag 0x0201
    tiff[10..12].copy_from_slice(&0x0201u16.to_le_bytes());
    tiff[12..14].copy_from_slice(&4u16.to_le_bytes()); // LONG
    tiff[14..18].copy_from_slice(&1u32.to_le_bytes()); // count 1
    tiff[18..22].copy_from_slice(&jpeg_offset.to_le_bytes()); // offset

    // Entry 1: Tag 0x0202
    tiff[22..24].copy_from_slice(&0x0202u16.to_le_bytes());
    tiff[24..26].copy_from_slice(&4u16.to_le_bytes()); // LONG
    tiff[26..30].copy_from_slice(&1u32.to_le_bytes()); // count 1
    tiff[30..34].copy_from_slice(&jpeg_len.to_le_bytes()); // len

    tiff.extend_from_slice(&jpeg_bytes);

    let extracted = extract_embedded_jpeg(&tiff);
    assert!(extracted.is_some(), "Failed to extract TIFF embedded JPEG");
    let img = extracted.unwrap();
    assert_eq!(img.width, 8);
    assert_eq!(img.height, 8);
}

#[test]
fn test_universal_raw_stream_preview_scanner() {
    let jpeg_bytes = create_test_jpeg(50, 200, 50);

    // Wrap the JPEG inside 100KB of arbitrary dummy RAW sensor data
    let mut dummy_raw = vec![0xAB; 40_000];
    dummy_raw.extend_from_slice(&jpeg_bytes);
    dummy_raw.resize(100_000, 0xCD);

    let extracted = extract_embedded_jpeg(&dummy_raw);
    assert!(extracted.is_some(), "Universal stream scanner failed to find JPEG preview");
    let img = extracted.unwrap();
    assert_eq!(img.width, 8);
    assert_eq!(img.height, 8);
}

#[test]
fn test_loaded_file_raw_fast_path() {
    let jpeg_bytes = create_test_jpeg(180, 70, 230);

    // Create a temporary simulated CR2 file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_sample_canon.cr2");

    let mut cr2_data = vec![0u8; 128];
    cr2_data[0..4].copy_from_slice(b"II*\0");
    cr2_data[8..12].copy_from_slice(b"CR\x02\0");
    // Append JPEG preview stream
    cr2_data.extend_from_slice(&jpeg_bytes);
    cr2_data.resize(cr2_data.len() + 1000, 0);

    {
        let mut f = File::create(&temp_file).unwrap();
        f.write_all(&cr2_data).unwrap();
    }

    let loaded = LoadedFile::open(&temp_file).expect("Failed to open simulated CR2");
    assert_eq!(loaded.format, ImageFormat::RawCanonCr2);

    let (decoded, is_preview) = loaded.decode_raw().expect("Failed to decode raw");
    assert!(is_preview, "Expected fast-path preview flag to be true");
    assert_eq!(decoded.width, 8);
    assert_eq!(decoded.height, 8);

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_sony_arw_decode() {
    let path = Path::new(r"C:\Users\Fareru\Downloads\5383671086.arw");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(path).unwrap();
    let decoded = photon_viewer::codec::raw::decode_full_raw(&data).unwrap();
    assert_eq!(decoded.width, 6048);
    assert_eq!(decoded.height, 4024);
    assert_eq!(decoded.rgba.len(), 6048 * 4024 * 4);

    let count = 10000;
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    for chunk in decoded.rgba.chunks_exact(4).take(count) {
        r_sum += chunk[0] as u64;
        g_sum += chunk[1] as u64;
        b_sum += chunk[2] as u64;
    }
    let avg_r = r_sum / count as u64;
    let avg_g = g_sum / count as u64;
    let avg_b = b_sum / count as u64;
    assert!(avg_r < 250 && avg_g < 250 && avg_b < 250, "Image should not be blown out white");
}

