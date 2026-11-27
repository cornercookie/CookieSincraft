// Anime4K CNN x2 (M) — pass 2/9: conv2d_1_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x3x3x8
// GLSL BIND: conv2d_tf
// GLSL SAVE: conv2d_1_tf

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
        out0 = out0 + ((-0.08532478 * vgo0_n1_n1[0]) + (-0.14302494 * vgo0_n1_n1[1]) + (-0.017921071 * vgo0_n1_n1[2]) + (-0.0032664281 * vgo0_n1_n1[3]));
        out1 = out1 + ((-0.09841952 * vgo0_n1_n1[0]) + (0.024187077 * vgo0_n1_n1[1]) + (0.10701477 * vgo0_n1_n1[2]) + (0.14110753 * vgo0_n1_n1[3]));
        out2 = out2 + ((-0.05714981 * vgo0_n1_n1[0]) + (-0.10897174 * vgo0_n1_n1[1]) + (0.073803626 * vgo0_n1_n1[2]) + (0.103992954 * vgo0_n1_n1[3]));
        out3 = out3 + ((0.07914382 * vgo0_n1_n1[0]) + (0.032193683 * vgo0_n1_n1[1]) + (-0.18346278 * vgo0_n1_n1[2]) + (-0.09723936 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((-0.034482613 * vgo0_n1_z[0]) + (-0.10742312 * vgo0_n1_z[1]) + (-0.047286414 * vgo0_n1_z[2]) + (-0.08641124 * vgo0_n1_z[3]));
        out1 = out1 + ((-0.33896688 * vgo0_n1_z[0]) + (-0.036533825 * vgo0_n1_z[1]) + (-0.48337597 * vgo0_n1_z[2]) + (0.034040943 * vgo0_n1_z[3]));
        out2 = out2 + ((-0.13598205 * vgo0_n1_z[0]) + (-0.080917805 * vgo0_n1_z[1]) + (0.08540263 * vgo0_n1_z[2]) + (-0.012667689 * vgo0_n1_z[3]));
        out3 = out3 + ((-0.009171425 * vgo0_n1_z[0]) + (-0.120026454 * vgo0_n1_z[1]) + (-0.20536867 * vgo0_n1_z[2]) + (-0.032149274 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((0.18687321 * vgo0_n1_p1[0]) + (0.066278316 * vgo0_n1_p1[1]) + (0.024327392 * vgo0_n1_p1[2]) + (0.08816582 * vgo0_n1_p1[3]));
        out1 = out1 + ((-0.08017908 * vgo0_n1_p1[0]) + (0.09488853 * vgo0_n1_p1[1]) + (0.26018232 * vgo0_n1_p1[2]) + (-0.101504356 * vgo0_n1_p1[3]));
        out2 = out2 + ((0.17487666 * vgo0_n1_p1[0]) + (0.31057635 * vgo0_n1_p1[1]) + (0.14785016 * vgo0_n1_p1[2]) + (-0.09622089 * vgo0_n1_p1[3]));
        out3 = out3 + ((-0.07537452 * vgo0_n1_p1[0]) + (-0.13844088 * vgo0_n1_p1[1]) + (-0.05810814 * vgo0_n1_p1[2]) + (0.09907489 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((-0.04183032 * vgo0_z_n1[0]) + (0.15207712 * vgo0_z_n1[1]) + (0.005002397 * vgo0_z_n1[2]) + (0.32277516 * vgo0_z_n1[3]));
        out1 = out1 + ((-0.16169126 * vgo0_z_n1[0]) + (-0.119836345 * vgo0_z_n1[1]) + (-0.04068436 * vgo0_z_n1[2]) + (-0.096728764 * vgo0_z_n1[3]));
        out2 = out2 + ((0.11943901 * vgo0_z_n1[0]) + (0.1789597 * vgo0_z_n1[1]) + (-0.20412198 * vgo0_z_n1[2]) + (0.19009817 * vgo0_z_n1[3]));
        out3 = out3 + ((0.36630696 * vgo0_z_n1[0]) + (0.06946421 * vgo0_z_n1[1]) + (-0.5254373 * vgo0_z_n1[2]) + (-0.11896399 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((-0.31916487 * vgo0_z_z[0]) + (-0.98911583 * vgo0_z_z[1]) + (1.0728644 * vgo0_z_z[2]) + (-0.39280394 * vgo0_z_z[3]));
        out1 = out1 + ((0.33458877 * vgo0_z_z[0]) + (-0.17325239 * vgo0_z_z[1]) + (-0.645045 * vgo0_z_z[2]) + (-0.28524077 * vgo0_z_z[3]));
        out2 = out2 + ((-0.14512783 * vgo0_z_z[0]) + (0.24996442 * vgo0_z_z[1]) + (-0.09837877 * vgo0_z_z[2]) + (0.05468934 * vgo0_z_z[3]));
        out3 = out3 + ((0.31559715 * vgo0_z_z[0]) + (-0.020504637 * vgo0_z_z[1]) + (-0.026724018 * vgo0_z_z[2]) + (0.24507573 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((-0.23759829 * vgo0_z_p1[0]) + (-0.08530173 * vgo0_z_p1[1]) + (-0.16665787 * vgo0_z_p1[2]) + (-0.22463752 * vgo0_z_p1[3]));
        out1 = out1 + ((0.109896734 * vgo0_z_p1[0]) + (0.13446991 * vgo0_z_p1[1]) + (-0.049552456 * vgo0_z_p1[2]) + (-0.02385489 * vgo0_z_p1[3]));
        out2 = out2 + ((-0.01245375 * vgo0_z_p1[0]) + (0.3833208 * vgo0_z_p1[1]) + (0.05758832 * vgo0_z_p1[2]) + (0.1528937 * vgo0_z_p1[3]));
        out3 = out3 + ((0.0501858 * vgo0_z_p1[0]) + (-0.19651426 * vgo0_z_p1[1]) + (0.0076587177 * vgo0_z_p1[2]) + (-0.03297025 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((0.14554465 * vgo0_p1_n1[0]) + (-0.01826686 * vgo0_p1_n1[1]) + (0.10284085 * vgo0_p1_n1[2]) + (-0.19152659 * vgo0_p1_n1[3]));
        out1 = out1 + ((-0.017585073 * vgo0_p1_n1[0]) + (-0.05511482 * vgo0_p1_n1[1]) + (0.06362406 * vgo0_p1_n1[2]) + (0.023924058 * vgo0_p1_n1[3]));
        out2 = out2 + ((-0.0018977845 * vgo0_p1_n1[0]) + (-0.103172876 * vgo0_p1_n1[1]) + (0.03287086 * vgo0_p1_n1[2]) + (-0.20085956 * vgo0_p1_n1[3]));
        out3 = out3 + ((0.36062446 * vgo0_p1_n1[0]) + (0.10749464 * vgo0_p1_n1[1]) + (-0.20984372 * vgo0_p1_n1[2]) + (0.018256644 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((-0.005534592 * vgo0_p1_z[0]) + (0.3709197 * vgo0_p1_z[1]) + (-0.18287498 * vgo0_p1_z[2]) + (0.1720451 * vgo0_p1_z[3]));
        out1 = out1 + ((0.030155553 * vgo0_p1_z[0]) + (-0.023265475 * vgo0_p1_z[1]) + (0.0058617783 * vgo0_p1_z[2]) + (-0.031765483 * vgo0_p1_z[3]));
        out2 = out2 + ((0.037328955 * vgo0_p1_z[0]) + (-0.2730994 * vgo0_p1_z[1]) + (0.35090837 * vgo0_p1_z[2]) + (-0.3269043 * vgo0_p1_z[3]));
        out3 = out3 + ((-0.028477207 * vgo0_p1_z[0]) + (0.32756507 * vgo0_p1_z[1]) + (-0.15989502 * vgo0_p1_z[2]) + (0.12158258 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((0.10873739 * vgo0_p1_p1[0]) + (0.19583772 * vgo0_p1_p1[1]) + (0.060394943 * vgo0_p1_p1[2]) + (0.09410379 * vgo0_p1_p1[3]));
        out1 = out1 + ((-0.04739245 * vgo0_p1_p1[0]) + (0.026561242 * vgo0_p1_p1[1]) + (0.022990001 * vgo0_p1_p1[2]) + (0.1093272 * vgo0_p1_p1[3]));
        out2 = out2 + ((-0.01071349 * vgo0_p1_p1[0]) + (-0.022938967 * vgo0_p1_p1[1]) + (-0.046423864 * vgo0_p1_p1[2]) + (0.2385325 * vgo0_p1_p1[3]));
        out3 = out3 + ((-0.0319821 * vgo0_p1_p1[0]) + (0.046962265 * vgo0_p1_p1[1]) + (0.09081178 * vgo0_p1_p1[2]) + (-0.11001857 * vgo0_p1_p1[3]));
    }
    {
        let sgo1_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo1_n1_n1 = relu_neg(sgo1_n1_n1);
        out0 = out0 + ((0.13012704 * vgo1_n1_n1[0]) + (0.112289295 * vgo1_n1_n1[1]) + (0.030790284 * vgo1_n1_n1[2]) + (-0.050499484 * vgo1_n1_n1[3]));
        out1 = out1 + ((0.11784853 * vgo1_n1_n1[0]) + (0.08107028 * vgo1_n1_n1[1]) + (-0.07556717 * vgo1_n1_n1[2]) + (-0.15643 * vgo1_n1_n1[3]));
        out2 = out2 + ((0.015249331 * vgo1_n1_n1[0]) + (0.015299608 * vgo1_n1_n1[1]) + (0.07748125 * vgo1_n1_n1[2]) + (0.054485757 * vgo1_n1_n1[3]));
        out3 = out3 + ((0.044857923 * vgo1_n1_n1[0]) + (0.12161275 * vgo1_n1_n1[1]) + (-0.048292994 * vgo1_n1_n1[2]) + (-0.033995003 * vgo1_n1_n1[3]));
    }
    {
        let sgo1_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo1_n1_z = relu_neg(sgo1_n1_z);
        out0 = out0 + ((0.12931514 * vgo1_n1_z[0]) + (0.15114146 * vgo1_n1_z[1]) + (0.070513315 * vgo1_n1_z[2]) + (0.11246343 * vgo1_n1_z[3]));
        out1 = out1 + ((0.4142387 * vgo1_n1_z[0]) + (0.213479 * vgo1_n1_z[1]) + (-0.5439916 * vgo1_n1_z[2]) + (0.07776645 * vgo1_n1_z[3]));
        out2 = out2 + ((0.13109331 * vgo1_n1_z[0]) + (0.2021147 * vgo1_n1_z[1]) + (0.25932786 * vgo1_n1_z[2]) + (-0.22157331 * vgo1_n1_z[3]));
        out3 = out3 + ((0.02377734 * vgo1_n1_z[0]) + (-0.014970623 * vgo1_n1_z[1]) + (-0.1943276 * vgo1_n1_z[2]) + (0.18440372 * vgo1_n1_z[3]));
    }
    {
        let sgo1_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo1_n1_p1 = relu_neg(sgo1_n1_p1);
        out0 = out0 + ((-0.22365458 * vgo1_n1_p1[0]) + (-0.19829084 * vgo1_n1_p1[1]) + (-0.06881161 * vgo1_n1_p1[2]) + (-0.06468993 * vgo1_n1_p1[3]));
        out1 = out1 + ((0.17202774 * vgo1_n1_p1[0]) + (0.0048758537 * vgo1_n1_p1[1]) + (-0.09235021 * vgo1_n1_p1[2]) + (0.18941896 * vgo1_n1_p1[3]));
        out2 = out2 + ((0.064125344 * vgo1_n1_p1[0]) + (-0.09067088 * vgo1_n1_p1[1]) + (0.09748182 * vgo1_n1_p1[2]) + (0.13561936 * vgo1_n1_p1[3]));
        out3 = out3 + ((-0.05876288 * vgo1_n1_p1[0]) + (-0.0122420965 * vgo1_n1_p1[1]) + (-0.054380875 * vgo1_n1_p1[2]) + (-0.17743628 * vgo1_n1_p1[3]));
    }
    {
        let sgo1_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo1_z_n1 = relu_neg(sgo1_z_n1);
        out0 = out0 + ((0.18582906 * vgo1_z_n1[0]) + (-0.09263032 * vgo1_z_n1[1]) + (-0.08210888 * vgo1_z_n1[2]) + (-0.20515606 * vgo1_z_n1[3]));
        out1 = out1 + ((0.11484005 * vgo1_z_n1[0]) + (0.08557595 * vgo1_z_n1[1]) + (0.0009253741 * vgo1_z_n1[2]) + (-0.051202174 * vgo1_z_n1[3]));
        out2 = out2 + ((-0.18535301 * vgo1_z_n1[0]) + (-0.1529345 * vgo1_z_n1[1]) + (-0.13092944 * vgo1_z_n1[2]) + (0.03770747 * vgo1_z_n1[3]));
        out3 = out3 + ((-0.020947013 * vgo1_z_n1[0]) + (0.19187425 * vgo1_z_n1[1]) + (-0.15494856 * vgo1_z_n1[2]) + (-0.048979875 * vgo1_z_n1[3]));
    }
    {
        let sgo1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo1_z_z = relu_neg(sgo1_z_z);
        out0 = out0 + ((-0.38131633 * vgo1_z_z[0]) + (0.4278787 * vgo1_z_z[1]) + (0.19763695 * vgo1_z_z[2]) + (0.27655518 * vgo1_z_z[3]));
        out1 = out1 + ((-0.08711912 * vgo1_z_z[0]) + (0.07374453 * vgo1_z_z[1]) + (-0.064803004 * vgo1_z_z[2]) + (0.5983854 * vgo1_z_z[3]));
        out2 = out2 + ((0.2361923 * vgo1_z_z[0]) + (-0.057221692 * vgo1_z_z[1]) + (-0.37138999 * vgo1_z_z[2]) + (-0.24259573 * vgo1_z_z[3]));
        out3 = out3 + ((0.13890724 * vgo1_z_z[0]) + (0.25706333 * vgo1_z_z[1]) + (-0.54021406 * vgo1_z_z[2]) + (0.08095518 * vgo1_z_z[3]));
    }
    {
        let sgo1_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo1_z_p1 = relu_neg(sgo1_z_p1);
        out0 = out0 + ((0.0991328 * vgo1_z_p1[0]) + (-0.022651536 * vgo1_z_p1[1]) + (-0.029148921 * vgo1_z_p1[2]) + (-0.009812537 * vgo1_z_p1[3]));
        out1 = out1 + ((-0.09523686 * vgo1_z_p1[0]) + (-0.15704902 * vgo1_z_p1[1]) + (0.052389514 * vgo1_z_p1[2]) + (0.21561539 * vgo1_z_p1[3]));
        out2 = out2 + ((0.1950314 * vgo1_z_p1[0]) + (-0.08572602 * vgo1_z_p1[1]) + (0.0016523858 * vgo1_z_p1[2]) + (0.14125621 * vgo1_z_p1[3]));
        out3 = out3 + ((-0.030999828 * vgo1_z_p1[0]) + (0.12009709 * vgo1_z_p1[1]) + (0.0373512 * vgo1_z_p1[2]) + (-0.105043754 * vgo1_z_p1[3]));
    }
    {
        let sgo1_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo1_p1_n1 = relu_neg(sgo1_p1_n1);
        out0 = out0 + ((-0.11251988 * vgo1_p1_n1[0]) + (0.12106985 * vgo1_p1_n1[1]) + (0.011923068 * vgo1_p1_n1[2]) + (0.3662747 * vgo1_p1_n1[3]));
        out1 = out1 + ((0.004800994 * vgo1_p1_n1[0]) + (0.017972551 * vgo1_p1_n1[1]) + (0.004761366 * vgo1_p1_n1[2]) + (-0.07934206 * vgo1_p1_n1[3]));
        out2 = out2 + ((-0.13755941 * vgo1_p1_n1[0]) + (-0.022852683 * vgo1_p1_n1[1]) + (0.1502225 * vgo1_p1_n1[2]) + (0.009758547 * vgo1_p1_n1[3]));
        out3 = out3 + ((-0.16964264 * vgo1_p1_n1[0]) + (0.00984782 * vgo1_p1_n1[1]) + (0.07855833 * vgo1_p1_n1[2]) + (0.035730787 * vgo1_p1_n1[3]));
    }
    {
        let sgo1_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo1_p1_z = relu_neg(sgo1_p1_z);
        out0 = out0 + ((0.01964957 * vgo1_p1_z[0]) + (-0.27226487 * vgo1_p1_z[1]) + (0.033933397 * vgo1_p1_z[2]) + (-0.117632054 * vgo1_p1_z[3]));
        out1 = out1 + ((-0.009058229 * vgo1_p1_z[0]) + (0.047830686 * vgo1_p1_z[1]) + (-0.01125145 * vgo1_p1_z[2]) + (0.136628 * vgo1_p1_z[3]));
        out2 = out2 + ((0.0056388285 * vgo1_p1_z[0]) + (0.3028781 * vgo1_p1_z[1]) + (-0.12286517 * vgo1_p1_z[2]) + (0.23498532 * vgo1_p1_z[3]));
        out3 = out3 + ((-0.009319075 * vgo1_p1_z[0]) + (-0.444048 * vgo1_p1_z[1]) + (0.16174883 * vgo1_p1_z[2]) + (-0.06367683 * vgo1_p1_z[3]));
    }
    {
        let sgo1_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo1_p1_p1 = relu_neg(sgo1_p1_p1);
        out0 = out0 + ((0.02343933 * vgo1_p1_p1[0]) + (-0.010915871 * vgo1_p1_p1[1]) + (-0.058680378 * vgo1_p1_p1[2]) + (-0.21886891 * vgo1_p1_p1[3]));
        out1 = out1 + ((-0.010750894 * vgo1_p1_p1[0]) + (-0.06671997 * vgo1_p1_p1[1]) + (0.0602906 * vgo1_p1_p1[2]) + (-0.07903071 * vgo1_p1_p1[3]));
        out2 = out2 + ((0.066891186 * vgo1_p1_p1[0]) + (0.06650588 * vgo1_p1_p1[1]) + (0.14362891 * vgo1_p1_p1[2]) + (-0.101870626 * vgo1_p1_p1[3]));
        out3 = out3 + ((0.02264628 * vgo1_p1_p1[0]) + (-0.06940821 * vgo1_p1_p1[1]) + (-0.077616625 * vgo1_p1_p1[2]) + (0.110911585 * vgo1_p1_p1[3]));
    }
    out0 = out0 + (0.032014452);
    out1 = out1 + (-0.020821465);
    out2 = out2 + (0.0826416);
    out3 = out3 + (-0.002838458);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
