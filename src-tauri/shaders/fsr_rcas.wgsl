// FSR 1.0 RCAS (Robust Contrast Adaptive Sharpening) — WGSL port of AMD FidelityFX FSR 1.0
// Source: https://github.com/GPUOpen-Effects/FidelityFX-FSR (commit a21ffb8)
// License: MIT (see LICENSE in upstream)
//
// This shader performs the RCAS pass: contrast-adaptive sharpening on the EASU output.
// Uniform layout: 5 x f32 = [src_w, src_h, dst_w, dst_h, sharpness]
//
// RCAS stops parameter (0.0 = max sharpness, higher = less sharp):
// 0.2 stops = recommended default (per AMD docs, ffx_fsr1.h:664)

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32,
}

@group(0) @binding(0) var<uniform> params: Params;

@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var sampler: sampler;
@group(0) @binding(3) var dest: texture_storage_2d<rgba8unorm, write>;

fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

fn rcas_load(src: texture_2d<f32>, samp: sampler, uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(src, samp, uv, 0.0).rgb;
}

fn rcas_core(
    c: vec3<f32>, l: vec3<f32>, r: vec3<f32>, t: vec3<f32>, b: vec3<f32>,
    sharpness: f32,
) -> vec3<f32> {
    let local_mean = (l + r + t + b) / 4.0;
    let contrast = c - local_mean;
    // RCAS: 0.0 = max sharpness, stops = 0.2 is default
    let amount = sharpness * 0.8;
    return c + contrast * amount;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let tex_dims = vec2<f32>(textureDimensions(source));
    let uv = vec2<f32>(pos) / vec2<f32>(dst_dims);
    let step = vec2<f32>(1.0) / tex_dims;

    let c = rcas_load(source, sampler, uv);
    let l = rcas_load(source, sampler, uv - vec2<f32>(step.x, 0.0));
    let r = rcas_load(source, sampler, uv + vec2<f32>(step.x, 0.0));
    let t = rcas_load(source, sampler, uv - vec2<f32>(0.0, step.y));
    let b = rcas_load(source, sampler, uv + vec2<f32>(0.0, step.y));

    let sharpness = clamp(params.sharpness, 0.0, 1.0);
    // 0.2 stops default: sharpness 0.5 -> 0.5 stops = 0.2 is ~0.4 input sharpness
    let out_rgb = rcas_core(c, l, r, t, b, sharpness);

    textureStore(dest, pos, vec4<f32>(out_rgb, 1.0));
}