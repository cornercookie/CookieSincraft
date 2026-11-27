// FSR 1.0 RCAS (Robust Contrast Adaptive Sharpening) — WGSL port of AMD FidelityFX FSR 1.0
// Source: docs/001-project-review-and-gap-fix/upstream/ffx_fsr1.h (commit a21ffb8)
//         docs/001-project-review-and-gap-fix/upstream/ffx_a.h            (commit a21ffb8)
// License: MIT (AMD). File header retained above; see upstream files.
//
// This shader performs the RCAS pass: contrast-adaptive sharpening on the EASU output.
// Runs after EASU (fsr_easu.wgsl) as a separate pass. No scaling.
//
// Constant (CPU-computed via FsrRcasCon, uploaded as 1 x vec4 = 4 f32):
//   con = [sharpness, sharpness, 0, 0]
//   where sharpness = exp2(-stops); stops is the RCAS "sharpness" parameter
//   (0.0 = maximum sharpness, N>0 = N stops of halving reduction).
//
// Default sharpness: ffx_fsr1.h:662-672 documents FsrRcasCon(stops) but gives
// no numeric default stops value in its comments. The RCAS call site example
// (ffx_fsr1.h:644-646) passes 0.0 (maximum sharpness) as illustration only,
// not a prescribed default. Per project rule, no upstream line number names a
// default, so we use a named constant and mark it "非上游指定預設".
//
// Alpha: passthrough from input (FSR_RCAS_PASSTHROUGH_ALPHA behaviour), NOT
// hardcoded to 1.
//
// Min/max ring clamping and noise detection are ported from upstream
// ffx_fsr1.h:684 (FsrRcasF, non-packed 32-bit).

struct Params {
    con_x: f32, con_y: f32, con_z: f32, con_w: f32,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;

// FSR_RCAS_LIMIT = 0.25 - (1.0/16.0) = 0.1875
const FSR_RCAS_LIMIT: f32 = 0.1875;

// RCAS default: named constant, NOT an upstream-specified default.
// 非上游指定預設：upstream 註解未給出具體 stops 預設值。
const RCAS_DEFAULT_STOPS: f32 = 0.33;

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn luma3(c: vec3<f32>) -> f32 {
    return c.z * 0.5 + (c.x * 0.5 + c.y);
}

// Upstream uses ARcpF1 with no clamping. The peak-range denominator
// (4*mn4 + peakC.y) is NEGATIVE whenever mn4 < 1, so clamping it to a positive
// epsilon flips the limiter sign and corrupts edges. Only exact zero is
// guarded, because 0 * inf would be NaN.
fn rcp_signed(d: f32) -> f32 {
    if d == 0.0 {
        return 0.0;
    }
    return 1.0 / d;
}

fn texel_load_xy(src: texture_2d<f32>, size_w: f32, size_h: f32, ix: i32, iy: i32) -> vec4<f32> {
    let cx = clamp(ix, 0, i32(size_w) - 1);
    let cy = clamp(iy, 0, i32(size_h) - 1);
    return textureLoad(src, vec2<i32>(cx, cy), 0);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let tex_dims = vec2<f32>(f32(textureDimensions(source).x), f32(textureDimensions(source).y));

    // 3x3 cross neighbourhood (upstream ffx_fsr1.h:697-707). RCAS is 1:1, so
    // the output pixel index is the source texel index.
    let cx = i32(pos.x);
    let cy = i32(pos.y);
    let b = texel_load_xy(source, tex_dims.x, tex_dims.y, cx,     cy - 1); // top
    let d = texel_load_xy(source, tex_dims.x, tex_dims.y, cx - 1, cy);     // left
    let e = texel_load_xy(source, tex_dims.x, tex_dims.y, cx,     cy);     // centre
    let f = texel_load_xy(source, tex_dims.x, tex_dims.y, cx + 1, cy);     // right
    let h = texel_load_xy(source, tex_dims.x, tex_dims.y, cx,     cy + 1); // bottom

    // Luma times 2 (upstream ffx_fsr1.h:731-735).
    let bL = luma3(b.rgb);
    let dL = luma3(d.rgb);
    let eL = luma3(e.rgb);
    let fL = luma3(f.rgb);
    let hL = luma3(h.rgb);

    // Noise detection (upstream ffx_fsr1.h:737-739).
    let nz = 0.25 * bL + 0.25 * dL + 0.25 * fL + 0.25 * hL - eL;
    let range = max(max(max(bL, dL), eL), max(fL, hL)) - min(min(min(bL, dL), eL), min(fL, hL));
    let nz2 = saturate(abs(nz) * (1.0 / max(range, 0.000001)));
    let nz3 = -0.5 * nz2 + 1.0;

    // Min and max of ring (upstream ffx_fsr1.h:741-746).
    let mn4R = min(min(min(b.r, d.r), f.r), h.r);
    let mn4G = min(min(min(b.g, d.g), f.g), h.g);
    let mn4B = min(min(min(b.b, d.b), f.b), h.b);
    let mx4R = max(max(max(b.r, d.r), f.r), h.r);
    let mx4G = max(max(max(b.g, d.g), f.g), h.g);
    let mx4B = max(max(max(b.b, d.b), f.b), h.b);

    // Sharpness from stops (upstream FsrRcasCon: sharpness = exp2(-stops)).
    // params.con_x holds sharpness (linear). Clamp to [0, 1].
    let sharpness_linear = saturate(params.con_x);
    // Apply RCAS_DEFAULT_STOPS if uniforms not yet set: sharpness = exp2(-stops).
    // For GPU path we trust the uploaded con_x value.

    // Limiters (upstream ffx_fsr1.h:750-759). peakC = (1.0, -4.0).
    let peakC_x = 1.0;
    let peakC_y = -4.0;
    let hitMinR = min(mn4R, e.r) * rcp_signed(4.0 * mx4R);
    let hitMinG = min(mn4G, e.g) * rcp_signed(4.0 * mx4G);
    let hitMinB = min(mn4B, e.b) * rcp_signed(4.0 * mx4B);
    let hitMaxR = (peakC_x - max(mx4R, e.r)) * rcp_signed(4.0 * mn4R + peakC_y);
    let hitMaxG = (peakC_x - max(mx4G, e.g)) * rcp_signed(4.0 * mn4G + peakC_y);
    let hitMaxB = (peakC_x - max(mx4B, e.b)) * rcp_signed(4.0 * mn4B + peakC_y);
    let lobeR = max(-hitMinR, hitMaxR);
    let lobeG = max(-hitMinG, hitMaxG);
    let lobeB = max(-hitMinB, hitMaxB);
    let lobe_rgb = max(lobeR, max(lobeG, lobeB));
    let lobe = max(-FSR_RCAS_LIMIT, min(lobe_rgb, 0.0)) * sharpness_linear;

    // Resolve (upstream ffx_fsr1.h:765-768). 4*lobe+1 is in [0.25, 1].
    let rcpL = rcp_signed(4.0 * lobe + 1.0);
    let out_r = (lobe * b.r + lobe * d.r + lobe * h.r + lobe * f.r + e.r) * rcpL;
    let out_g = (lobe * b.g + lobe * d.g + lobe * h.g + lobe * f.g + e.g) * rcpL;
    let out_b = (lobe * b.b + lobe * d.b + lobe * h.b + lobe * f.b + e.b) * rcpL;

    // Alpha passthrough (upstream FSR_RCAS_PASSTHROUGH_ALPHA).
    let out_a = e.a;

    textureStore(dest, pos, vec4<f32>(out_r, out_g, out_b, out_a));
}
