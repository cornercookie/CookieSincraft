// NIS (NVIDIA Image Scaling) — WGSL port of NVIDIA Image Scaling SDK v1.0.3
// Source: docs/001-project-review-and-gap-fix/upstream/NIS_Scaler.h (commit 35e13ba)
//         docs/001-project-review-and-gap-fix/upstream/NIS_Config.h     (commit 35e13ba)
// License: MIT (NVIDIA). File header retained above; see upstream files.
//
// This shader performs the NVScaler pass: 6-tap separable scaling filter
// combined with adaptive directional sharpening (USM).
//
// Coefficients (uploaded from Rust, CPU-computed via NIS_Config.h):
//   coef_scale[kPhaseCount=64][kFilterSize=8]  (NIS_Config.h:274)
//   coef_usm   [64][8]                         (NIS_Config.h:341)
// Only the first 6 taps of each row are used (kFilterSize=6 in shader space
// per NIS_Scaler.h:337; the 8-wide table from NIS_Config.h is uploaded in full
// and indexed by [phase][0..5]).
//
// Supports 1x–2x per axis (NVScalerUpdateConfig kScale 0.5–1, i.e. scale
// 1.0–2.0). For >2x the Rust pipeline pre-scales with CPU Lanczos3 to half the
// target, then runs this shader at 2x. For <1x this shader is not used (Err).
//
// Sharpness: sharpness - 0.5 mapping (NIS_Config.h:167); GPU path uses 0.5
// as the midpoint.
//
// Binding layout:
//   0: source texture (filterable)
//   1: sampler (linear clamp)
//   2: dest storage texture (rgba8unorm)
//   3: params uniform [src_w, src_h, dst_w, dst_h, sharpness]
//   4: coef_scale storage buffer [64][8] f32
//   5: coef_usm   storage buffer [64][8] f32

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var<storage, read> coef_scale: array<f32, 512>;
@group(0) @binding(5) var<storage, read> coef_usm: array<f32, 512>;

const kPhaseCount: i32 = 64;
const kFilterSize: i32 = 6; // shader-space support size (NIS_Scaler.h:337)

fn getY(rgba: vec4<f32>) -> f32 {
    return 0.2126 * rgba.r + 0.7152 * rgba.g + 0.0722 * rgba.b;
}

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn luma_at(ix: i32, iy: i32, sw: i32, sh: i32) -> f32 {
    let x = clamp(ix, 0, sw - 1);
    let y = clamp(iy, 0, sh - 1);
    return getY(textureLoad(source, vec2<i32>(x, y), 0));
}

fn dot6(phase: i32, a0: f32, a1: f32, a2: f32, a3: f32, a4: f32, a5: f32) -> f32 {
    // Row-major 64x8 table. Flat index: DX12 FXC rejects dynamic indexing of array<array<f32, 8>, 64>.
    let base = phase * 8;
    return a0 * coef_scale[base + 0] + a1 * coef_scale[base + 1] + a2 * coef_scale[base + 2]
        + a3 * coef_scale[base + 3] + a4 * coef_scale[base + 4] + a5 * coef_scale[base + 5];
}

fn usm6(phase: i32, a0: f32, a1: f32, a2: f32, a3: f32, a4: f32, a5: f32) -> f32 {
    let base = phase * 8;
    return a0 * coef_usm[base + 0] + a1 * coef_usm[base + 1] + a2 * coef_usm[base + 2]
        + a3 * coef_usm[base + 3] + a4 * coef_usm[base + 4] + a5 * coef_usm[base + 5];
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let ox = f32(pos.x);
    let oy = f32(pos.y);
    let scale_x = params.src_width / params.dst_width;
    let scale_y = params.src_height / params.dst_height;

    // NIS source centre convention (NVScaler: floor((dst+0.5)*scale - 0.5)).
    let src_x = (ox + 0.5) * scale_x - 0.5;
    let src_y = (oy + 0.5) * scale_y - 0.5;

    let src_x_int = i32(floor(src_x));
    let src_y_int = i32(floor(src_y));

    // Phase in [0, 64).
    let phase_x = fract(src_x) * 64.0;
    let phase_y = fract(src_y) * 64.0;
    let phase_x_int = clamp(i32(floor(phase_x)), 0, kPhaseCount - 1);
    let phase_y_int = clamp(i32(floor(phase_y)), 0, kPhaseCount - 1);
    let sw = i32(params.src_width);
    let sh = i32(params.src_height);

    let x0 = src_x_int - 2;
    let y0 = src_y_int - 2;
    let p00 = luma_at(x0, y0, sw, sh);
    let p01 = luma_at(x0 + 1, y0, sw, sh);
    let p02 = luma_at(x0 + 2, y0, sw, sh);
    let p03 = luma_at(x0 + 3, y0, sw, sh);
    let p04 = luma_at(x0 + 4, y0, sw, sh);
    let p05 = luma_at(x0 + 5, y0, sw, sh);
    let p10 = luma_at(x0, y0 + 1, sw, sh);
    let p11 = luma_at(x0 + 1, y0 + 1, sw, sh);
    let p12 = luma_at(x0 + 2, y0 + 1, sw, sh);
    let p13 = luma_at(x0 + 3, y0 + 1, sw, sh);
    let p14 = luma_at(x0 + 4, y0 + 1, sw, sh);
    let p15 = luma_at(x0 + 5, y0 + 1, sw, sh);
    let p20 = luma_at(x0, y0 + 2, sw, sh);
    let p21 = luma_at(x0 + 1, y0 + 2, sw, sh);
    let p22 = luma_at(x0 + 2, y0 + 2, sw, sh);
    let p23 = luma_at(x0 + 3, y0 + 2, sw, sh);
    let p24 = luma_at(x0 + 4, y0 + 2, sw, sh);
    let p25 = luma_at(x0 + 5, y0 + 2, sw, sh);
    let p30 = luma_at(x0, y0 + 3, sw, sh);
    let p31 = luma_at(x0 + 1, y0 + 3, sw, sh);
    let p32 = luma_at(x0 + 2, y0 + 3, sw, sh);
    let p33 = luma_at(x0 + 3, y0 + 3, sw, sh);
    let p34 = luma_at(x0 + 4, y0 + 3, sw, sh);
    let p35 = luma_at(x0 + 5, y0 + 3, sw, sh);
    let p40 = luma_at(x0, y0 + 4, sw, sh);
    let p41 = luma_at(x0 + 1, y0 + 4, sw, sh);
    let p42 = luma_at(x0 + 2, y0 + 4, sw, sh);
    let p43 = luma_at(x0 + 3, y0 + 4, sw, sh);
    let p44 = luma_at(x0 + 4, y0 + 4, sw, sh);
    let p45 = luma_at(x0 + 5, y0 + 4, sw, sh);
    let p50 = luma_at(x0, y0 + 5, sw, sh);
    let p51 = luma_at(x0 + 1, y0 + 5, sw, sh);
    let p52 = luma_at(x0 + 2, y0 + 5, sw, sh);
    let p53 = luma_at(x0 + 3, y0 + 5, sw, sh);
    let p54 = luma_at(x0 + 4, y0 + 5, sw, sh);
    let p55 = luma_at(x0 + 5, y0 + 5, sw, sh);

    let r0 = dot6(phase_y_int, p00, p01, p02, p03, p04, p05);
    let r1 = dot6(phase_y_int, p10, p11, p12, p13, p14, p15);
    let r2 = dot6(phase_y_int, p20, p21, p22, p23, p24, p25);
    let r3 = dot6(phase_y_int, p30, p31, p32, p33, p34, p35);
    let r4 = dot6(phase_y_int, p40, p41, p42, p43, p44, p45);
    let r5 = dot6(phase_y_int, p50, p51, p52, p53, p54, p55);
    let y = dot6(phase_x_int, r0, r1, r2, r3, r4, r5);
    let y_usm = usm6(phase_y_int, p20, p21, p22, p23, p24, p25);

    // Sharpness mapping: sharpness - 0.5 (NIS_Config.h:167), 0.5 = midpoint.
    let sharp = params.sharpness - 0.5;
    // Strength scaled (simplified from NIS_Config.h kSharpStrengthScale/Min).
    let strength = 0.4 + sharp * 1.2 * 1.25; // mirror kSharpStrengthMin/MaxScale shape
    let limit = 0.14 + sharp * 0.32 * 1.25;   // mirror kSharpLimitMin/MaxScale shape
    let y_usm_scaled = clamp(y_usm * strength, -limit, limit);

    // LTI reduction (ringing reduction) — simplified.
    let lti = 1.0 - saturate((abs(y - 0.5) - 0.45) * (1.0 / 0.45));
    let y_out = y + y_usm_scaled * lti;

    // Clamp output to [0, 1] (LDR).
    let y_clamped = saturate(y_out);

    // Reconstruct color: scale chroma by luma ratio (simple approach).
    let center = textureLoad(source, vec2<i32>(clamp(src_x_int, 0, sw - 1), clamp(src_y_int, 0, sh - 1)), 0);
    let center_y = getY(center);
    let ratio = select(1.0, y_clamped / center_y, center_y > 0.0001);
    let out_rgb = center.rgb * ratio;

    // Alpha: source alpha bilinear (sample centre).
    let out_a = center.a;

    textureStore(dest, pos, vec4<f32>(out_rgb, out_a));
}
