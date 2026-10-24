use super::jpeg::DecodedImage;
use std::time::Instant;

pub fn decode_hdr(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    let img = image::load_from_memory_with_format(data, image::ImageFormat::Hdr)
        .map_err(|e| format!("HDR decode error: {}", e))?;

    let rgb32f = img.to_rgb32f();
    let (width, height) = rgb32f.dimensions();

    // Preserve full linear dynamic range using half-float (f16) RGBA
    let mut rgba = Vec::with_capacity((width * height * 8) as usize);

    for pixel in rgb32f.pixels() {
        let r = half::f16::from_f32(pixel[0]);
        let g = half::f16::from_f32(pixel[1]);
        let b = half::f16::from_f32(pixel[2]);
        let a = half::f16::from_f32(1.0);

        rgba.extend_from_slice(&r.to_le_bytes());
        rgba.extend_from_slice(&g.to_le_bytes());
        rgba.extend_from_slice(&b.to_le_bytes());
        rgba.extend_from_slice(&a.to_le_bytes());
    }

    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width,
        height,
        rgba,
        decode_duration_us: duration,
        orientation: 1,
        is_hdr: true,
    })
}

pub fn decode_exr(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    let img = image::load_from_memory_with_format(data, image::ImageFormat::OpenExr)
        .map_err(|e| format!("OpenEXR decode error: {}", e))?;

    let rgb32f = img.to_rgb32f();
    let (width, height) = rgb32f.dimensions();

    // Preserve full linear dynamic range using half-float (f16) RGBA
    let mut rgba = Vec::with_capacity((width * height * 8) as usize);

    for pixel in rgb32f.pixels() {
        let r = half::f16::from_f32(pixel[0]);
        let g = half::f16::from_f32(pixel[1]);
        let b = half::f16::from_f32(pixel[2]);
        let a = half::f16::from_f32(1.0);

        rgba.extend_from_slice(&r.to_le_bytes());
        rgba.extend_from_slice(&g.to_le_bytes());
        rgba.extend_from_slice(&b.to_le_bytes());
        rgba.extend_from_slice(&a.to_le_bytes());
    }

    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width,
        height,
        rgba,
        decode_duration_us: duration,
        orientation: 1,
        is_hdr: true,
    })
}
