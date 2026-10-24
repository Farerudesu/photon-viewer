use super::jpeg::DecodedImage;
use std::time::Instant;

pub fn decode_tiff(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    let img = image::load_from_memory_with_format(data, image::ImageFormat::Tiff)
        .map_err(|e| format!("TIFF decode error: {}", e))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width,
        height,
        rgba: rgba.into_raw(),
        decode_duration_us: duration,
        orientation: 1,
    })
}
