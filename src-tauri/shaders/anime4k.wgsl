// Anime4K — simplified compute pass.
// Anime4K-style upscaling: extract fine detail via a Laplacian-like
// high-pass filter and add it back to the upsampled centre colour.
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

    // Map destination pixel back into source UV space (upscale).
    let scale = vec2<f32>(params.src_width / params.dst_width,
                          params.src_height / params.dst_height);
    let center = vec2<f32>(pos) * scale;
    let tex_dims = vec2<f32>(textureDimensions(src));
    let uv = center / tex_dims;

    // Laplacian detail extraction (Anime4K-style edge/detail enhancement).
    let step = vec2<f32>(1.0) / tex_dims;
    let c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    let l = textureSampleLevel(src, samp, uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let r = textureSampleLevel(src, samp, uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let t = textureSampleLevel(src, samp, uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let b = textureSampleLevel(src, samp, uv + vec2<f32>(0.0, step.y), 0.0).rgb;

    let lap = c * 4.0 - (l + r + t + b);
    let detail = clamp(lap * 0.5, -0.5, 0.5) * clamp(params.sharpness, 0.0, 1.0);
    let out = c + detail;

    textureStore(dst, pos, vec4<f32>(out, 1.0));
}
