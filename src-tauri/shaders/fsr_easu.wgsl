// FSR 1.0 EASU (Edge-Adaptive Spatial Upsampling) — WGSL port of AMD FidelityFX FSR 1.0
// Source: docs/001-project-review-and-gap-fix/upstream/ffx_fsr1.h (commit a21ffb8)
//         docs/001-project-review-and-gap-fix/upstream/ffx_a.h            (commit a21ffb8)
// License: MIT (AMD). File header retained above; see upstream files.
//
// This shader performs the EASU upscaling pass: edge-adaptive spatial upsampling.
// Two-pass FSR: EASU first (this shader), then RCAS (fsr_rcas.wgsl).
//
// Constants (CPU-computed via FsrEasuCon, uploaded as 4 x vec4 = 16 f32):
//   con0 = [vpW/dstW, vpH/dstH, 0.5*vpW/dstW-0.5, 0.5*vpH/dstH-0.5]
//   con1 = [1/srcW, 1/srcH,  1/srcW,           -1/srcH          ]
//   con2 = [-1/srcW, 2/srcH,  1/srcW,           2/srcH           ]
//   con3 = [0, 4/srcH, 0, 0]
//
// Intermediate texture: rgba16float. RGB walks FSR; alpha is source-alpha
// bilinear, NOT hardcoded to 1.
//
// textureGather is replaced by textureLoad + clamp-to-edge per upstream
// permission ("textureGather 可改 textureLoad 加邊界夾取").
// This is a 12-tap kernel (taps b,c,e,f,g,h,i,j,k,l,n,o); NOT a 3x3 sharpen.

struct Params {
    con0_x: f32, con0_y: f32, con0_z: f32, con0_w: f32,
    con1_x: f32, con1_y: f32, con1_z: f32, con1_w: f32,
    con2_x: f32, con2_y: f32, con2_z: f32, con2_w: f32,
    con3_x: f32, con3_y: f32, con3_z: f32, con3_w: f32,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: Params;

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

// Luma times 2, matching upstream: L = B*0.5 + (R*0.5 + G)
fn luma3(c: vec3<f32>) -> f32 {
    return c.z * 0.5 + (c.x * 0.5 + c.y);
}

fn texel_load_xy(src: texture_2d<f32>, size_w: f32, size_h: f32, ix: i32, iy: i32) -> vec4<f32> {
    let cx = clamp(ix, 0, i32(size_w) - 1);
    let cy = clamp(iy, 0, i32(size_h) - 1);
    return textureLoad(src, vec2<i32>(cx, cy), 0);
}

// Load a 2x2 block of texels from `source` with clamp-to-edge.
// Returns [tl, tr, bl, br]. (WGSL does not allow nested functions, so this
// lives at module level and reads the module-level `source` texture.)
fn load_offset(base: vec2<i32>, ox: i32, oy: i32, size_w: f32, size_h: f32) -> vec4<f32> {
    return texel_load_xy(source, size_w, size_h, base.x + ox, base.y + oy);
}

// Lanczos2-like weight function (upstream FsrEasuTapF weight).
fn lanczos2_w(d2: f32, lob: f32, clp: f32) -> f32 {
    let d2c = min(d2, clp);
    let wB = (2.0 / 5.0) * d2c - 1.0;
    let wA = lob * d2c - 1.0;
    let wB2 = wB * wB;
    let wA2 = wA * wA;
    let wB3 = (25.0 / 16.0) * wB2 - (25.0 / 16.0 - 1.0);
    return wB3 * wA2;
}

// Rotate offset by direction, scale by anisotropic length.
fn tap_weight(off: vec2<f32>, dir: vec2<f32>, len2: vec2<f32>, lob: f32, clp: f32) -> f32 {
    let v = vec2<f32>(
        off.x * dir.x + off.y * dir.y,
        off.x * (-dir.y) + off.y * dir.x,
    );
    let vs = vec2<f32>(v.x * len2.x, v.y * len2.y);
    let d2 = vs.x * vs.x + vs.y * vs.y;
    return lanczos2_w(d2, lob, clp);
}

// One FsrEasuSetF quadrant (ffx_fsr1.h:283-310): returns
// [dirX*w, dirY*w, (lenX^2 + lenY^2)*w]. The four calls are made from main.
fn easu_set(lA: f32, lB: f32, lC: f32, lD: f32, lE: f32, w: f32) -> vec3<f32> {
    let dc = lD - lC;
    let cb = lC - lB;
    let lenX = max(abs(dc), abs(cb));
    let dirX = lD - lB;
    let satX = saturate(abs(dirX) * (1.0 / max(lenX, 1e-30)));
    let ec = lE - lC;
    let ca = lC - lA;
    let lenY = max(abs(ec), abs(ca));
    let dirY = lE - lA;
    let satY = saturate(abs(dirY) * (1.0 / max(lenY, 1e-30)));
    return vec3<f32>(dirX * w, dirY * w, (satX * satX + satY * satY) * w);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    // Source dimensions reconstructed from con1 (con1.x = 1/srcW, con1.y = 1/srcH).
    let src_w = 1.0 / params.con1_x;
    let src_h = 1.0 / params.con1_y;
    let fsrc_size = vec2<f32>(src_w, src_h);
    let fdst_size = vec2<f32>(f32(dst_dims.x), f32(dst_dims.y));

    // Position of 'f' (centre tap): pp = ip * con0.xy + con0.zw
    let pp = vec2<f32>(f32(pos.x), f32(pos.y)) * vec2<f32>(params.con0_x, params.con0_y)
        + vec2<f32>(params.con0_z, params.con0_w);
    // Integer texel position of 'f'; frac is the sub-texel offset
    // (upstream: fp = floor(pp); pp -= fp). Never truncate toward zero:
    // con0.zw is negative for the first output row/column.
    let ip = vec2<i32>(i32(floor(pp.x)), i32(floor(pp.y)));
    let frac = pp - vec2<f32>(f32(ip.x), f32(ip.y));

    // 12-tap kernel, integer offsets from 'f' (ffx_fsr1.h:336-360):
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let bC = load_offset(ip, 0, -1, src_w, src_h).rgb;
    let cC = load_offset(ip, 1, -1, src_w, src_h).rgb;
    let eC = load_offset(ip, -1, 0, src_w, src_h).rgb;
    let fC = load_offset(ip, 0, 0, src_w, src_h).rgb;
    let gC = load_offset(ip, 1, 0, src_w, src_h).rgb;
    let hC = load_offset(ip, 2, 0, src_w, src_h).rgb;
    let iC = load_offset(ip, -1, 1, src_w, src_h).rgb;
    let jC = load_offset(ip, 0, 1, src_w, src_h).rgb;
    let kC = load_offset(ip, 1, 1, src_w, src_h).rgb;
    let lC = load_offset(ip, 2, 1, src_w, src_h).rgb;
    let nC = load_offset(ip, 0, 2, src_w, src_h).rgb;
    let oC = load_offset(ip, 1, 2, src_w, src_h).rgb;

    let bL = luma3(bC);
    let cL = luma3(cC);
    let iL = luma3(iC);
    let jL = luma3(jC);
    let fL = luma3(fC);
    let eL = luma3(eC);
    let kL = luma3(kC);
    let lL = luma3(lC);
    let hL = luma3(hC);
    let gL = luma3(gC);
    let oL = luma3(oC);
    let nL = luma3(nC);

    // ---- Direction & length: four bilinear quadrants (ffx_fsr1.h:383-386) ----
    // Each quadrant is one FsrEasuSetF call: dirX*w, dirY*w and len*w.
    let wS = (1.0 - frac.x) * (1.0 - frac.y);
    let wT = frac.x * (1.0 - frac.y);
    let wU = (1.0 - frac.x) * frac.y;
    let wV = frac.x * frac.y;
    let sS = easu_set(bL, eL, fL, gL, jL, wS);
    let sT = easu_set(cL, fL, gL, hL, kL, wT);
    let sU = easu_set(fL, iL, jL, kL, nL, wU);
    let sV = easu_set(gL, jL, kL, lL, oL, wV);
    var dir = vec2<f32>(
        sS.x + sT.x + sU.x + sV.x,
        sS.y + sT.y + sU.y + sV.y,
    );
    var len = sS.z + sT.z + sU.z + sV.z;

    // Normalize direction (upstream: zro keeps dir.y, forces dir.x = 1).
    let dir2 = dir * dir;
    let dirR = dir2.x + dir2.y;
    let zro = dirR < (1.0 / 32768.0);
    let dirR_inv = select(1.0 / sqrt(max(dirR, 1e-30)), 1.0, zro);
    dir = select(dir * dirR_inv, vec2<f32>(1.0, dir.y), zro);

    // Shape length: {0 to 2} -> {0 to 1}, square.
    len = len * 0.5;
    len = len * len;

    // Stretch kernel {1.0 vert|horz, to sqrt(2.0) on diagonal}.
    let stretch = (dir.x * dir.x + dir.y * dir.y)
        * (1.0 / max(abs(dir.x), abs(dir.y)));
    let len2_x = 1.0 + (stretch - 1.0) * len;
    let len2_y = 1.0 + (-0.5) * len;
    let len2 = vec2<f32>(len2_x, len2_y);

    // Lobes and clipping window.
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = 1.0 / max(lob, 0.000001);

    // Min/max of the 4 nearest taps (upstream ffx_fsr1.h:416-419):
    // ijfe.z = f, klhg.w = g, ijfe.y = j, klhg.x = k.
    let min4 = min(min(min(fC, gC), jC), kC);
    let max4 = max(max(max(fC, gC), jC), kC);

    // ---- 12-tap accumulation (upstream ffx_fsr1.h:423-434) ----
    // Each tap: offset in pp fractional space, rotated by dir, scaled by len2.
    var aC = vec3<f32>(0.0, 0.0, 0.0);
    var aW = 0.0;

    {
        let w = tap_weight(vec2<f32>(0.0, -1.0) - frac, dir, len2, lob, clp);
        aC = aC + bC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(1.0, -1.0) - frac, dir, len2, lob, clp);
        aC = aC + cC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(-1.0, 1.0) - frac, dir, len2, lob, clp);
        aC = aC + iC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(0.0, 1.0) - frac, dir, len2, lob, clp);
        aC = aC + jC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(0.0, 0.0) - frac, dir, len2, lob, clp);
        aC = aC + fC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(-1.0, 0.0) - frac, dir, len2, lob, clp);
        aC = aC + eC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(1.0, 1.0) - frac, dir, len2, lob, clp);
        aC = aC + kC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(2.0, 1.0) - frac, dir, len2, lob, clp);
        aC = aC + lC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(2.0, 0.0) - frac, dir, len2, lob, clp);
        aC = aC + hC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(1.0, 0.0) - frac, dir, len2, lob, clp);
        aC = aC + gC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(1.0, 2.0) - frac, dir, len2, lob, clp);
        aC = aC + oC * w;
        aW = aW + w;
    }
    {
        let w = tap_weight(vec2<f32>(0.0, 2.0) - frac, dir, len2, lob, clp);
        aC = aC + nC * w;
        aW = aW + w;
    }

    // Normalize and dering: pix = min(max4, max(min4, aC / aW)).
    let inv_w = select(0.0, 1.0 / aW, aW > 0.0);
    let pix = aC * inv_w;
    let out_rgb = min(max4, max(min4, pix));

    // Alpha: source-alpha bilinear over the four nearest taps f/g/j/k
    // (NOT hardcoded to 1, and NOT the gather base of the old port).
    let a00 = texel_load_xy(source, src_w, src_h, ip.x, ip.y).a;
    let a10 = texel_load_xy(source, src_w, src_h, ip.x + 1, ip.y).a;
    let a01 = texel_load_xy(source, src_w, src_h, ip.x, ip.y + 1).a;
    let a11 = texel_load_xy(source, src_w, src_h, ip.x + 1, ip.y + 1).a;
    let alpha = mix(mix(a00, a10, frac.x), mix(a01, a11, frac.x), frac.y);

    textureStore(dest, pos, vec4<f32>(out_rgb, alpha));
}
