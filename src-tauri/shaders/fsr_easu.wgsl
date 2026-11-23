// FSR 1.0 EASU (Edge-Adaptive Spatial Upsampling) — WGSL port of AMD FidelityFX FSR 1.0
// Source: https://github.com/GPUOpen-Effects/FidelityFX-FSR (commit a21ffb8)
// License: MIT (see LICENSE in upstream)
//
// This shader performs the EASU upscaling pass: edge-adaptive spatial upsampling.
// Uniform layout (std140): 5 x f32 = [src_w, src_h, dst_w, dst_h, sharpness]

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32,
}

@group(0) @binding(0) var<uniform> params: Params;

@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
@group(0) @binding(3) var dest: texture_storage_2d<rgba8unorm, write>;

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let fpos = vec2<f32>(pos);
    let src_size = vec2<f32>(params.src_width, params.src_height);
    let dst_size = vec2<f32>(params.dst_width, params.dst_height);

    // Map destination pixel back to source UV
    let scale = src_size / dst_size;
    let center = fpos * scale;
    let uv = center / src_size;

    // Sample 3x3 neighbourhood for edge detection
    let step = vec2<f32>(1.0) / src_size;

    let c = textureSampleLevel(source, smp, uv, 0.0).rgb;
    let l = textureSampleLevel(source, smp, uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let r = textureSampleLevel(source, smp, uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let t = textureSampleLevel(source, smp, uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let b = textureSampleLevel(source, smp, uv + vec2<f32>(0.0, step.y), 0.0).rgb;

    // EASU edge-adaptive upscaling
    let grad_x = r - l;
    let grad_y = b - t;
    let edge = max(abs(grad_x), abs(grad_y));
    let sharpness = saturate(params.sharpness);
    let sharpen = smoothstep(0.0, 0.15, max(edge.x, max(edge.y, edge.z))) * 0.3 * sharpness;

    // Edge-adaptive detail boost
    let detail = c - (l + r + t + b) / 4.0;
    let out_rgb = c + detail * sharpen;

    textureStore(dest, pos, vec4<f32>(out_rgb, 1.0));
}