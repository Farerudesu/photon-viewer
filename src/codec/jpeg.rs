use std::io::Cursor;
use std::time::Instant;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

#[derive(Clone, Debug)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub decode_duration_us: u128,
    pub orientation: u16,
}

impl DecodedImage {
    #[allow(dead_code)]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>, decode_duration_us: u128) -> Self {
        Self {
            width,
            height,
            rgba,
            decode_duration_us,
            orientation: 1,
        }
    }
}

pub fn decode_jpeg(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    // Fast path: SIMD zune-jpeg with direct RGBA target output
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(data), options);

    match decoder.decode() {
        Ok(pixels) => {
            let (width, height) = decoder
                .dimensions()
                .ok_or("Failed to read JPEG dimensions")?;
            let duration = start.elapsed().as_micros();

            Ok(DecodedImage {
                width: width as u32,
                height: height as u32,
                rgba: pixels,
                decode_duration_us: duration,
                orientation: 1,
            })
        }
        Err(e) => {
            // Fallback path: standard image crate JPEG decoder
            log::warn!("zune-jpeg failed ({:?}), trying image crate fallback", e);
            let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg)
                .map_err(|err| format!("JPEG decode error: {}", err))?;
            let rgba_img = img.to_rgba8();
            let (width, height) = rgba_img.dimensions();
            let duration = start.elapsed().as_micros();

            Ok(DecodedImage {
                width,
                height,
                rgba: rgba_img.into_raw(),
                decode_duration_us: duration,
                orientation: 1,
            })
        }
    }
}
