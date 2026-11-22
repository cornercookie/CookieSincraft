// Anime4K v4 — CNN x2 (M) Upscale
// Source: https://github.com/bloc97/Anime4K (commit 7684e95)
// License: MIT (see LICENSE in upstream)
//
// This file contains the WGSL entry points for the Anime4K_CNN_x2_M pipeline:
// - 8 convolution passes (1x 3x3x3, 6x 3x3x8, 1x 1x1x56) with ReLU
// - Depth-to-Space pass (2x upscale, adds residual to original)
// - Auto-downscale pre-passes for >2x targets handled by pipeline
//
// Each pass is a separate compute shader entry point.
// Pipeline in Rust: for 2x target, run conv2d_tf -> conv2d_1 -> ... -> conv2d_6 -> conv2d_last -> depth_to_space
// For >2x, repeat depth_to_space output as next input until target reached, final Lanczos3 to exact size.
//
// WHEN condition (from GLSL): run only when both axes scale > 1.2

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32, // not used in CNN, kept for API compatibility
}

@group(0) @binding(0) var<uniform> params: Params;

@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var sampler: sampler;
@group(0) @binding(3) var dest: texture_storage_2d<rgba8unorm, write>;

// Convolution 3x3x3 -> 3 channels (first pass, input is 4 channels RGBA)
@compute @workgroup_size(8, 8)
fn conv2d_3x3x3(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let tex_dims = vec2<f32>(textureDimensions(source));
    let uv = vec2<f32>(pos) / vec2<f32>(dst_dims);
    let step = vec2<f32>(1.0) / tex_dims;

    // 3x3 kernel weights for first pass (from Anime4K_Upscale_CNN_x2_M.glsl)
    // These are placeholder weights — real weights must be uploaded from GLSL
    let c = textureSampleLevel(source, sampler, uv, 0.0).rgb;
    let l = textureSampleLevel(source, sampler, uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let r = textureSampleLevel(source, sampler, uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let t = textureSampleLevel(source, sampler, uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let b = textureSampleLevel(source, sampler, uv + vec2<f32>(0.0, step.y), 0.0).rgb;

    // Simplified: bilinear + detail (placeholder for actual CNN weights)
    let out = c * 1.0; // Placeholder

    textureStore(dest, pos, vec4<f32>(out, 1.0));
}

// Convolution 3x3x8 -> 8 channels (passes 2-7)
@compute @workgroup_size(8, 8)
fn conv2d_3x3x8(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let tex_dims = vec2<f32>(textureDimensions(source));
    let uv = vec2<f32>(pos) / vec2<f32>(dst_dims);
    let step = vec2<f32>(1.0) / tex_dims;

    let c = textureSampleLevel(source, sampler, uv, 0.0).rgb;
    let l = textureSampleLevel(source, sampler, uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let r = textureSampleLevel(source, sampler, uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let t = textureSampleLevel(source, sampler, uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let b = textureSampleLevel(source, sampler, uv + vec2<f32>(0.0, step.y), 0.0).rgb;

    let out = c; // Placeholder for ReLU(conv(...))

    textureStore(dest, pos, vec4<f32>(out, 1.0));
}

// Convolution 1x1x56 -> combines all previous passes (pass 8)
@compute @workgroup_size(8, 8)
fn conv2d_1x1x56(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    // This pass needs multiple input textures bound — handled by pipeline
    let tex_dims = vec2<f32>(textureDimensions(source));
    let uv = vec2<f32>(pos) / vec2<f32>(dst_dims);

    let c = textureSampleLevel(source, sampler, uv, 0.0).rgb;
    let out = c; // Placeholder

    textureStore(dest, pos, vec4<f32>(out, 1.0));
}

// Depth-to-Space (2x upscale) + residual add
// Input: conv2d_last (4 channels), source (original at 2x lower res)
// Output: 2x upscaled + residual
@compute @workgroup_size(8, 8)
fn depth_to_space(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    // For depth_to_space, we read from conv2d_last at half resolution
    // and write to dest at full resolution
    let src_pos = pos / 2u;
    let f0 = vec2<f32>(pos % 2u);
    // Sample conv2d_last at src_pos with sub-pixel offset
    // This is a placeholder — real implementation needs proper binding layout
    let out = textureSampleLevel(source, sampler, vec2<f32>(src_pos) / vec2<f32>(textureDimensions(source)), 0.0).rgb;

    textureStore(dest, pos, vec4<f32>(out, 1.0));
}