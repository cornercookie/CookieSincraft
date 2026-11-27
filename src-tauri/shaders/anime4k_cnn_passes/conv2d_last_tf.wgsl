// Anime4K CNN x2 (M) — pass 8/9: conv2d_last_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Depth-to-Space
// GLSL BIND: conv2d_tf, conv2d_1_tf, conv2d_2_tf, conv2d_3_tf, conv2d_4_tf, conv2d_5_tf, conv2d_6_tf
// GLSL SAVE: conv2d_last_tf

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    _pad: f32,
}

@group(0) @binding(0) var source0: texture_2d<f32>;
@group(0) @binding(1) var source1: texture_2d<f32>;
@group(0) @binding(2) var source2: texture_2d<f32>;
@group(0) @binding(3) var source3: texture_2d<f32>;
@group(0) @binding(4) var source4: texture_2d<f32>;
@group(0) @binding(5) var source5: texture_2d<f32>;
@group(0) @binding(6) var source6: texture_2d<f32>;
@group(0) @binding(7) var smp: sampler;
@group(0) @binding(8) var dest: texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var<uniform> params: Params;

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
        let sg0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg0_z_z = relu_pos(sg0_z_z);
        out0 = out0 + ((-0.0067711817 * vg0_z_z[0]) + (0.08160003 * vg0_z_z[1]) + (0.0247279 * vg0_z_z[2]) + (0.03084815 * vg0_z_z[3]));
        out1 = out1 + ((-0.026977416 * vg0_z_z[0]) + (-0.02120602 * vg0_z_z[1]) + (-0.025078611 * vg0_z_z[2]) + (-0.029852165 * vg0_z_z[3]));
        out2 = out2 + ((-0.011627478 * vg0_z_z[0]) + (-0.012742972 * vg0_z_z[1]) + (0.022736797 * vg0_z_z[2]) + (-0.0028815821 * vg0_z_z[3]));
        out3 = out3 + ((-0.007515677 * vg0_z_z[0]) + (0.0172887 * vg0_z_z[1]) + (-0.023259213 * vg0_z_z[2]) + (0.009608947 * vg0_z_z[3]));
    }
    {
        let sg1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg1_z_z = relu_neg(sg1_z_z);
        out0 = out0 + ((-0.028660107 * vg1_z_z[0]) + (-0.014015208 * vg1_z_z[1]) + (-0.027838672 * vg1_z_z[2]) + (-0.013171922 * vg1_z_z[3]));
        out1 = out1 + ((0.0029435428 * vg1_z_z[0]) + (0.027047642 * vg1_z_z[1]) + (-0.017478354 * vg1_z_z[2]) + (0.022834882 * vg1_z_z[3]));
        out2 = out2 + ((-0.037572853 * vg1_z_z[0]) + (-0.0034044068 * vg1_z_z[1]) + (-0.0149029335 * vg1_z_z[2]) + (-0.013362301 * vg1_z_z[3]));
        out3 = out3 + ((0.009827443 * vg1_z_z[0]) + (-0.015742151 * vg1_z_z[1]) + (-0.0074795415 * vg1_z_z[2]) + (-0.0022266617 * vg1_z_z[3]));
    }
    {
        let sg2_z_z = textureLoad(source1,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg2_z_z = relu_pos(sg2_z_z);
        out0 = out0 + ((-0.07579662 * vg2_z_z[0]) + (-0.039754186 * vg2_z_z[1]) + (-0.066026606 * vg2_z_z[2]) + (-0.046816852 * vg2_z_z[3]));
        out1 = out1 + ((0.1099032 * vg2_z_z[0]) + (0.043956704 * vg2_z_z[1]) + (0.073109835 * vg2_z_z[2]) + (0.04680284 * vg2_z_z[3]));
        out2 = out2 + ((-0.06896613 * vg2_z_z[0]) + (-0.008838632 * vg2_z_z[1]) + (-0.044584926 * vg2_z_z[2]) + (-0.01319039 * vg2_z_z[3]));
        out3 = out3 + ((-0.0021152915 * vg2_z_z[0]) + (-0.04503326 * vg2_z_z[1]) + (0.027061926 * vg2_z_z[2]) + (-0.028334105 * vg2_z_z[3]));
    }
    {
        let sg3_z_z = textureLoad(source1,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg3_z_z = relu_neg(sg3_z_z);
        out0 = out0 + ((0.15458213 * vg3_z_z[0]) + (0.059769996 * vg3_z_z[1]) + (0.09327123 * vg3_z_z[2]) + (-0.028782733 * vg3_z_z[3]));
        out1 = out1 + ((0.023459995 * vg3_z_z[0]) + (-0.15390377 * vg3_z_z[1]) + (-0.13432898 * vg3_z_z[2]) + (-0.1127775 * vg3_z_z[3]));
        out2 = out2 + ((0.072764635 * vg3_z_z[0]) + (-0.0020463336 * vg3_z_z[1]) + (0.034736466 * vg3_z_z[2]) + (-0.0012086042 * vg3_z_z[3]));
        out3 = out3 + ((-0.05847183 * vg3_z_z[0]) + (-0.029952323 * vg3_z_z[1]) + (0.052969377 * vg3_z_z[2]) + (0.09590908 * vg3_z_z[3]));
    }
    {
        let sg4_z_z = textureLoad(source2,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg4_z_z = relu_pos(sg4_z_z);
        out0 = out0 + ((-0.07476772 * vg4_z_z[0]) + (-0.016574614 * vg4_z_z[1]) + (0.04131183 * vg4_z_z[2]) + (0.017335678 * vg4_z_z[3]));
        out1 = out1 + ((0.009654406 * vg4_z_z[0]) + (0.072183535 * vg4_z_z[1]) + (-0.002266456 * vg4_z_z[2]) + (0.086873695 * vg4_z_z[3]));
        out2 = out2 + ((9.310129e-05 * vg4_z_z[0]) + (0.0056416965 * vg4_z_z[1]) + (-0.004188391 * vg4_z_z[2]) + (0.023132093 * vg4_z_z[3]));
        out3 = out3 + ((-0.05183336 * vg4_z_z[0]) + (-0.025825873 * vg4_z_z[1]) + (-0.03684392 * vg4_z_z[2]) + (-0.0075729224 * vg4_z_z[3]));
    }
    {
        let sg5_z_z = textureLoad(source2,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg5_z_z = relu_neg(sg5_z_z);
        out0 = out0 + ((0.00878842 * vg5_z_z[0]) + (0.03869637 * vg5_z_z[1]) + (-0.035759524 * vg5_z_z[2]) + (0.003345386 * vg5_z_z[3]));
        out1 = out1 + ((-0.064184256 * vg5_z_z[0]) + (-0.034568302 * vg5_z_z[1]) + (-0.06672922 * vg5_z_z[2]) + (-0.0686381 * vg5_z_z[3]));
        out2 = out2 + ((-0.06794392 * vg5_z_z[0]) + (-0.10685906 * vg5_z_z[1]) + (0.04679947 * vg5_z_z[2]) + (-0.012535639 * vg5_z_z[3]));
        out3 = out3 + ((0.006932529 * vg5_z_z[0]) + (-0.007783515 * vg5_z_z[1]) + (0.109123886 * vg5_z_z[2]) + (0.13804391 * vg5_z_z[3]));
    }
    {
        let sg6_z_z = textureLoad(source3,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg6_z_z = relu_pos(sg6_z_z);
        out0 = out0 + ((-0.03160699 * vg6_z_z[0]) + (0.050473 * vg6_z_z[1]) + (-0.09030729 * vg6_z_z[2]) + (0.0649397 * vg6_z_z[3]));
        out1 = out1 + ((0.11466501 * vg6_z_z[0]) + (0.17912874 * vg6_z_z[1]) + (-0.0081851315 * vg6_z_z[2]) + (0.052244574 * vg6_z_z[3]));
        out2 = out2 + ((0.051632743 * vg6_z_z[0]) + (0.061941486 * vg6_z_z[1]) + (0.06546816 * vg6_z_z[2]) + (0.12174249 * vg6_z_z[3]));
        out3 = out3 + ((-0.05104755 * vg6_z_z[0]) + (-0.018193979 * vg6_z_z[1]) + (-0.032196652 * vg6_z_z[2]) + (-0.035292786 * vg6_z_z[3]));
    }
    {
        let sg7_z_z = textureLoad(source3,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg7_z_z = relu_neg(sg7_z_z);
        out0 = out0 + ((0.013612735 * vg7_z_z[0]) + (-0.0024100312 * vg7_z_z[1]) + (-0.068611205 * vg7_z_z[2]) + (-0.07369285 * vg7_z_z[3]));
        out1 = out1 + ((-0.019647537 * vg7_z_z[0]) + (-0.066944756 * vg7_z_z[1]) + (-0.010012875 * vg7_z_z[2]) + (-0.06785739 * vg7_z_z[3]));
        out2 = out2 + ((-0.062246565 * vg7_z_z[0]) + (-0.087313406 * vg7_z_z[1]) + (-0.044278186 * vg7_z_z[2]) + (-0.09368995 * vg7_z_z[3]));
        out3 = out3 + ((0.052555013 * vg7_z_z[0]) + (0.13604961 * vg7_z_z[1]) + (0.05645059 * vg7_z_z[2]) + (0.08763303 * vg7_z_z[3]));
    }
    {
        let sg8_z_z = textureLoad(source4,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg8_z_z = relu_pos(sg8_z_z);
        out0 = out0 + ((0.04218486 * vg8_z_z[0]) + (-0.05028401 * vg8_z_z[1]) + (0.059086576 * vg8_z_z[2]) + (-0.03545452 * vg8_z_z[3]));
        out1 = out1 + ((0.027737848 * vg8_z_z[0]) + (0.0043074046 * vg8_z_z[1]) + (0.0011001764 * vg8_z_z[2]) + (-0.073026665 * vg8_z_z[3]));
        out2 = out2 + ((-0.04094988 * vg8_z_z[0]) + (0.044061556 * vg8_z_z[1]) + (-0.009812515 * vg8_z_z[2]) + (0.06841999 * vg8_z_z[3]));
        out3 = out3 + ((-0.06612581 * vg8_z_z[0]) + (0.037223976 * vg8_z_z[1]) + (-0.07759491 * vg8_z_z[2]) + (-0.04356598 * vg8_z_z[3]));
    }
    {
        let sg9_z_z = textureLoad(source4,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg9_z_z = relu_neg(sg9_z_z);
        out0 = out0 + ((-0.027558247 * vg9_z_z[0]) + (0.014248466 * vg9_z_z[1]) + (-0.019813016 * vg9_z_z[2]) + (-0.058107473 * vg9_z_z[3]));
        out1 = out1 + ((-0.016717663 * vg9_z_z[0]) + (-0.020424338 * vg9_z_z[1]) + (0.0053625097 * vg9_z_z[2]) + (-0.009917319 * vg9_z_z[3]));
        out2 = out2 + ((0.013678771 * vg9_z_z[0]) + (0.0113340765 * vg9_z_z[1]) + (0.0061787106 * vg9_z_z[2]) + (-0.036083996 * vg9_z_z[3]));
        out3 = out3 + ((-0.020179711 * vg9_z_z[0]) + (-0.011310535 * vg9_z_z[1]) + (0.054827053 * vg9_z_z[2]) + (-0.0008278952 * vg9_z_z[3]));
    }
    {
        let sg10_z_z = textureLoad(source5,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg10_z_z = relu_pos(sg10_z_z);
        out0 = out0 + ((0.028690035 * vg10_z_z[0]) + (-0.012079616 * vg10_z_z[1]) + (0.11931408 * vg10_z_z[2]) + (-0.048533775 * vg10_z_z[3]));
        out1 = out1 + ((0.069336995 * vg10_z_z[0]) + (0.0049852817 * vg10_z_z[1]) + (0.013774468 * vg10_z_z[2]) + (0.035233382 * vg10_z_z[3]));
        out2 = out2 + ((-0.07384821 * vg10_z_z[0]) + (0.0003354423 * vg10_z_z[1]) + (-0.0059171803 * vg10_z_z[2]) + (-0.04503906 * vg10_z_z[3]));
        out3 = out3 + ((0.08727279 * vg10_z_z[0]) + (0.005138857 * vg10_z_z[1]) + (-0.17724465 * vg10_z_z[2]) + (0.055782065 * vg10_z_z[3]));
    }
    {
        let sg11_z_z = textureLoad(source5,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg11_z_z = relu_neg(sg11_z_z);
        out0 = out0 + ((-0.20744391 * vg11_z_z[0]) + (0.24348328 * vg11_z_z[1]) + (-0.3145766 * vg11_z_z[2]) + (0.17026486 * vg11_z_z[3]));
        out1 = out1 + ((-0.022870807 * vg11_z_z[0]) + (-0.01648648 * vg11_z_z[1]) + (-0.05912279 * vg11_z_z[2]) + (-0.012555373 * vg11_z_z[3]));
        out2 = out2 + ((-0.066004686 * vg11_z_z[0]) + (0.03182394 * vg11_z_z[1]) + (0.16285324 * vg11_z_z[2]) + (-0.1221846 * vg11_z_z[3]));
        out3 = out3 + ((-0.31816196 * vg11_z_z[0]) + (0.007928748 * vg11_z_z[1]) + (0.43180224 * vg11_z_z[2]) + (-0.015949022 * vg11_z_z[3]));
    }
    {
        let sg12_z_z = textureLoad(source6,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg12_z_z = relu_pos(sg12_z_z);
        out0 = out0 + ((0.16363169 * vg12_z_z[0]) + (0.14781676 * vg12_z_z[1]) + (-0.2377973 * vg12_z_z[2]) + (-0.1571377 * vg12_z_z[3]));
        out1 = out1 + ((-0.09038187 * vg12_z_z[0]) + (0.0046504294 * vg12_z_z[1]) + (0.033955004 * vg12_z_z[2]) + (-0.051421452 * vg12_z_z[3]));
        out2 = out2 + ((0.046735536 * vg12_z_z[0]) + (0.006827522 * vg12_z_z[1]) + (-0.121338 * vg12_z_z[2]) + (0.12671822 * vg12_z_z[3]));
        out3 = out3 + ((0.15833299 * vg12_z_z[0]) + (-0.1858712 * vg12_z_z[1]) + (-0.1942371 * vg12_z_z[2]) + (0.17336044 * vg12_z_z[3]));
    }
    {
        let sg13_z_z = textureLoad(source6,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vg13_z_z = relu_neg(sg13_z_z);
        out0 = out0 + ((-0.018145572 * vg13_z_z[0]) + (-0.015550516 * vg13_z_z[1]) + (0.044410378 * vg13_z_z[2]) + (0.046016492 * vg13_z_z[3]));
        out1 = out1 + ((0.084021375 * vg13_z_z[0]) + (0.05327457 * vg13_z_z[1]) + (-0.008270992 * vg13_z_z[2]) + (-0.045435544 * vg13_z_z[3]));
        out2 = out2 + ((0.07185879 * vg13_z_z[0]) + (-0.131923 * vg13_z_z[1]) + (0.26721445 * vg13_z_z[2]) + (-0.26745328 * vg13_z_z[3]));
        out3 = out3 + ((-0.07093472 * vg13_z_z[0]) + (0.042701527 * vg13_z_z[1]) + (0.13793674 * vg13_z_z[2]) + (-0.095621444 * vg13_z_z[3]));
    }
    out0 = out0 + (0.016836504);
    out1 = out1 + (0.010161949);
    out2 = out2 + (0.021351453);
    out3 = out3 + (0.01278978);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
