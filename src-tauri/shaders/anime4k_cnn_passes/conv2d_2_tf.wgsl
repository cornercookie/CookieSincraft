// Anime4K CNN x2 (M) — pass 3/9: conv2d_2_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x3x3x8
// GLSL BIND: conv2d_1_tf
// GLSL SAVE: conv2d_2_tf

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
        out0 = out0 + ((-0.06963679 * vgo0_n1_n1[0]) + (-0.07560548 * vgo0_n1_n1[1]) + (-0.069522075 * vgo0_n1_n1[2]) + (0.0038078027 * vgo0_n1_n1[3]));
        out1 = out1 + ((-0.08002613 * vgo0_n1_n1[0]) + (0.13671301 * vgo0_n1_n1[1]) + (0.084461786 * vgo0_n1_n1[2]) + (-0.039376218 * vgo0_n1_n1[3]));
        out2 = out2 + ((0.19136548 * vgo0_n1_n1[0]) + (-0.123174496 * vgo0_n1_n1[1]) + (0.26566333 * vgo0_n1_n1[2]) + (-0.16583005 * vgo0_n1_n1[3]));
        out3 = out3 + ((-0.18664864 * vgo0_n1_n1[0]) + (-0.023539122 * vgo0_n1_n1[1]) + (-0.21928434 * vgo0_n1_n1[2]) + (-0.026818147 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((0.16660932 * vgo0_n1_z[0]) + (-0.18558703 * vgo0_n1_z[1]) + (0.37230486 * vgo0_n1_z[2]) + (0.118128106 * vgo0_n1_z[3]));
        out1 = out1 + ((-0.14098641 * vgo0_n1_z[0]) + (0.14659132 * vgo0_n1_z[1]) + (-0.22217897 * vgo0_n1_z[2]) + (0.12952235 * vgo0_n1_z[3]));
        out2 = out2 + ((-0.4139033 * vgo0_n1_z[0]) + (-0.04308319 * vgo0_n1_z[1]) + (0.12885277 * vgo0_n1_z[2]) + (-0.17986743 * vgo0_n1_z[3]));
        out3 = out3 + ((-0.23556231 * vgo0_n1_z[0]) + (-0.08351981 * vgo0_n1_z[1]) + (-0.43240538 * vgo0_n1_z[2]) + (0.019033253 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((-0.18008037 * vgo0_n1_p1[0]) + (-0.04448665 * vgo0_n1_p1[1]) + (0.011906908 * vgo0_n1_p1[2]) + (-0.023056917 * vgo0_n1_p1[3]));
        out1 = out1 + ((0.18136618 * vgo0_n1_p1[0]) + (-0.04723555 * vgo0_n1_p1[1]) + (-0.0050158803 * vgo0_n1_p1[2]) + (-0.14823224 * vgo0_n1_p1[3]));
        out2 = out2 + ((-0.2105281 * vgo0_n1_p1[0]) + (0.023047728 * vgo0_n1_p1[1]) + (-0.14040631 * vgo0_n1_p1[2]) + (-0.03178526 * vgo0_n1_p1[3]));
        out3 = out3 + ((-0.13477588 * vgo0_n1_p1[0]) + (-0.01820428 * vgo0_n1_p1[1]) + (0.058358394 * vgo0_n1_p1[2]) + (0.23792502 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((0.07363309 * vgo0_z_n1[0]) + (-0.061728477 * vgo0_z_n1[1]) + (0.03573137 * vgo0_z_n1[2]) + (-0.0050971056 * vgo0_z_n1[3]));
        out1 = out1 + ((-0.012813505 * vgo0_z_n1[0]) + (-0.17236637 * vgo0_z_n1[1]) + (0.1697835 * vgo0_z_n1[2]) + (0.055788577 * vgo0_z_n1[3]));
        out2 = out2 + ((-0.22263195 * vgo0_z_n1[0]) + (0.10324512 * vgo0_z_n1[1]) + (0.58971673 * vgo0_z_n1[2]) + (-0.4872246 * vgo0_z_n1[3]));
        out3 = out3 + ((-0.1555681 * vgo0_z_n1[0]) + (0.032747746 * vgo0_z_n1[1]) + (-0.096495196 * vgo0_z_n1[2]) + (0.070196226 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((0.14174286 * vgo0_z_z[0]) + (0.099460006 * vgo0_z_z[1]) + (-0.088765986 * vgo0_z_z[2]) + (0.58350676 * vgo0_z_z[3]));
        out1 = out1 + ((-0.025177564 * vgo0_z_z[0]) + (-0.46004987 * vgo0_z_z[1]) + (0.37007022 * vgo0_z_z[2]) + (-0.11437029 * vgo0_z_z[3]));
        out2 = out2 + ((-0.5164534 * vgo0_z_z[0]) + (-0.60465246 * vgo0_z_z[1]) + (0.38859612 * vgo0_z_z[2]) + (-0.32846406 * vgo0_z_z[3]));
        out3 = out3 + ((0.050266482 * vgo0_z_z[0]) + (-0.20334712 * vgo0_z_z[1]) + (0.18316261 * vgo0_z_z[2]) + (-0.19327633 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((-0.09377763 * vgo0_z_p1[0]) + (-0.0012762006 * vgo0_z_p1[1]) + (-0.028991895 * vgo0_z_p1[2]) + (-0.26523829 * vgo0_z_p1[3]));
        out1 = out1 + ((0.20173682 * vgo0_z_p1[0]) + (0.037923716 * vgo0_z_p1[1]) + (-0.03174243 * vgo0_z_p1[2]) + (0.07103378 * vgo0_z_p1[3]));
        out2 = out2 + ((-0.10764164 * vgo0_z_p1[0]) + (-0.30752546 * vgo0_z_p1[1]) + (0.20556998 * vgo0_z_p1[2]) + (-0.1892279 * vgo0_z_p1[3]));
        out3 = out3 + ((0.08115748 * vgo0_z_p1[0]) + (-0.023550175 * vgo0_z_p1[1]) + (-0.07627362 * vgo0_z_p1[2]) + (0.11746628 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((-0.06998859 * vgo0_p1_n1[0]) + (-0.017997518 * vgo0_p1_n1[1]) + (0.069938794 * vgo0_p1_n1[2]) + (-0.14943017 * vgo0_p1_n1[3]));
        out1 = out1 + ((-0.14179112 * vgo0_p1_n1[0]) + (0.16643842 * vgo0_p1_n1[1]) + (-0.110231474 * vgo0_p1_n1[2]) + (0.08895815 * vgo0_p1_n1[3]));
        out2 = out2 + ((-0.24074875 * vgo0_p1_n1[0]) + (0.3277253 * vgo0_p1_n1[1]) + (-0.07435203 * vgo0_p1_n1[2]) + (-0.23452802 * vgo0_p1_n1[3]));
        out3 = out3 + ((0.039962552 * vgo0_p1_n1[0]) + (-0.07145652 * vgo0_p1_n1[1]) + (-0.022511544 * vgo0_p1_n1[2]) + (-0.04571222 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((-0.059785757 * vgo0_p1_z[0]) + (-0.23771374 * vgo0_p1_z[1]) + (-0.030571314 * vgo0_p1_z[2]) + (0.25222278 * vgo0_p1_z[3]));
        out1 = out1 + ((0.106601834 * vgo0_p1_z[0]) + (0.34398326 * vgo0_p1_z[1]) + (0.14511436 * vgo0_p1_z[2]) + (-0.03867526 * vgo0_p1_z[3]));
        out2 = out2 + ((-0.38982397 * vgo0_p1_z[0]) + (-0.11944689 * vgo0_p1_z[1]) + (0.12997924 * vgo0_p1_z[2]) + (-0.13079585 * vgo0_p1_z[3]));
        out3 = out3 + ((0.005729482 * vgo0_p1_z[0]) + (0.012653905 * vgo0_p1_z[1]) + (-0.063693404 * vgo0_p1_z[2]) + (0.09632285 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((-0.04933823 * vgo0_p1_p1[0]) + (0.0547175 * vgo0_p1_p1[1]) + (0.050636575 * vgo0_p1_p1[2]) + (-0.10060694 * vgo0_p1_p1[3]));
        out1 = out1 + ((0.1344485 * vgo0_p1_p1[0]) + (0.19752938 * vgo0_p1_p1[1]) + (-0.100068115 * vgo0_p1_p1[2]) + (-0.028829506 * vgo0_p1_p1[3]));
        out2 = out2 + ((-0.14096203 * vgo0_p1_p1[0]) + (-0.079092234 * vgo0_p1_p1[1]) + (0.092109434 * vgo0_p1_p1[2]) + (0.011606209 * vgo0_p1_p1[3]));
        out3 = out3 + ((-0.04052607 * vgo0_p1_p1[0]) + (-0.008347507 * vgo0_p1_p1[1]) + (0.06956573 * vgo0_p1_p1[2]) + (-0.028109524 * vgo0_p1_p1[3]));
    }
    {
        let sgo1_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo1_n1_n1 = relu_neg(sgo1_n1_n1);
        out0 = out0 + ((0.21918017 * vgo1_n1_n1[0]) + (-0.11115073 * vgo1_n1_n1[1]) + (0.2262453 * vgo1_n1_n1[2]) + (-0.06889667 * vgo1_n1_n1[3]));
        out1 = out1 + ((-0.11256312 * vgo1_n1_n1[0]) + (-0.07438075 * vgo1_n1_n1[1]) + (-0.088454485 * vgo1_n1_n1[2]) + (0.13672407 * vgo1_n1_n1[3]));
        out2 = out2 + ((-0.06905764 * vgo1_n1_n1[0]) + (0.08128395 * vgo1_n1_n1[1]) + (0.016103368 * vgo1_n1_n1[2]) + (0.050190717 * vgo1_n1_n1[3]));
        out3 = out3 + ((0.09691516 * vgo1_n1_n1[0]) + (0.05845721 * vgo1_n1_n1[1]) + (0.4886816 * vgo1_n1_n1[2]) + (0.041121427 * vgo1_n1_n1[3]));
    }
    {
        let sgo1_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo1_n1_z = relu_neg(sgo1_n1_z);
        out0 = out0 + ((-0.3449472 * vgo1_n1_z[0]) + (0.09711974 * vgo1_n1_z[1]) + (-0.13881907 * vgo1_n1_z[2]) + (-0.018265123 * vgo1_n1_z[3]));
        out1 = out1 + ((0.27855873 * vgo1_n1_z[0]) + (-0.07030004 * vgo1_n1_z[1]) + (0.29545054 * vgo1_n1_z[2]) + (0.37216932 * vgo1_n1_z[3]));
        out2 = out2 + ((0.08657718 * vgo1_n1_z[0]) + (0.099066615 * vgo1_n1_z[1]) + (-0.10574013 * vgo1_n1_z[2]) + (-0.17667885 * vgo1_n1_z[3]));
        out3 = out3 + ((-0.14855732 * vgo1_n1_z[0]) + (-0.11351448 * vgo1_n1_z[1]) + (0.66945946 * vgo1_n1_z[2]) + (0.11312157 * vgo1_n1_z[3]));
    }
    {
        let sgo1_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo1_n1_p1 = relu_neg(sgo1_n1_p1);
        out0 = out0 + ((0.2526151 * vgo1_n1_p1[0]) + (-0.04594331 * vgo1_n1_p1[1]) + (-0.06606611 * vgo1_n1_p1[2]) + (0.09104881 * vgo1_n1_p1[3]));
        out1 = out1 + ((0.06857995 * vgo1_n1_p1[0]) + (-0.075284235 * vgo1_n1_p1[1]) + (-0.17664689 * vgo1_n1_p1[2]) + (0.21578754 * vgo1_n1_p1[3]));
        out2 = out2 + ((0.0696524 * vgo1_n1_p1[0]) + (0.09142951 * vgo1_n1_p1[1]) + (0.080997564 * vgo1_n1_p1[2]) + (-0.0682772 * vgo1_n1_p1[3]));
        out3 = out3 + ((-0.0011445724 * vgo1_n1_p1[0]) + (-0.11736295 * vgo1_n1_p1[1]) + (0.2519232 * vgo1_n1_p1[2]) + (-0.101926275 * vgo1_n1_p1[3]));
    }
    {
        let sgo1_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo1_z_n1 = relu_neg(sgo1_z_n1);
        out0 = out0 + ((-0.12913518 * vgo1_z_n1[0]) + (0.058357026 * vgo1_z_n1[1]) + (0.195421 * vgo1_z_n1[2]) + (-0.15651494 * vgo1_z_n1[3]));
        out1 = out1 + ((0.2877076 * vgo1_z_n1[0]) + (0.0033844314 * vgo1_z_n1[1]) + (-0.07831594 * vgo1_z_n1[2]) + (0.052855384 * vgo1_z_n1[3]));
        out2 = out2 + ((-0.031295884 * vgo1_z_n1[0]) + (0.03301088 * vgo1_z_n1[1]) + (-0.18408822 * vgo1_z_n1[2]) + (0.06732994 * vgo1_z_n1[3]));
        out3 = out3 + ((0.23742151 * vgo1_z_n1[0]) + (-0.12568143 * vgo1_z_n1[1]) + (0.22810535 * vgo1_z_n1[2]) + (-0.11545694 * vgo1_z_n1[3]));
    }
    {
        let sgo1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo1_z_z = relu_neg(sgo1_z_z);
        out0 = out0 + ((-0.49203303 * vgo1_z_z[0]) + (-0.22656603 * vgo1_z_z[1]) + (0.1723193 * vgo1_z_z[2]) + (-0.51250046 * vgo1_z_z[3]));
        out1 = out1 + ((-0.09742038 * vgo1_z_z[0]) + (0.758559 * vgo1_z_z[1]) + (-0.3387505 * vgo1_z_z[2]) + (-0.6193586 * vgo1_z_z[3]));
        out2 = out2 + ((0.14136684 * vgo1_z_z[0]) + (0.27679884 * vgo1_z_z[1]) + (-0.050113205 * vgo1_z_z[2]) + (0.31041816 * vgo1_z_z[3]));
        out3 = out3 + ((-0.36475047 * vgo1_z_z[0]) + (-0.48746544 * vgo1_z_z[1]) + (0.3233227 * vgo1_z_z[2]) + (0.4579754 * vgo1_z_z[3]));
    }
    {
        let sgo1_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo1_z_p1 = relu_neg(sgo1_z_p1);
        out0 = out0 + ((0.46636763 * vgo1_z_p1[0]) + (0.1507748 * vgo1_z_p1[1]) + (-0.2581362 * vgo1_z_p1[2]) + (0.15413165 * vgo1_z_p1[3]));
        out1 = out1 + ((-0.17160143 * vgo1_z_p1[0]) + (0.14256273 * vgo1_z_p1[1]) + (-0.074575804 * vgo1_z_p1[2]) + (-0.099299066 * vgo1_z_p1[3]));
        out2 = out2 + ((-0.0017214464 * vgo1_z_p1[0]) + (0.13778336 * vgo1_z_p1[1]) + (-0.07378213 * vgo1_z_p1[2]) + (-0.15489665 * vgo1_z_p1[3]));
        out3 = out3 + ((-0.10533715 * vgo1_z_p1[0]) + (-0.0011083825 * vgo1_z_p1[1]) + (0.39584312 * vgo1_z_p1[2]) + (0.0023906573 * vgo1_z_p1[3]));
    }
    {
        let sgo1_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo1_p1_n1 = relu_neg(sgo1_p1_n1);
        out0 = out0 + ((0.026959421 * vgo1_p1_n1[0]) + (-0.06391859 * vgo1_p1_n1[1]) + (0.0034752619 * vgo1_p1_n1[2]) + (0.14521928 * vgo1_p1_n1[3]));
        out1 = out1 + ((-0.0010877338 * vgo1_p1_n1[0]) + (-0.032619733 * vgo1_p1_n1[1]) + (0.005375293 * vgo1_p1_n1[2]) + (-0.018952755 * vgo1_p1_n1[3]));
        out2 = out2 + ((0.03381545 * vgo1_p1_n1[0]) + (-0.007652831 * vgo1_p1_n1[1]) + (0.034141563 * vgo1_p1_n1[2]) + (0.046016496 * vgo1_p1_n1[3]));
        out3 = out3 + ((0.11219674 * vgo1_p1_n1[0]) + (0.030913852 * vgo1_p1_n1[1]) + (0.077403754 * vgo1_p1_n1[2]) + (0.17192438 * vgo1_p1_n1[3]));
    }
    {
        let sgo1_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo1_p1_z = relu_neg(sgo1_p1_z);
        out0 = out0 + ((0.040326044 * vgo1_p1_z[0]) + (0.17290725 * vgo1_p1_z[1]) + (-0.1220239 * vgo1_p1_z[2]) + (-0.09594783 * vgo1_p1_z[3]));
        out1 = out1 + ((-0.025229257 * vgo1_p1_z[0]) + (0.17913155 * vgo1_p1_z[1]) + (-0.26623353 * vgo1_p1_z[2]) + (-0.033396784 * vgo1_p1_z[3]));
        out2 = out2 + ((-0.03075146 * vgo1_p1_z[0]) + (0.009143897 * vgo1_p1_z[1]) + (-0.0136083895 * vgo1_p1_z[2]) + (-0.13886899 * vgo1_p1_z[3]));
        out3 = out3 + ((0.075683735 * vgo1_p1_z[0]) + (-0.11584183 * vgo1_p1_z[1]) + (0.22182357 * vgo1_p1_z[2]) + (0.19350322 * vgo1_p1_z[3]));
    }
    {
        let sgo1_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo1_p1_p1 = relu_neg(sgo1_p1_p1);
        out0 = out0 + ((0.15726025 * vgo1_p1_p1[0]) + (-0.10215694 * vgo1_p1_p1[1]) + (-0.060057458 * vgo1_p1_p1[2]) + (0.26487043 * vgo1_p1_p1[3]));
        out1 = out1 + ((-0.04075552 * vgo1_p1_p1[0]) + (-0.016496127 * vgo1_p1_p1[1]) + (0.0015382086 * vgo1_p1_p1[2]) + (0.108562306 * vgo1_p1_p1[3]));
        out2 = out2 + ((0.026795091 * vgo1_p1_p1[0]) + (0.0441233 * vgo1_p1_p1[1]) + (-0.08754318 * vgo1_p1_p1[2]) + (-0.0460157 * vgo1_p1_p1[3]));
        out3 = out3 + ((0.048422016 * vgo1_p1_p1[0]) + (0.14107347 * vgo1_p1_p1[1]) + (0.07986661 * vgo1_p1_p1[2]) + (0.1047697 * vgo1_p1_p1[3]));
    }
    out0 = out0 + (0.0766796);
    out1 = out1 + (0.08115133);
    out2 = out2 + (-0.05703058);
    out3 = out3 + (0.14025708);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
