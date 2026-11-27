// Anime4K CNN x2 (M) — pass 5/9: conv2d_4_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x3x3x8
// GLSL BIND: conv2d_3_tf
// GLSL SAVE: conv2d_4_tf

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
        out0 = out0 + ((-0.14533128 * vgo0_n1_n1[0]) + (0.07266841 * vgo0_n1_n1[1]) + (0.13238011 * vgo0_n1_n1[2]) + (-0.23328504 * vgo0_n1_n1[3]));
        out1 = out1 + ((0.031516243 * vgo0_n1_n1[0]) + (0.058471266 * vgo0_n1_n1[1]) + (-0.06394412 * vgo0_n1_n1[2]) + (0.090752736 * vgo0_n1_n1[3]));
        out2 = out2 + ((-0.0042359144 * vgo0_n1_n1[0]) + (0.12357294 * vgo0_n1_n1[1]) + (-0.04377495 * vgo0_n1_n1[2]) + (0.0011743477 * vgo0_n1_n1[3]));
        out3 = out3 + ((0.05412243 * vgo0_n1_n1[0]) + (-0.08146249 * vgo0_n1_n1[1]) + (0.04002749 * vgo0_n1_n1[2]) + (-0.032876283 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((-0.036972385 * vgo0_n1_z[0]) + (-0.15238069 * vgo0_n1_z[1]) + (-0.3453321 * vgo0_n1_z[2]) + (-0.36025128 * vgo0_n1_z[3]));
        out1 = out1 + ((0.07597202 * vgo0_n1_z[0]) + (-0.02368151 * vgo0_n1_z[1]) + (-0.3889606 * vgo0_n1_z[2]) + (0.34607083 * vgo0_n1_z[3]));
        out2 = out2 + ((0.3133179 * vgo0_n1_z[0]) + (-0.21712309 * vgo0_n1_z[1]) + (-0.4210954 * vgo0_n1_z[2]) + (0.21450534 * vgo0_n1_z[3]));
        out3 = out3 + ((0.15226828 * vgo0_n1_z[0]) + (0.25326282 * vgo0_n1_z[1]) + (0.45327064 * vgo0_n1_z[2]) + (-0.3350824 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((0.019018406 * vgo0_n1_p1[0]) + (-0.33060563 * vgo0_n1_p1[1]) + (-0.092601225 * vgo0_n1_p1[2]) + (0.14970545 * vgo0_n1_p1[3]));
        out1 = out1 + ((0.1441509 * vgo0_n1_p1[0]) + (-0.19228427 * vgo0_n1_p1[1]) + (-0.032771986 * vgo0_n1_p1[2]) + (0.26331595 * vgo0_n1_p1[3]));
        out2 = out2 + ((0.052981265 * vgo0_n1_p1[0]) + (-0.06627376 * vgo0_n1_p1[1]) + (-0.08634131 * vgo0_n1_p1[2]) + (0.038706224 * vgo0_n1_p1[3]));
        out3 = out3 + ((0.13403937 * vgo0_n1_p1[0]) + (-4.4842476e-05 * vgo0_n1_p1[1]) + (0.049002815 * vgo0_n1_p1[2]) + (-0.12719193 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((0.17527401 * vgo0_z_n1[0]) + (-0.0035254909 * vgo0_z_n1[1]) + (-0.047959115 * vgo0_z_n1[2]) + (-0.4526988 * vgo0_z_n1[3]));
        out1 = out1 + ((-0.07510284 * vgo0_z_n1[0]) + (0.0013256798 * vgo0_z_n1[1]) + (-0.07539148 * vgo0_z_n1[2]) + (0.24220634 * vgo0_z_n1[3]));
        out2 = out2 + ((-0.08708839 * vgo0_z_n1[0]) + (-0.14494033 * vgo0_z_n1[1]) + (-0.17085724 * vgo0_z_n1[2]) + (-0.099797316 * vgo0_z_n1[3]));
        out3 = out3 + ((0.0068515535 * vgo0_z_n1[0]) + (-0.08918779 * vgo0_z_n1[1]) + (0.27164719 * vgo0_z_n1[2]) + (-0.1702649 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((0.31848368 * vgo0_z_z[0]) + (0.48983255 * vgo0_z_z[1]) + (-0.44140294 * vgo0_z_z[2]) + (-0.65174145 * vgo0_z_z[3]));
        out1 = out1 + ((-0.004199057 * vgo0_z_z[0]) + (0.19494705 * vgo0_z_z[1]) + (0.5196497 * vgo0_z_z[2]) + (-0.027118586 * vgo0_z_z[3]));
        out2 = out2 + ((0.032509074 * vgo0_z_z[0]) + (-0.23900363 * vgo0_z_z[1]) + (-0.14489244 * vgo0_z_z[2]) + (0.36314297 * vgo0_z_z[3]));
        out3 = out3 + ((-0.23168536 * vgo0_z_z[0]) + (-0.20960593 * vgo0_z_z[1]) + (0.61471456 * vgo0_z_z[2]) + (0.12401275 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((-0.24317405 * vgo0_z_p1[0]) + (0.21560913 * vgo0_z_p1[1]) + (0.15564032 * vgo0_z_p1[2]) + (0.11606844 * vgo0_z_p1[3]));
        out1 = out1 + ((-0.15039803 * vgo0_z_p1[0]) + (-0.59578896 * vgo0_z_p1[1]) + (0.14100945 * vgo0_z_p1[2]) + (-0.026194477 * vgo0_z_p1[3]));
        out2 = out2 + ((0.37237462 * vgo0_z_p1[0]) + (-0.49472088 * vgo0_z_p1[1]) + (-0.15215331 * vgo0_z_p1[2]) + (-0.38820064 * vgo0_z_p1[3]));
        out3 = out3 + ((-0.25089455 * vgo0_z_p1[0]) + (-0.29643852 * vgo0_z_p1[1]) + (-0.09513793 * vgo0_z_p1[2]) + (0.019779462 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((0.12498539 * vgo0_p1_n1[0]) + (0.0710632 * vgo0_p1_n1[1]) + (-0.25012368 * vgo0_p1_n1[2]) + (-0.2272255 * vgo0_p1_n1[3]));
        out1 = out1 + ((-0.08647026 * vgo0_p1_n1[0]) + (0.12277892 * vgo0_p1_n1[1]) + (0.011025097 * vgo0_p1_n1[2]) + (-0.12168395 * vgo0_p1_n1[3]));
        out2 = out2 + ((-0.13489573 * vgo0_p1_n1[0]) + (0.016708186 * vgo0_p1_n1[1]) + (-0.15583871 * vgo0_p1_n1[2]) + (-0.057124946 * vgo0_p1_n1[3]));
        out3 = out3 + ((0.1216943 * vgo0_p1_n1[0]) + (0.019803725 * vgo0_p1_n1[1]) + (0.06952334 * vgo0_p1_n1[2]) + (-0.032985855 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((0.28794885 * vgo0_p1_z[0]) + (0.33783793 * vgo0_p1_z[1]) + (-0.14469545 * vgo0_p1_z[2]) + (-0.081780486 * vgo0_p1_z[3]));
        out1 = out1 + ((-0.50320613 * vgo0_p1_z[0]) + (-0.067601606 * vgo0_p1_z[1]) + (-0.06847453 * vgo0_p1_z[2]) + (-0.021648854 * vgo0_p1_z[3]));
        out2 = out2 + ((-0.34295765 * vgo0_p1_z[0]) + (0.15071863 * vgo0_p1_z[1]) + (-0.06619896 * vgo0_p1_z[2]) + (-0.084465064 * vgo0_p1_z[3]));
        out3 = out3 + ((0.31909832 * vgo0_p1_z[0]) + (0.015414661 * vgo0_p1_z[1]) + (0.14930317 * vgo0_p1_z[2]) + (-0.11295768 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((0.24530606 * vgo0_p1_p1[0]) + (0.25526014 * vgo0_p1_p1[1]) + (0.09971985 * vgo0_p1_p1[2]) + (-0.07749641 * vgo0_p1_p1[3]));
        out1 = out1 + ((-0.2361951 * vgo0_p1_p1[0]) + (-0.07997673 * vgo0_p1_p1[1]) + (0.03617294 * vgo0_p1_p1[2]) + (0.02959561 * vgo0_p1_p1[3]));
        out2 = out2 + ((-0.4498983 * vgo0_p1_p1[0]) + (-0.014073485 * vgo0_p1_p1[1]) + (-0.20587012 * vgo0_p1_p1[2]) + (0.06396779 * vgo0_p1_p1[3]));
        out3 = out3 + ((0.1262825 * vgo0_p1_p1[0]) + (0.027433183 * vgo0_p1_p1[1]) + (0.14469334 * vgo0_p1_p1[2]) + (0.011538011 * vgo0_p1_p1[3]));
    }
    {
        let sgo1_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo1_n1_n1 = relu_neg(sgo1_n1_n1);
        out0 = out0 + ((-0.038572453 * vgo1_n1_n1[0]) + (-0.023108613 * vgo1_n1_n1[1]) + (-0.039481267 * vgo1_n1_n1[2]) + (-0.012160024 * vgo1_n1_n1[3]));
        out1 = out1 + ((-0.004521989 * vgo1_n1_n1[0]) + (-0.028665857 * vgo1_n1_n1[1]) + (0.04295255 * vgo1_n1_n1[2]) + (0.10580258 * vgo1_n1_n1[3]));
        out2 = out2 + ((0.05439479 * vgo1_n1_n1[0]) + (-0.072261885 * vgo1_n1_n1[1]) + (0.11030243 * vgo1_n1_n1[2]) + (0.08934696 * vgo1_n1_n1[3]));
        out3 = out3 + ((0.09133867 * vgo1_n1_n1[0]) + (0.017547369 * vgo1_n1_n1[1]) + (0.097613186 * vgo1_n1_n1[2]) + (0.05491059 * vgo1_n1_n1[3]));
    }
    {
        let sgo1_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo1_n1_z = relu_neg(sgo1_n1_z);
        out0 = out0 + ((-0.09972817 * vgo1_n1_z[0]) + (0.057730395 * vgo1_n1_z[1]) + (0.12665828 * vgo1_n1_z[2]) + (0.32861367 * vgo1_n1_z[3]));
        out1 = out1 + ((-0.16186063 * vgo1_n1_z[0]) + (0.0745509 * vgo1_n1_z[1]) + (0.2394045 * vgo1_n1_z[2]) + (-0.08687853 * vgo1_n1_z[3]));
        out2 = out2 + ((-0.034404907 * vgo1_n1_z[0]) + (-0.05843572 * vgo1_n1_z[1]) + (0.0684561 * vgo1_n1_z[2]) + (-0.1355754 * vgo1_n1_z[3]));
        out3 = out3 + ((0.19248672 * vgo1_n1_z[0]) + (-0.60372186 * vgo1_n1_z[1]) + (0.12583947 * vgo1_n1_z[2]) + (0.4388962 * vgo1_n1_z[3]));
    }
    {
        let sgo1_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo1_n1_p1 = relu_neg(sgo1_n1_p1);
        out0 = out0 + ((0.10341107 * vgo1_n1_p1[0]) + (0.061113223 * vgo1_n1_p1[1]) + (0.08773817 * vgo1_n1_p1[2]) + (-0.082504354 * vgo1_n1_p1[3]));
        out1 = out1 + ((-0.16612078 * vgo1_n1_p1[0]) + (0.2681751 * vgo1_n1_p1[1]) + (0.019737698 * vgo1_n1_p1[2]) + (-0.17122322 * vgo1_n1_p1[3]));
        out2 = out2 + ((-0.135949 * vgo1_n1_p1[0]) + (0.3048101 * vgo1_n1_p1[1]) + (0.087803006 * vgo1_n1_p1[2]) + (0.11373851 * vgo1_n1_p1[3]));
        out3 = out3 + ((0.013192192 * vgo1_n1_p1[0]) + (-0.27022064 * vgo1_n1_p1[1]) + (0.35529897 * vgo1_n1_p1[2]) + (-0.15321451 * vgo1_n1_p1[3]));
    }
    {
        let sgo1_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo1_z_n1 = relu_neg(sgo1_z_n1);
        out0 = out0 + ((-0.032835662 * vgo1_z_n1[0]) + (0.11123062 * vgo1_z_n1[1]) + (-0.11322452 * vgo1_z_n1[2]) + (-0.17300649 * vgo1_z_n1[3]));
        out1 = out1 + ((0.04680824 * vgo1_z_n1[0]) + (0.12849288 * vgo1_z_n1[1]) + (0.17269878 * vgo1_z_n1[2]) + (-0.048671383 * vgo1_z_n1[3]));
        out2 = out2 + ((0.05189037 * vgo1_z_n1[0]) + (-0.009078046 * vgo1_z_n1[1]) + (0.22105052 * vgo1_z_n1[2]) + (0.013008137 * vgo1_z_n1[3]));
        out3 = out3 + ((-0.009738674 * vgo1_z_n1[0]) + (0.15391739 * vgo1_z_n1[1]) + (0.20969556 * vgo1_z_n1[2]) + (0.14189166 * vgo1_z_n1[3]));
    }
    {
        let sgo1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo1_z_z = relu_neg(sgo1_z_z);
        out0 = out0 + ((-0.47377753 * vgo1_z_z[0]) + (0.3038031 * vgo1_z_z[1]) + (0.18604809 * vgo1_z_z[2]) + (0.1931698 * vgo1_z_z[3]));
        out1 = out1 + ((-0.2964668 * vgo1_z_z[0]) + (-0.12287907 * vgo1_z_z[1]) + (-0.7107761 * vgo1_z_z[2]) + (0.26619422 * vgo1_z_z[3]));
        out2 = out2 + ((-0.33923018 * vgo1_z_z[0]) + (0.19200724 * vgo1_z_z[1]) + (0.013786281 * vgo1_z_z[2]) + (-0.17496964 * vgo1_z_z[3]));
        out3 = out3 + ((0.079325035 * vgo1_z_z[0]) + (-0.3694445 * vgo1_z_z[1]) + (0.0054486147 * vgo1_z_z[2]) + (-0.33018264 * vgo1_z_z[3]));
    }
    {
        let sgo1_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo1_z_p1 = relu_neg(sgo1_z_p1);
        out0 = out0 + ((0.14903802 * vgo1_z_p1[0]) + (-0.028043179 * vgo1_z_p1[1]) + (1.5238678e-05 * vgo1_z_p1[2]) + (0.021232028 * vgo1_z_p1[3]));
        out1 = out1 + ((0.16025065 * vgo1_z_p1[0]) + (0.14746875 * vgo1_z_p1[1]) + (-0.22831628 * vgo1_z_p1[2]) + (-0.12177345 * vgo1_z_p1[3]));
        out2 = out2 + ((0.038778774 * vgo1_z_p1[0]) + (0.32188168 * vgo1_z_p1[1]) + (-0.042017702 * vgo1_z_p1[2]) + (0.27155936 * vgo1_z_p1[3]));
        out3 = out3 + ((0.17920609 * vgo1_z_p1[0]) + (0.04099755 * vgo1_z_p1[1]) + (0.28527525 * vgo1_z_p1[2]) + (0.074623376 * vgo1_z_p1[3]));
    }
    {
        let sgo1_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo1_p1_n1 = relu_neg(sgo1_p1_n1);
        out0 = out0 + ((0.057019282 * vgo1_p1_n1[0]) + (-0.112741895 * vgo1_p1_n1[1]) + (0.030361209 * vgo1_p1_n1[2]) + (0.14567861 * vgo1_p1_n1[3]));
        out1 = out1 + ((0.056265317 * vgo1_p1_n1[0]) + (-0.01573537 * vgo1_p1_n1[1]) + (-0.06707608 * vgo1_p1_n1[2]) + (0.016657263 * vgo1_p1_n1[3]));
        out2 = out2 + ((0.09829025 * vgo1_p1_n1[0]) + (-0.026795063 * vgo1_p1_n1[1]) + (0.023042196 * vgo1_p1_n1[2]) + (0.09438241 * vgo1_p1_n1[3]));
        out3 = out3 + ((-0.025483066 * vgo1_p1_n1[0]) + (-0.052787006 * vgo1_p1_n1[1]) + (0.19730279 * vgo1_p1_n1[2]) + (0.021218104 * vgo1_p1_n1[3]));
    }
    {
        let sgo1_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo1_p1_z = relu_neg(sgo1_p1_z);
        out0 = out0 + ((0.19868211 * vgo1_p1_z[0]) + (-0.01531125 * vgo1_p1_z[1]) + (0.108596824 * vgo1_p1_z[2]) + (-0.035456363 * vgo1_p1_z[3]));
        out1 = out1 + ((0.0033609823 * vgo1_p1_z[0]) + (0.057961613 * vgo1_p1_z[1]) + (-0.013726211 * vgo1_p1_z[2]) + (0.101742364 * vgo1_p1_z[3]));
        out2 = out2 + ((0.33357215 * vgo1_p1_z[0]) + (0.14468077 * vgo1_p1_z[1]) + (0.29711527 * vgo1_p1_z[2]) + (-0.24662566 * vgo1_p1_z[3]));
        out3 = out3 + ((-0.119014986 * vgo1_p1_z[0]) + (-0.1899639 * vgo1_p1_z[1]) + (0.11246697 * vgo1_p1_z[2]) + (-0.0035374009 * vgo1_p1_z[3]));
    }
    {
        let sgo1_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo1_p1_p1 = relu_neg(sgo1_p1_p1);
        out0 = out0 + ((-0.05602109 * vgo1_p1_p1[0]) + (-0.15539522 * vgo1_p1_p1[1]) + (0.010730943 * vgo1_p1_p1[2]) + (0.057116497 * vgo1_p1_p1[3]));
        out1 = out1 + ((-0.02037749 * vgo1_p1_p1[0]) + (0.084210664 * vgo1_p1_p1[1]) + (-0.028235348 * vgo1_p1_p1[2]) + (0.10574697 * vgo1_p1_p1[3]));
        out2 = out2 + ((0.056925274 * vgo1_p1_p1[0]) + (0.07922333 * vgo1_p1_p1[1]) + (-0.090088 * vgo1_p1_p1[2]) + (0.1615985 * vgo1_p1_p1[3]));
        out3 = out3 + ((-0.0044301567 * vgo1_p1_p1[0]) + (-0.089945644 * vgo1_p1_p1[1]) + (0.024176618 * vgo1_p1_p1[2]) + (-0.041844133 * vgo1_p1_p1[3]));
    }
    out0 = out0 + (0.0015292584);
    out1 = out1 + (-0.043625206);
    out2 = out2 + (-0.09429898);
    out3 = out3 + (-0.06280405);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
