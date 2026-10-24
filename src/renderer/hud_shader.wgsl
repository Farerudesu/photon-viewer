struct HudUniforms {
    viewport_size: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u_hud: HudUniforms;

struct HudVertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct HudVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_hud(model: HudVertexInput) -> HudVertexOutput {
    var out: HudVertexOutput;
    // Map screen pixel coordinates [0..w, 0..h] to NDC [-1..1, 1..-1]
    let ndc_x = (model.position.x / u_hud.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (model.position.y / u_hud.viewport_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = model.color;
    return out;
}

@fragment
fn fs_hud(in: HudVertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
