use image::{ImageBuffer, Rgba};
use photon_viewer::codec::detector::{detect_format, ImageFormat};
use photon_viewer::codec::jpeg::decode_jpeg;
use photon_viewer::codec::png::decode_png;
use photon_viewer::codec::LoadedFile;
use std::io::Cursor;

#[test]
fn test_magic_bytes_detection() {
    // JPEG header
    let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
    assert_eq!(detect_format(&jpeg_bytes), ImageFormat::Jpeg);

    // PNG header
    let png_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
    assert_eq!(detect_format(&png_bytes), ImageFormat::Png);

    // GIF header
    assert_eq!(detect_format(b"GIF89a\x01\x00\x01\x00"), ImageFormat::Gif);

    // BMP header
    assert_eq!(detect_format(b"BM\x36\x00\x00\x00"), ImageFormat::Bmp);

    // WebP header
    assert_eq!(
        detect_format(b"RIFF\x20\x00\x00\x00WEBPVP8 "),
        ImageFormat::WebP
    );

    // Canon CR2 header
    let mut cr2_bytes = vec![0u8; 16];
    cr2_bytes[0..4].copy_from_slice(b"II*\0");
    cr2_bytes[8..12].copy_from_slice(b"CR\x02\x00");
    assert_eq!(detect_format(&cr2_bytes), ImageFormat::RawCanonCr2);

    // Unknown
    assert_eq!(detect_format(&[0x01, 0x02, 0x03, 0x04]), ImageFormat::Unknown);
}

#[test]
fn test_decode_png_in_memory() {
    let width = 64;
    let height = 32;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([(x * 4) as u8, (y * 8) as u8, 128, 255]);
    }

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png)
        .expect("Failed to encode PNG");

    let decoded = decode_png(&encoded).expect("Failed to decode PNG");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
    assert_eq!(decoded.rgba.len(), (width * height * 4) as usize);

    // Verify first pixel
    assert_eq!(decoded.rgba[0], 0);
    assert_eq!(decoded.rgba[1], 0);
    assert_eq!(decoded.rgba[2], 128);
    assert_eq!(decoded.rgba[3], 255);
}

#[test]
fn test_decode_jpeg_in_memory() {
    let width = 64;
    let height = 64;
    let mut img: ImageBuffer<image::Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x * 3) as u8, (y * 3) as u8, 200]);
    }

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Jpeg)
        .expect("Failed to encode JPEG");

    let decoded = decode_jpeg(&encoded).expect("Failed to decode JPEG");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
    assert_eq!(decoded.rgba.len(), (width * height * 4) as usize);
    // Alpha channel must be 255
    assert_eq!(decoded.rgba[3], 255);
    assert!(decoded.decode_duration_us > 0);
}

#[test]
fn test_wrong_extension_handling() {
    let width = 32;
    let height = 32;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba([255, 0, 0, 255]));

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("disguised_image.txt"); // Intentionally .txt extension!

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png)
        .expect("Failed to encode PNG");
    std::fs::write(&temp_path, &encoded).expect("Failed to write temp file");

    let loaded = LoadedFile::open(&temp_path).expect("Failed to load file");
    assert_eq!(loaded.format, ImageFormat::Png); // Detected as PNG despite .txt extension!

    let decoded = loaded.decode().expect("Failed to decode file");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);

    let _ = std::fs::remove_file(temp_path);
}
