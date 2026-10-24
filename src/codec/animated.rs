use super::jpeg::DecodedImage;
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, ImageDecoder};
use std::io::Cursor;
use std::time::Instant;

pub struct AnimatedFrame {
    pub rgba: Vec<u8>,
    pub delay_ms: u32,
}

pub struct AnimatedImage {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<AnimatedFrame>,
}

pub fn decode_gif(data: &[u8]) -> Result<(DecodedImage, Option<AnimatedImage>), String> {
    let start = Instant::now();

    let decoder = GifDecoder::new(Cursor::new(data))
        .map_err(|e| format!("GIF init error: {}", e))?;

    let (width, height) = decoder.dimensions();

    let frames_res = decoder.into_frames().collect_frames();
    match frames_res {
        Ok(raw_frames) if !raw_frames.is_empty() => {
            let mut anim_frames = Vec::with_capacity(raw_frames.len());
            for f in raw_frames {
                let (num, denom) = f.delay().numer_denom_ms();
                let delay = if denom > 0 { num / denom } else { 100 };
                let delay_ms = delay.max(10); // Minimum 10ms frame delay
                anim_frames.push(AnimatedFrame {
                    rgba: f.into_buffer().into_raw(),
                    delay_ms,
                });
            }

            let first_frame_rgba = anim_frames[0].rgba.clone();
            let duration = start.elapsed().as_micros();

            let first_image = DecodedImage {
                width,
                height,
                rgba: first_frame_rgba,
                decode_duration_us: duration,
                orientation: 1,
            };

            let anim = if anim_frames.len() > 1 {
                Some(AnimatedImage {
                    width,
                    height,
                    frames: anim_frames,
                })
            } else {
                None
            };

            Ok((first_image, anim))
        }
        _ => {
            // Fallback: static image decode
            let img = image::load_from_memory_with_format(data, image::ImageFormat::Gif)
                .map_err(|e| format!("GIF fallback decode error: {}", e))?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let duration = start.elapsed().as_micros();
            Ok((
                DecodedImage {
                    width: w,
                    height: h,
                    rgba: rgba.into_raw(),
                    decode_duration_us: duration,
                    orientation: 1,
                },
                None,
            ))
        }
    }
}

pub fn decode_webp(data: &[u8]) -> Result<(DecodedImage, Option<AnimatedImage>), String> {
    let start = Instant::now();

    // Check if animated WebP
    if let Ok(decoder) = WebPDecoder::new(Cursor::new(data)) {
        if decoder.has_animation() {
            let (width, height) = decoder.dimensions();
            if let Ok(raw_frames) = decoder.into_frames().collect_frames() {
                if !raw_frames.is_empty() {
                    let mut anim_frames = Vec::with_capacity(raw_frames.len());
                    for f in raw_frames {
                        let (num, denom) = f.delay().numer_denom_ms();
                        let delay = if denom > 0 { num / denom } else { 100 };
                        let delay_ms = delay.max(10);
                        anim_frames.push(AnimatedFrame {
                            rgba: f.into_buffer().into_raw(),
                            delay_ms,
                        });
                    }

                    let first_rgba = anim_frames[0].rgba.clone();
                    let duration = start.elapsed().as_micros();

                    return Ok((
                        DecodedImage {
                            width,
                            height,
                            rgba: first_rgba,
                            decode_duration_us: duration,
                            orientation: 1,
                        },
                        Some(AnimatedImage {
                            width,
                            height,
                            frames: anim_frames,
                        }),
                    ));
                }
            }
        }
    }

    // Static WebP
    let img = image::load_from_memory_with_format(data, image::ImageFormat::WebP)
        .map_err(|e| format!("WebP decode error: {}", e))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let duration = start.elapsed().as_micros();

    Ok((
        DecodedImage {
            width: w,
            height: h,
            rgba: rgba.into_raw(),
            decode_duration_us: duration,
            orientation: 1,
        },
        None,
    ))
}
