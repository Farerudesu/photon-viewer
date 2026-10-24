use super::jpeg::DecodedImage;
use resvg::tiny_skia;
use resvg::usvg;
use std::time::Instant;

pub fn decode_svg(data: &[u8]) -> Result<DecodedImage, String> {
    let start = Instant::now();

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &opt)
        .map_err(|e| format!("SVG parse error: {}", e))?;

    let size = tree.size();
    let mut width = size.width() as u32;
    let mut height = size.height() as u32;

    width = width.max(16);
    height = height.max(16);

    let max_dim = width.max(height);
    let scale: f32 = if max_dim < 1024 {
        (1024.0f32 / max_dim as f32).clamp(1.0f32, 4.0f32)
    } else {
        1.0f32
    };

    let target_w = (width as f32 * scale).round() as u32;
    let target_h = (height as f32 * scale).round() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
        .ok_or("Failed to allocate SVG render pixmap")?;

    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let duration = start.elapsed().as_micros();

    Ok(DecodedImage {
        width: target_w,
        height: target_h,
        rgba: pixmap.take(),
        decode_duration_us: duration,
        orientation: 1,
        is_hdr: false,
    })
}
