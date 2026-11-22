// FSR 1.0 EASU (Edge-Adaptive Spatial Upsampling) — simplified compute pass.
// Reads a source texture and writes an upscaled result to a storage texture.
// Uniform layout (std140): 5 x f32 = [src_w, src_h, dst_w, dst_h, sharpness]

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

    // Map destination pixel back into source UV space.
    let scale = vec2<f32>(params.src_width / params.dst_width,
                          params.src_height / params.dst_height);
    let center = vec2<f32>(pos) * scale;
    let tex_dims = vec2<f32>(textureDimensions(src));
    let uv = center / tex_dims;

    // Sample centre and neighbours for edge detection (EASU gradient pass).
    let step = vec2<f32>(1.0) / tex_dims;
    let c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    let l = textureSampleLevel(src, samp, uv - vec2<f32>(step.x, 0.0), 0.0).rgb;
    let r = textureSampleLevel(src, samp, uv + vec2<f32>(step.x, 0.0), 0.0).rgb;
    let t = textureSampleLevel(src, samp, uv - vec2<f32>(0.0, step.y), 0.0).rgb;
    let b = textureSampleLevel(src, samp, uv + vec2<f32>(0.0, step.y), 0.0).rgb;

    let grad_x = r - l;
    let grad_y = b - t;
    let edge = max(abs(grad_x), abs(grad_y));
    let sharpen = smoothstep(0.0, 0.15, edge) * 0.3 * params.sharpness;

    // Edge-adaptive detail boost: emphasise edges, suppress flat areas.
    let detail = c - (l + r + t + b) / 4.0;
    let out = c + detail * sharpen;

    textureStore(dst, pos, vec4<f32>(out, 1.0));
}
