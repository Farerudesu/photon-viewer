struct Uniforms {
    transform: mat4x4<f32>,
    viewport_size: vec2<f32>,
    image_size: vec2<f32>,
    zoom: f32,
    flags: u32,       // bit 0: checkerboard, bit 1: pixel grid
    filter_mode: u32, // 0: Bilinear, 1: Bicubic (Catmull-Rom), 2: Mitchell-Netravali, 3: Nearest, 4: Lanczos3
    tonemap_mode: u32,// 0: None, 1: Reinhard, 2: ACES Filmic
    exposure: f32,    // exposure multiplier: 2.0^EV
    color_space: u32, // 0: sRGB, 1: Display P3, 2: Adobe RGB
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;
@group(0) @binding(2) var s_linear: sampler;
@group(0) @binding(3) var s_nearest: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) frag_coord: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = u.transform * vec4<f32>(model.position, 0.0, 1.0);
    out.tex_coords = model.tex_coords;
    out.frag_coord = (out.clip_position.xy / out.clip_position.w * 0.5 + 0.5) * u.viewport_size;
    return out;
}

// ----------------------------------------------------------------------------
// Resampling Filters
// ----------------------------------------------------------------------------

// Catmull-Rom spline 4-tap evaluation
fn cubic_weight_catmull_rom(x: f32) -> f32 {
    let ax = abs(x);
    if (ax <= 1.0) {
        return 1.5 * ax * ax * ax - 2.5 * ax * ax + 1.0;
    } else if (ax <= 2.0) {
        return -0.5 * ax * ax * ax + 2.5 * ax * ax - 4.0 * ax + 2.0;
    }
    return 0.0;
}

// Mitchell-Netravali cubic spline (B=1/3, C=1/3)
fn cubic_weight_mitchell(x: f32) -> f32 {
    let ax = abs(x);
    let b = 1.0 / 3.0;
    let c = 1.0 / 3.0;
    if (ax < 1.0) {
        return ((12.0 - 9.0 * b - 6.0 * c) * ax * ax * ax + (-18.0 + 12.0 * b + 6.0 * c) * ax * ax + (6.0 - 2.0 * b)) / 6.0;
    } else if (ax < 2.0) {
        return ((-b - 6.0 * c) * ax * ax * ax + (6.0 * b + 30.0 * c) * ax * ax + (-12.0 * b - 48.0 * c) * ax + (8.0 * b + 24.0 * c)) / 6.0;
    }
    return 0.0;
}

// Sinc function
fn sinc(x: f32) -> f32 {
    if (abs(x) < 0.0001) {
        return 1.0;
    }
    let pi_x = 3.14159265359 * x;
    return sin(pi_x) / pi_x;
}

// Lanczos-3 kernel window
fn lanczos3_weight(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 3.0) {
        return sinc(ax) * sinc(ax / 3.0);
    }
    return 0.0;
}

// Sample texture with Catmull-Rom bicubic reconstruction
fn sample_bicubic(uv: vec2<f32>) -> vec4<f32> {
    let tex_size = u.image_size;
    let inv_tex_size = 1.0 / tex_size;
    let coord = uv * tex_size - 0.5;
    let f = fract(coord);
    let base = floor(coord);

    var total_color = vec4<f32>(0.0);
    var total_weight = 0.0;

    for (var dy: i32 = -1; dy <= 2; dy = dy + 1) {
        let wy = cubic_weight_catmull_rom(f.y - f32(dy));
        for (var dx: i32 = -1; dx <= 2; dx = dx + 1) {
            let wx = cubic_weight_catmull_rom(f.x - f32(dx));
            let w = wx * wy;

            let sample_uv = (base + vec2<f32>(f32(dx), f32(dy)) + 0.5) * inv_tex_size;
            total_color = total_color + textureSample(t_diffuse, s_linear, sample_uv) * w;
            total_weight = total_weight + w;
        }
    }

    return total_color / max(total_weight, 0.0001);
}

// Sample texture with Mitchell-Netravali filter
fn sample_mitchell(uv: vec2<f32>) -> vec4<f32> {
    let tex_size = u.image_size;
    let inv_tex_size = 1.0 / tex_size;
    let coord = uv * tex_size - 0.5;
    let f = fract(coord);
    let base = floor(coord);

    var total_color = vec4<f32>(0.0);
    var total_weight = 0.0;

    for (var dy: i32 = -1; dy <= 2; dy = dy + 1) {
        let wy = cubic_weight_mitchell(f.y - f32(dy));
        for (var dx: i32 = -1; dx <= 2; dx = dx + 1) {
            let wx = cubic_weight_mitchell(f.x - f32(dx));
            let w = wx * wy;

            let sample_uv = (base + vec2<f32>(f32(dx), f32(dy)) + 0.5) * inv_tex_size;
            total_color = total_color + textureSample(t_diffuse, s_linear, sample_uv) * w;
            total_weight = total_weight + w;
        }
    }

    return total_color / max(total_weight, 0.0001);
}

// Sample texture with Lanczos-3 filter (3x3 window around center)
fn sample_lanczos3(uv: vec2<f32>) -> vec4<f32> {
    let tex_size = u.image_size;
    let inv_tex_size = 1.0 / tex_size;
    let coord = uv * tex_size - 0.5;
    let f = fract(coord);
    let base = floor(coord);

    var total_color = vec4<f32>(0.0);
    var total_weight = 0.0;

    for (var dy: i32 = -2; dy <= 3; dy = dy + 1) {
        let wy = lanczos3_weight(f.y - f32(dy));
        for (var dx: i32 = -2; dx <= 3; dx = dx + 1) {
            let wx = lanczos3_weight(f.x - f32(dx));
            let w = wx * wy;

            let sample_uv = (base + vec2<f32>(f32(dx), f32(dy)) + 0.5) * inv_tex_size;
            total_color = total_color + textureSample(t_diffuse, s_linear, sample_uv) * w;
            total_weight = total_weight + w;
        }
    }

    return total_color / max(total_weight, 0.0001);
}

// ----------------------------------------------------------------------------
// Tone Mapping Operators
// ----------------------------------------------------------------------------

// Extended Reinhard tone mapping: L_d = L * (1 + L / L_white^2) / (1 + L)
fn tonemap_reinhard(hdr: vec3<f32>) -> vec3<f32> {
    let l_white_sq = 16.0;
    return (hdr * (vec3<f32>(1.0) + hdr / l_white_sq)) / (vec3<f32>(1.0) + hdr);
}

// ACES Filmic Tone Mapping Curve (Narkowicz fit)
fn tonemap_aces(hdr: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((hdr * (a * hdr + b)) / (hdr * (c * hdr + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// ----------------------------------------------------------------------------
// Color Space Conversions
// ----------------------------------------------------------------------------

// Display P3 to standard sRGB conversion (column-major in WGSL)
fn convert_p3_to_srgb(p3: vec3<f32>) -> vec3<f32> {
    let col0 = vec3<f32>(1.2249, -0.0420, -0.0197);
    let col1 = vec3<f32>(-0.2247, 1.0419, -0.0786);
    let col2 = vec3<f32>(0.0000, 0.0000, 1.0979);
    let m = mat3x3<f32>(col0, col1, col2);
    return clamp(m * p3, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Adobe RGB (1998) to standard sRGB conversion (column-major in WGSL)
fn convert_adobe_to_srgb(adobe: vec3<f32>) -> vec3<f32> {
    let col0 = vec3<f32>(1.3983, 0.0000, 0.0000);
    let col1 = vec3<f32>(-0.3983, 1.0000, -0.0429);
    let col2 = vec3<f32>(0.0000, 0.0000, 1.0429);
    let m = mat3x3<f32>(col0, col1, col2);
    return clamp(m * adobe, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Texture resampling based on active filter mode
    var color: vec4<f32>;

    switch (u.filter_mode) {
        case 1u: {
            // Bicubic (Catmull-Rom spline)
            color = sample_bicubic(in.tex_coords);
        }
        case 2u: {
            // Mitchell-Netravali
            color = sample_mitchell(in.tex_coords);
        }
        case 3u: {
            // Nearest-Neighbor
            color = textureSample(t_diffuse, s_nearest, in.tex_coords);
        }
        case 4u: {
            // Lanczos-3
            color = sample_lanczos3(in.tex_coords);
        }
        default: {
            // 0u: Default bilinear (or nearest when heavily zoomed > 300% if not explicitly set)
            if (u.zoom > 3.0) {
                color = textureSample(t_diffuse, s_nearest, in.tex_coords);
            } else {
                color = textureSample(t_diffuse, s_linear, in.tex_coords);
            }
        }
    }

    // 2. Exposure compensation
    var rgb = color.rgb * u.exposure;

    // 3. Color Space Conversion
    if (u.color_space == 1u) {
        rgb = convert_p3_to_srgb(rgb);
    } else if (u.color_space == 2u) {
        rgb = convert_adobe_to_srgb(rgb);
    }

    // 4. Tone Mapping
    if (u.tonemap_mode == 1u) {
        rgb = tonemap_reinhard(rgb);
    } else if (u.tonemap_mode == 2u) {
        rgb = tonemap_aces(rgb);
    }

    color = vec4<f32>(rgb, color.a);

    // 5. Pixel grid outline at high zoom (>= 500%)
    if ((u.flags & 2u) != 0u && u.zoom >= 5.0) {
        let pixel_coord = in.tex_coords * u.image_size;
        let frac_pixel = fract(pixel_coord);
        let border_dist = min(frac_pixel, 1.0 - frac_pixel);
        let min_border = min(border_dist.x, border_dist.y);
        let border_width = 1.0 / u.zoom;
        if (min_border < border_width) {
            let grid_color = vec4<f32>(0.2, 0.2, 0.25, 0.35);
            color = mix(color, grid_color, grid_color.a);
        }
    }

    // 6. Alpha checkerboard background for transparent pixels
    if ((u.flags & 1u) != 0u && color.a < 0.999) {
        let check_size = 12.0;
        let check_pos = floor(in.frag_coord / check_size);
        let is_odd = (check_pos.x + check_pos.y) % 2.0;
        let bg_color = select(vec3<f32>(0.15, 0.15, 0.16), vec3<f32>(0.22, 0.22, 0.24), is_odd == 0.0);
        let final_rgb = mix(bg_color, color.rgb, color.a);
        return vec4<f32>(final_rgb, 1.0);
    }

    return color;
}
