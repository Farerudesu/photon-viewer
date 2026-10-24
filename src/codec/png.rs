use super::jpeg::DecodedImage;
use std::io::Cursor;
use std::time::Instant;

pub fn decode_png(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    // Fast path: png crate with transform to RGBA8
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);

    match decoder.read_info() {
        Ok(mut reader) => {
            let info = reader.info().clone();
            let buf_size = reader
                .output_buffer_size()
                .unwrap_or((info.width * info.height * 4) as usize);
            let mut buf = vec![0u8; buf_size];
            match reader.next_frame(&mut buf) {
                Ok(output_info) => {
                    let width = output_info.width;
                    let height = output_info.height;
                    buf.truncate(output_info.buffer_size());

                    let rgba = match info.color_type {
                        png::ColorType::Rgba => buf,
                        png::ColorType::Rgb => {
                            let mut rgba_buf = Vec::with_capacity((width * height * 4) as usize);
                            for chunk in buf.chunks_exact(3) {
                                rgba_buf.push(chunk[0]);
                                rgba_buf.push(chunk[1]);
                                rgba_buf.push(chunk[2]);
                                rgba_buf.push(255);
                            }
                            rgba_buf
                        }
                        png::ColorType::GrayscaleAlpha => {
                            let mut rgba_buf = Vec::with_capacity((width * height * 4) as usize);
                            for chunk in buf.chunks_exact(2) {
                                rgba_buf.push(chunk[0]);
                                rgba_buf.push(chunk[0]);
                                rgba_buf.push(chunk[0]);
                                rgba_buf.push(chunk[1]);
                            }
                            rgba_buf
                        }
                        png::ColorType::Grayscale => {
                            let mut rgba_buf = Vec::with_capacity((width * height * 4) as usize);
                            for &gray in &buf {
                                rgba_buf.push(gray);
                                rgba_buf.push(gray);
                                rgba_buf.push(gray);
                                rgba_buf.push(255);
                            }
                            rgba_buf
                        }
                        png::ColorType::Indexed => {
                            let img = image::load_from_memory_with_format(data, image::ImageFormat::Png)
                                .map_err(|e| format!("PNG decode error: {}", e))?;
                            img.to_rgba8().into_raw()
                        }
                    };

                    let duration = start.elapsed().as_micros();
                    Ok(DecodedImage {
                        width,
                        height,
                        rgba,
                        decode_duration_us: duration,
                        orientation: 1,
                    })
                }
                Err(e) => {
                    log::warn!("png frame read failed ({:?}), trying image crate fallback", e);
                    decode_png_fallback(data, start)
                }
            }
        }
        Err(e) => {
            log::warn!("png header read failed ({:?}), trying image crate fallback", e);
            decode_png_fallback(data, start)
        }
    }
}

fn decode_png_fallback(data: &[u8], start: Instant) -> Result<DecodedImage, String> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Png)
        .map_err(|e| format!("PNG fallback decode error: {}", e))?;
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
