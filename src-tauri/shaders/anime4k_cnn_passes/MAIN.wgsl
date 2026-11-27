// Anime4K CNN x2 (M) — pass 9/9: Depth-to-Space (2x upscale)
// Source: docs/001-project-review-and-gap-fix/upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
//         //!HOOK MAIN ... //!SAVE MAIN (Depth-to-Space), lines 284-300
// License: MIT (bloc97). File header retained above; see upstream GLSL.
//
// Depth-to-Space: dest = conv2d_last (MAIN-sized feature map) upscaled 2x.
// Each output pixel selects one of the 4 conv2d_last channels based on its
// sub-pixel position (fract-based depth-to-space), replicates that scalar
// across RGB, then adds the original MAIN texel to RGB only. Alpha is
// preserved from the original MAIN texel (bilinear at texel center = exact).
//
// Inputs:
//   source0 = conv2d_last_tf output (MAIN-sized, rgba16float, signed unclamped)
//   source1 = MAIN original image (MAIN-sized, rgba8unorm)
// Output dest = MAIN-sized * 2 (2x upscale), rgba8unorm.

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    _pad: f32,
}

@group(0) @binding(0) var source0: texture_2d<f32>;
@group(0) @binding(1) var source1: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
@group(0) @binding(3) var dest: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    // Map 2x dest pixel back to the MAIN-sized pixel.
    let cpos = vec2<i32>(pos / 2u);

    // Depth-to-space: select one of 4 channels based on sub-pixel position.
    // Upstream GLSL: i0 = ivec2(fract(pos * size) * vec2(2.0)), then
    // channel = i0.y * 2 + i0.x. Here pos.x % 2 / pos.y % 2 gives the same
    // sub-pixel index (0 or 1) for each axis.
    let sub_x = pos.x % 2u;
    let sub_y = pos.y % 2u;
    let channel = sub_y * 2u + sub_x; // 0, 1, 2, or 3

    // Sample the conv2d_last feature map at the MAIN pixel (signed unclamped float).
    let texel = textureLoad(source0, cpos, 0);
    let scalar = texel[channel];

    // Source1 original image: Design B3 pixel-center bilinear at the
    // normalized destination pixel center, sampled with the filterable
    // sampler (nearest integer load contradicts pixel-center bilinear).
    let uv = (vec2<f32>(pos) + vec2<f32>(0.5)) / vec2<f32>(dst_dims);
    let main_texel = textureSampleLevel(source1, smp, uv, 0.0);

    // Depth-to-Space: replicate the selected channel across RGB, add the
    // original MAIN texel to RGB only (residual). Alpha preserved from
    // the original MAIN texel — NOT added to, NOT replaced by the scalar.
    let out = vec4<f32>(scalar, scalar, scalar, 0.0) + vec4<f32>(main_texel.rgb, main_texel.a);

    textureStore(dest, pos, out);
}
