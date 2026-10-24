use super::jpeg::DecodedImage;
use std::time::Instant;

pub fn decode_hdr(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    let img = image::load_from_memory_with_format(data, image::ImageFormat::Hdr)
        .map_err(|e| format!("HDR decode error: {}", e))?;

    let rgb32f = img.to_rgb32f();
    let (width, height) = rgb32f.dimensions();

    // Fast tone mapping: Reinhard tone curve + gamma to sRGB
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    let exposure = 1.0;

    for pixel in rgb32f.pixels() {
        let r = pixel[0] * exposure;
        let g = pixel[1] * exposure;
        let b = pixel[2] * exposure;

        // Reinhard: c / (1.0 + c)
        let tm_r = r / (1.0 + r);
        let tm_g = g / (1.0 + g);
        let tm_b = b / (1.0 + b);

        // sRGB gamma ~2.2 approximation: pow(1/2.2)
        let srgb_r = (tm_r.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0) as u8;
        let srgb_g = (tm_g.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0) as u8;
        let srgb_b = (tm_b.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0) as u8;

        rgba.push(srgb_r);
        rgba.push(srgb_g);
        rgba.push(srgb_b);
        rgba.push(255);
    }

    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width,
        height,
        rgba,
        decode_duration_us: duration,
        orientation: 1,
    })
}

pub fn decode_exr(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    let img = image::load_from_memory_with_format(data, image::ImageFormat::OpenExr)
        .map_err(|e| format!("OpenEXR decode error: {}", e))?;

    let rgb32f = img.to_rgb32f();
    let (width, height) = rgb32f.dimensions();

    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    let exposure = 1.0;

    for pixel in rgb32f.pixels() {
        let r = pixel[0] * exposure;
        let g = pixel[1] * exposure;
        let b = pixel[2] * exposure;

        // ACES / Reinhard hybrid tone mapping
        let tm_r = r / (1.0 + r);
        let tm_g = g / (1.0 + g);
        let tm_b = b / (1.0 + b);

        let srgb_r = (tm_r.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0) as u8;
        let srgb_g = (tm_g.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0) as u8;
        let srgb_b = (tm_b.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0) as u8;

        rgba.push(srgb_r);
        rgba.push(srgb_g);
        rgba.push(srgb_b);
        rgba.push(255);
    }

    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width,
        height,
        rgba,
        decode_duration_us: duration,
        orientation: 1,
    })
}
