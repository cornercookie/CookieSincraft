// Anime4K CNN x2 (M) — pass 1/9: conv2d_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x3x3x8
// GLSL BIND: MAIN
// GLSL SAVE: conv2d_tf

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    _pad: f32,
}

@group(0) @binding(0) var source0: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: Params;

fn relu_pos(v: vec4<f32>) -> vec4<f32> {
    return max(v, vec4<f32>(0.0));
}
fn relu_neg(v: vec4<f32>) -> vec4<f32> {
    return max(-v, vec4<f32>(0.0));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dims = textureDimensions(dest);
    let pos = vec2<u32>(gid.xy);
    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {
        return;
    }

    let src_dims = vec2<f32>(params.src_width, params.src_height);
    let fpos = vec2<f32>(f32(pos.x), f32(pos.y));
    let uv = fpos / vec2<f32>(f32(dst_dims.x), f32(dst_dims.y));
    let step = vec2<f32>(1.0) / src_dims;

    var out0 = 0.0;
    var out1 = 0.0;
    var out2 = 0.0;
    var out3 = 0.0;

    {
        let sgo0_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo0_n1_n1 = relu_pos(sgo0_n1_n1);
        out0 = out0 + ((-0.010995803 * vgo0_n1_n1[0]) + (0.077095956 * vgo0_n1_n1[1]) + (-0.043992598 * vgo0_n1_n1[2]) + (0.06048717 * vgo0_n1_n1[3]));
        out1 = out1 + ((0.1164834 * vgo0_n1_n1[0]) + (-0.11689607 * vgo0_n1_n1[1]) + (0.072985925 * vgo0_n1_n1[2]) + (-0.078805886 * vgo0_n1_n1[3]));
        out2 = out2 + ((0.01182932 * vgo0_n1_n1[0]) + (0.054985743 * vgo0_n1_n1[1]) + (-0.09018186 * vgo0_n1_n1[2]) + (0.044907484 * vgo0_n1_n1[3]));
        out3 = out3 + ((0 * vgo0_n1_n1[0]) + (0 * vgo0_n1_n1[1]) + (0 * vgo0_n1_n1[2]) + (0 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((0.1813623 * vgo0_n1_z[0]) + (-0.14752422 * vgo0_n1_z[1]) + (0.025720436 * vgo0_n1_z[2]) + (-0.17639883 * vgo0_n1_z[3]));
        out1 = out1 + ((0.15697388 * vgo0_n1_z[0]) + (0.10445984 * vgo0_n1_z[1]) + (-0.1843076 * vgo0_n1_z[2]) + (0.5264643 * vgo0_n1_z[3]));
        out2 = out2 + ((0.047516696 * vgo0_n1_z[0]) + (-0.097305484 * vgo0_n1_z[1]) + (0.09740847 * vgo0_n1_z[2]) + (-0.29619336 * vgo0_n1_z[3]));
        out3 = out3 + ((0 * vgo0_n1_z[0]) + (0 * vgo0_n1_z[1]) + (0 * vgo0_n1_z[2]) + (0 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((-0.014534763 * vgo0_n1_p1[0]) + (0.09486465 * vgo0_n1_p1[1]) + (0.046173926 * vgo0_n1_p1[2]) + (0.039391946 * vgo0_n1_p1[3]));
        out1 = out1 + ((0.09609376 * vgo0_n1_p1[0]) + (-0.060574662 * vgo0_n1_p1[1]) + (0.042200956 * vgo0_n1_p1[2]) + (-0.3269777 * vgo0_n1_p1[3]));
        out2 = out2 + ((0.051006425 * vgo0_n1_p1[0]) + (0.059818447 * vgo0_n1_p1[1]) + (0.04366627 * vgo0_n1_p1[2]) + (0.17699827 * vgo0_n1_p1[3]));
        out3 = out3 + ((0 * vgo0_n1_p1[0]) + (0 * vgo0_n1_p1[1]) + (0 * vgo0_n1_p1[2]) + (0 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((0.04268535 * vgo0_z_n1[0]) + (-0.08152529 * vgo0_z_n1[1]) + (0.10577459 * vgo0_z_n1[2]) + (-0.036936995 * vgo0_z_n1[3]));
        out1 = out1 + ((-0.051562306 * vgo0_z_n1[0]) + (0.054872766 * vgo0_z_n1[1]) + (0.09194519 * vgo0_z_n1[2]) + (0.0025066638 * vgo0_z_n1[3]));
        out2 = out2 + ((-0.01073954 * vgo0_z_n1[0]) + (0.00064474024 * vgo0_z_n1[1]) + (0.10038221 * vgo0_z_n1[2]) + (0.02131141 * vgo0_z_n1[3]));
        out3 = out3 + ((0 * vgo0_z_n1[0]) + (0 * vgo0_z_n1[1]) + (0 * vgo0_z_n1[2]) + (0 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((-0.51751363 * vgo0_z_z[0]) + (-0.40028602 * vgo0_z_z[1]) + (0.3469574 * vgo0_z_z[2]) + (0.5933738 * vgo0_z_z[3]));
        out1 = out1 + ((-0.91357684 * vgo0_z_z[0]) + (-0.67692596 * vgo0_z_z[1]) + (0.57815677 * vgo0_z_z[2]) + (0.39809322 * vgo0_z_z[3]));
        out2 = out2 + ((-0.16341521 * vgo0_z_z[0]) + (-0.27169713 * vgo0_z_z[1]) + (0.12232366 * vgo0_z_z[2]) + (0.4318641 * vgo0_z_z[3]));
        out3 = out3 + ((0 * vgo0_z_z[0]) + (0 * vgo0_z_z[1]) + (0 * vgo0_z_z[2]) + (0 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((0.12601124 * vgo0_z_p1[0]) + (-0.06263236 * vgo0_z_p1[1]) + (-0.45907676 * vgo0_z_p1[2]) + (-0.41514075 * vgo0_z_p1[3]));
        out1 = out1 + ((0.3330334 * vgo0_z_p1[0]) + (-0.1929565 * vgo0_z_p1[1]) + (-0.6333532 * vgo0_z_p1[2]) + (-0.6552794 * vgo0_z_p1[3]));
        out2 = out2 + ((-0.045809917 * vgo0_z_p1[0]) + (0.046351526 * vgo0_z_p1[1]) + (-0.26173338 * vgo0_z_p1[2]) + (-0.30252662 * vgo0_z_p1[3]));
        out3 = out3 + ((0 * vgo0_z_p1[0]) + (0 * vgo0_z_p1[1]) + (0 * vgo0_z_p1[2]) + (0 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((0.0030332592 * vgo0_p1_n1[0]) + (0.012103107 * vgo0_p1_n1[1]) + (0.010537323 * vgo0_p1_n1[2]) + (-0.02038607 * vgo0_p1_n1[3]));
        out1 = out1 + ((0.095558085 * vgo0_p1_n1[0]) + (0.097704545 * vgo0_p1_n1[1]) + (0.083433494 * vgo0_p1_n1[2]) + (0.026790185 * vgo0_p1_n1[3]));
        out2 = out2 + ((0.01943357 * vgo0_p1_n1[0]) + (-0.061712462 * vgo0_p1_n1[1]) + (-0.00015703632 * vgo0_p1_n1[2]) + (-0.032268334 * vgo0_p1_n1[3]));
        out3 = out3 + ((0 * vgo0_p1_n1[0]) + (0 * vgo0_p1_n1[1]) + (0 * vgo0_p1_n1[2]) + (0 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((0.016870102 * vgo0_p1_z[0]) + (0.5215812 * vgo0_p1_z[1]) + (-0.11525501 * vgo0_p1_z[2]) + (0.027527615 * vgo0_p1_z[3]));
        out1 = out1 + ((-0.09045733 * vgo0_p1_z[0]) + (0.61310345 * vgo0_p1_z[1]) + (-0.1575268 * vgo0_p1_z[2]) + (0.1905386 * vgo0_p1_z[3]));
        out2 = out2 + ((0.020172214 * vgo0_p1_z[0]) + (0.3503187 * vgo0_p1_z[1]) + (-0.08209157 * vgo0_p1_z[2]) + (-0.051328037 * vgo0_p1_z[3]));
        out3 = out3 + ((0 * vgo0_p1_z[0]) + (0 * vgo0_p1_z[1]) + (0 * vgo0_p1_z[2]) + (0 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((0.005494087 * vgo0_p1_p1[0]) + (-0.010656317 * vgo0_p1_p1[1]) + (0.07682753 * vgo0_p1_p1[2]) + (-0.08116042 * vgo0_p1_p1[3]));
        out1 = out1 + ((-0.03934524 * vgo0_p1_p1[0]) + (0.16589017 * vgo0_p1_p1[1]) + (0.101483546 * vgo0_p1_p1[2]) + (-0.066603065 * vgo0_p1_p1[3]));
        out2 = out2 + ((0.03494657 * vgo0_p1_p1[0]) + (-0.07885597 * vgo0_p1_p1[1]) + (0.074227594 * vgo0_p1_p1[2]) + (0.0016264897 * vgo0_p1_p1[3]));
        out3 = out3 + ((0 * vgo0_p1_p1[0]) + (0 * vgo0_p1_p1[1]) + (0 * vgo0_p1_p1[2]) + (0 * vgo0_p1_p1[3]));
    }
    out0 = out0 + (0.014463938);
    out1 = out1 + (-0.0031906287);
    out2 = out2 + (0.007015422);
    out3 = out3 + (-0.003888468);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
