// FSR 1.0 RCAS (Robust Contrast Adaptive Sharpening) — simplified compute pass.
// Reads the EASU-upscaled intermediate texture and applies contrast-adaptive
// sharpening, writing the final result to a storage texture.
// Uniform layout: 5 x f32 = [src_w, src_h, dst_w, dst_h, sharpness]

[[group(0), binding(0)]] var<uniform> params: Params;
[[group(0), binding(1)]] var samp: sampler;
[[group(0), binding(2)]] var src: texture_2d<f32>;
[[group(0), binding(3)]] var dst: texture_storage_2d<rgba8unorm, write>;

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32,
};

[[stage(compute), workgroup_size(8, 8)]]
fn main([[builtin(global_invocation_id)]] gid: vec3<u32>) {
    let dst_dims = textureDimensions(dst);
    let pos = vec2<i32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let tex_dims = vec2<f32>(textureDimensions(src));
    let uv = vec2<f32>(pos) / vec2<f32>(dst_dims);
    let step = vec2<f32>(1.0) / tex_dims;

    let c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    let l = textureSampleLevel(src, samp, uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let r = textureSampleLevel(src, samp, uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let t = textureSampleLevel(src, samp, uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let b = textureSampleLevel(src, samp, uv + vec2<f32>(0.0, step.y), 0.0).rgb;

    // RCAS: contrast between centre pixel and its local neighbourhood.
    let local_mean = (l + r + t + b) / 4.0;
    let contrast = c - local_mean;
    let amount = clamp(params.sharpness, 0.0, 1.0) * 0.8;
    let out = c + contrast * amount;

    textureStore(dst, pos, vec4<f32>(out, 1.0));
}
