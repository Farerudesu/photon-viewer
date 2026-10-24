use image::{ImageBuffer, Rgb, Rgba};
use photon_viewer::codec::animated::{decode_gif, decode_webp};
use photon_viewer::codec::detector::{detect_format, ImageFormat};
use photon_viewer::codec::hdr::decode_hdr;
use photon_viewer::codec::svg::decode_svg;
use photon_viewer::codec::tiff::decode_tiff;
use std::io::Cursor;

#[test]
fn test_svg_vector_rasterization() {
    let svg_data = br##"
    <svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">
        <rect width="200" height="100" fill="#1e90ff"/>
        <circle cx="100" cy="50" r="40" fill="#ffffff"/>
    </svg>
    "##;

    assert_eq!(detect_format(svg_data), ImageFormat::Svg);

    let decoded = decode_svg(svg_data).expect("Failed to rasterize SVG");
    assert!(decoded.width >= 200);
    assert!(decoded.height >= 100);
    assert_eq!(decoded.rgba.len(), (decoded.width * decoded.height * 4) as usize);
}

#[test]
fn test_webp_decoding() {
    let width = 64;
    let height = 64;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba([255, 128, 0, 255]));

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::WebP)
        .expect("Failed to encode WebP");

    assert_eq!(detect_format(&encoded), ImageFormat::WebP);

    let (decoded, _) = decode_webp(&encoded).expect("Failed to decode WebP");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
    assert_eq!(decoded.rgba.len(), (width * height * 4) as usize);
}

#[test]
fn test_gif_decoding() {
    let width = 32;
    let height = 32;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba([0, 200, 100, 255]));

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Gif)
        .expect("Failed to encode GIF");

    assert_eq!(detect_format(&encoded), ImageFormat::Gif);

    let (decoded, _) = decode_gif(&encoded).expect("Failed to decode GIF");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
}

#[test]
fn test_bmp_decoding() {
    let width = 32;
    let height = 32;
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgb([100, 150, 200]));

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Bmp)
        .expect("Failed to encode BMP");

    assert_eq!(detect_format(&encoded), ImageFormat::Bmp);
}

#[test]
fn test_tiff_decoding() {
    let width = 32;
    let height = 32;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba([50, 100, 150, 255]));

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Tiff)
        .expect("Failed to encode TIFF");

    assert_eq!(detect_format(&encoded), ImageFormat::Tiff);

    let decoded = decode_tiff(&encoded).expect("Failed to decode TIFF");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
}

#[test]
fn test_hdr_rgbe_decoding() {
    let width = 16;
    let height = 16;
    let img: ImageBuffer<image::Rgb<f32>, Vec<f32>> = ImageBuffer::from_pixel(width, height, image::Rgb([1.5, 0.8, 0.2]));

    let mut encoded = Vec::new();
    img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Hdr)
        .expect("Failed to encode HDR");

    assert_eq!(detect_format(&encoded), ImageFormat::Hdr);

    let decoded = decode_hdr(&encoded).expect("Failed to decode HDR");
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
    assert!(decoded.is_hdr);
    assert_eq!(decoded.rgba.len(), (width * height * 8) as usize);
}
