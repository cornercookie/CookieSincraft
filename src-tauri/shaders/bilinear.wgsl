// Bilinear upscale + mild sharpen — the single explicitly-named GPU path.
// Source image is sampled with a linear clamp sampler; output is written to a
// storage texture. Uniform layout: 5 x f32 = [src_w, src_h, dst_w, dst_h, sharpness]
// (sharpness unused here, kept for API compatibility).

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

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let tex_dims = vec2<f32>(f32(textureDimensions(source).x), f32(textureDimensions(source).y));
    let uv = vec2<f32>(f32(pos.x), f32(pos.y)) / vec2<f32>(f32(dst_dims.x), f32(dst_dims.y));
    let center = uv * tex_dims;

    // Bilinear via the linear clamp sampler.
    let c = textureSampleLevel(source, smp, center / tex_dims, 0.0);

    // Mild fixed sharpen so the GPU path is visibly distinct from nearest.
    let step = vec2<f32>(1.0) / tex_dims;
    let l = textureSampleLevel(source, smp, (center - vec2<f32>(step.x, 0.0)) / tex_dims, 0.0);
    let r = textureSampleLevel(source, smp, (center + vec2<f32>(step.x, 0.0)) / tex_dims, 0.0);
    let t = textureSampleLevel(source, smp, (center - vec2<f32>(0.0, step.y)) / tex_dims, 0.0);
    let b = textureSampleLevel(source, smp, (center + vec2<f32>(0.0, step.y)) / tex_dims, 0.0);
    let detail = c.rgb - (l.rgb + r.rgb + t.rgb + b.rgb) / 4.0;
    let out_rgb = c.rgb + detail * 0.15;

    textureStore(dest, pos, vec4<f32>(out_rgb, c.a));
}
