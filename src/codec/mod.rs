pub mod animated;
pub mod detector;
pub mod exif;
pub mod hdr;
pub mod jpeg;
pub mod png;
pub mod psd;
pub mod raw;
pub mod svg;
pub mod tiff;

#[allow(unused_imports)]
pub use animated::{AnimatedFrame, AnimatedImage};
#[allow(unused_imports)]
pub use detector::{detect_format, ImageFormat};
pub use jpeg::DecodedImage;

use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use std::time::Instant;

#[allow(dead_code)]
pub struct LoadedFile {
    pub mmap: Mmap,
    pub format: ImageFormat,
    pub file_size: u64,
    pub orientation: u16,
}

impl LoadedFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let metadata = file.metadata().map_err(|e| format!("Failed to get metadata: {}", e))?;
        let file_size = metadata.len();

        if file_size == 0 {
            return Err("File is empty (0 bytes)".to_string());
        }

        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| format!("Memory mapping failed: {}", e))?
        };

        let format = detector::detect_format_with_path(&mmap, Some(path.as_ref()));
        let orientation = exif::read_exif_orientation(&mmap);

        Ok(LoadedFile {
            mmap,
            format,
            file_size,
            orientation,
        })
    }

    pub fn decode(&self) -> Result<DecodedImage, String> {
        let (img, _) = self.decode_with_animation()?;
        Ok(img)
    }

    pub fn decode_raw(&self) -> Result<(DecodedImage, bool), String> {
        // Fast path: attempt embedded JPEG preview extraction first
        if let Some(mut preview) = raw::extract_embedded_jpeg(&self.mmap) {
            preview.orientation = self.orientation;
            return Ok((preview, true));
        }

        // Full Bayer demosaicing fallback
        let mut full = raw::decode_full_raw(&self.mmap)?;
        full.orientation = self.orientation;
        Ok((full, false))
    }

    pub fn decode_with_animation(&self) -> Result<(DecodedImage, Option<AnimatedImage>), String> {
        let (mut img, anim) = self.decode_with_animation_inner()?;
        img.orientation = self.orientation;
        Ok((img, anim))
    }

    fn decode_with_animation_inner(&self) -> Result<(DecodedImage, Option<AnimatedImage>), String> {
        match self.format {
            ImageFormat::Jpeg => {
                let img = jpeg::decode_jpeg(&self.mmap)?;
                Ok((img, None))
            }
            ImageFormat::Png => {
                let img = png::decode_png(&self.mmap)?;
                Ok((img, None))
            }
            ImageFormat::Gif => animated::decode_gif(&self.mmap),
            ImageFormat::WebP => animated::decode_webp(&self.mmap),
            ImageFormat::Bmp => {
                let img = decode_with_image_crate(&self.mmap, image::ImageFormat::Bmp, "BMP")?;
                Ok((img, None))
            }
            ImageFormat::Tiff => {
                let img = tiff::decode_tiff(&self.mmap)?;
                Ok((img, None))
            }
            ImageFormat::Ico => {
                let img = decode_with_image_crate(&self.mmap, image::ImageFormat::Ico, "ICO")?;
                Ok((img, None))
            }
            ImageFormat::Tga => {
                let img = decode_with_image_crate(&self.mmap, image::ImageFormat::Tga, "TGA")?;
                Ok((img, None))
            }
            ImageFormat::Hdr => {
                let img = hdr::decode_hdr(&self.mmap)?;
                Ok((img, None))
            }
            ImageFormat::OpenExr => {
                let img = hdr::decode_exr(&self.mmap)?;
                Ok((img, None))
            }
            ImageFormat::Psd => {
                let img = psd::decode_psd(&self.mmap)?;
                Ok((img, None))
            }
            ImageFormat::Svg => {
                let img = svg::decode_svg(&self.mmap)?;
                Ok((img, None))
            }
            fmt if fmt.is_raw() => {
                let (img, _) = self.decode_raw()?;
                Ok((img, None))
            }
            _ => {
                // Universal fallback for PNM, DDS, QOI, etc.
                let start = Instant::now();
                match image::load_from_memory(&self.mmap) {
                    Ok(dyn_img) => {
                        let rgba = dyn_img.to_rgba8();
                        let (width, height) = rgba.dimensions();
                        let duration = start.elapsed().as_micros();
                        Ok((
                            DecodedImage {
                                width,
                                height,
                                rgba: rgba.into_raw(),
                                decode_duration_us: duration,
                                orientation: 1,
                            },
                            None,
                        ))
                    }
                    Err(e) => Err(format!(
                        "Unsupported or undecodable image format {:?}: {}",
                        self.format, e
                    )),
                }
            }
        }
    }
}

fn decode_with_image_crate(
    data: &[u8],
    format: image::ImageFormat,
    name: &str,
) -> Result<DecodedImage, String> {
    let start = Instant::now();
    let img = image::load_from_memory_with_format(data, format)
        .map_err(|e| format!("{} decode error: {}", name, e))?;
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
