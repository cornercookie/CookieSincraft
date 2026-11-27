// Anime4K CNN x2 (M) — pass 6/9: conv2d_5_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x3x3x8
// GLSL BIND: conv2d_4_tf
// GLSL SAVE: conv2d_5_tf

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
        out0 = out0 + ((0.06051604 * vgo0_n1_n1[0]) + (-0.028152643 * vgo0_n1_n1[1]) + (-0.21418124 * vgo0_n1_n1[2]) + (0.13032125 * vgo0_n1_n1[3]));
        out1 = out1 + ((0.42565975 * vgo0_n1_n1[0]) + (-0.09571944 * vgo0_n1_n1[1]) + (-0.34494513 * vgo0_n1_n1[2]) + (0.30004 * vgo0_n1_n1[3]));
        out2 = out2 + ((-0.073245734 * vgo0_n1_n1[0]) + (-0.028659137 * vgo0_n1_n1[1]) + (0.0032105136 * vgo0_n1_n1[2]) + (-0.05009555 * vgo0_n1_n1[3]));
        out3 = out3 + ((-0.048971225 * vgo0_n1_n1[0]) + (0.04814533 * vgo0_n1_n1[1]) + (0.002843805 * vgo0_n1_n1[2]) + (-0.046224426 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((-0.07495975 * vgo0_n1_z[0]) + (0.018714864 * vgo0_n1_z[1]) + (0.21229684 * vgo0_n1_z[2]) + (-0.13614887 * vgo0_n1_z[3]));
        out1 = out1 + ((0.79988647 * vgo0_n1_z[0]) + (-0.0697328 * vgo0_n1_z[1]) + (0.38232988 * vgo0_n1_z[2]) + (0.24165109 * vgo0_n1_z[3]));
        out2 = out2 + ((0.25947478 * vgo0_n1_z[0]) + (-0.0009418982 * vgo0_n1_z[1]) + (-0.17369923 * vgo0_n1_z[2]) + (0.10007766 * vgo0_n1_z[3]));
        out3 = out3 + ((0.024117598 * vgo0_n1_z[0]) + (0.028611807 * vgo0_n1_z[1]) + (0.15090801 * vgo0_n1_z[2]) + (-0.06344829 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((-0.07982219 * vgo0_n1_p1[0]) + (0.0900347 * vgo0_n1_p1[1]) + (0.007609254 * vgo0_n1_p1[2]) + (-0.0034791247 * vgo0_n1_p1[3]));
        out1 = out1 + ((0.013611781 * vgo0_n1_p1[0]) + (-0.13560618 * vgo0_n1_p1[1]) + (0.09685799 * vgo0_n1_p1[2]) + (0.06276075 * vgo0_n1_p1[3]));
        out2 = out2 + ((0.134693 * vgo0_n1_p1[0]) + (-0.14370437 * vgo0_n1_p1[1]) + (-0.25175703 * vgo0_n1_p1[2]) + (-0.0016138123 * vgo0_n1_p1[3]));
        out3 = out3 + ((-0.0075672898 * vgo0_n1_p1[0]) + (-0.13325731 * vgo0_n1_p1[1]) + (-0.061100446 * vgo0_n1_p1[2]) + (0.0059743375 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((-0.039018434 * vgo0_z_n1[0]) + (-0.19668463 * vgo0_z_n1[1]) + (-0.43018532 * vgo0_z_n1[2]) + (0.31886247 * vgo0_z_n1[3]));
        out1 = out1 + ((0.4965479 * vgo0_z_n1[0]) + (0.114569925 * vgo0_z_n1[1]) + (0.19110382 * vgo0_z_n1[2]) + (0.27343535 * vgo0_z_n1[3]));
        out2 = out2 + ((0.0707728 * vgo0_z_n1[0]) + (-0.11877004 * vgo0_z_n1[1]) + (-0.25827697 * vgo0_z_n1[2]) + (0.37012872 * vgo0_z_n1[3]));
        out3 = out3 + ((0.1474777 * vgo0_z_n1[0]) + (0.07056952 * vgo0_z_n1[1]) + (-0.14965728 * vgo0_z_n1[2]) + (0.061595406 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((0.506543 * vgo0_z_z[0]) + (-0.16268773 * vgo0_z_z[1]) + (0.455319 * vgo0_z_z[2]) + (-0.0702646 * vgo0_z_z[3]));
        out1 = out1 + ((0.70102173 * vgo0_z_z[0]) + (-0.14041683 * vgo0_z_z[1]) + (0.70184857 * vgo0_z_z[2]) + (0.4817842 * vgo0_z_z[3]));
        out2 = out2 + ((-0.3389246 * vgo0_z_z[0]) + (-0.14463086 * vgo0_z_z[1]) + (0.13763213 * vgo0_z_z[2]) + (-1.1259074 * vgo0_z_z[3]));
        out3 = out3 + ((0.47722015 * vgo0_z_z[0]) + (0.38352612 * vgo0_z_z[1]) + (-0.04293366 * vgo0_z_z[2]) + (-0.5604627 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((0.17606944 * vgo0_z_p1[0]) + (0.15897374 * vgo0_z_p1[1]) + (0.13499324 * vgo0_z_p1[2]) + (0.29241478 * vgo0_z_p1[3]));
        out1 = out1 + ((-0.032824475 * vgo0_z_p1[0]) + (0.11128662 * vgo0_z_p1[1]) + (-0.22204424 * vgo0_z_p1[2]) + (-0.051803727 * vgo0_z_p1[3]));
        out2 = out2 + ((0.013195331 * vgo0_z_p1[0]) + (-0.42040786 * vgo0_z_p1[1]) + (-0.3950585 * vgo0_z_p1[2]) + (0.70745844 * vgo0_z_p1[3]));
        out3 = out3 + ((0.38646924 * vgo0_z_p1[0]) + (-0.19080774 * vgo0_z_p1[1]) + (-0.15171832 * vgo0_z_p1[2]) + (-0.10742828 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((-0.039278325 * vgo0_p1_n1[0]) + (0.18421806 * vgo0_p1_n1[1]) + (-0.044948544 * vgo0_p1_n1[2]) + (0.07902063 * vgo0_p1_n1[3]));
        out1 = out1 + ((-0.2149251 * vgo0_p1_n1[0]) + (0.09913459 * vgo0_p1_n1[1]) + (-0.09743655 * vgo0_p1_n1[2]) + (-0.26899317 * vgo0_p1_n1[3]));
        out2 = out2 + ((-0.002695496 * vgo0_p1_n1[0]) + (-0.07554527 * vgo0_p1_n1[1]) + (-0.22373366 * vgo0_p1_n1[2]) + (0.17830558 * vgo0_p1_n1[3]));
        out3 = out3 + ((-0.047994815 * vgo0_p1_n1[0]) + (-0.06789183 * vgo0_p1_n1[1]) + (-0.06755918 * vgo0_p1_n1[2]) + (-0.104452066 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((-0.0493473 * vgo0_p1_z[0]) + (-0.30411786 * vgo0_p1_z[1]) + (-0.056439694 * vgo0_p1_z[2]) + (-0.06582185 * vgo0_p1_z[3]));
        out1 = out1 + ((-0.21309847 * vgo0_p1_z[0]) + (0.100670904 * vgo0_p1_z[1]) + (-0.22966193 * vgo0_p1_z[2]) + (-0.045954112 * vgo0_p1_z[3]));
        out2 = out2 + ((0.12728062 * vgo0_p1_z[0]) + (-0.25081897 * vgo0_p1_z[1]) + (-0.094699375 * vgo0_p1_z[2]) + (-0.4036555 * vgo0_p1_z[3]));
        out3 = out3 + ((0.060854495 * vgo0_p1_z[0]) + (-0.64373237 * vgo0_p1_z[1]) + (-0.21522263 * vgo0_p1_z[2]) + (-0.6683476 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((0.063481025 * vgo0_p1_p1[0]) + (0.11744312 * vgo0_p1_p1[1]) + (-0.043330096 * vgo0_p1_p1[2]) + (0.33817932 * vgo0_p1_p1[3]));
        out1 = out1 + ((-0.06679828 * vgo0_p1_p1[0]) + (-0.23207302 * vgo0_p1_p1[1]) + (-0.10188898 * vgo0_p1_p1[2]) + (-0.10590511 * vgo0_p1_p1[3]));
        out2 = out2 + ((0.058780864 * vgo0_p1_p1[0]) + (0.047292337 * vgo0_p1_p1[1]) + (-0.11834696 * vgo0_p1_p1[2]) + (0.10076128 * vgo0_p1_p1[3]));
        out3 = out3 + ((-0.036641665 * vgo0_p1_p1[0]) + (0.30200714 * vgo0_p1_p1[1]) + (-0.0002892557 * vgo0_p1_p1[2]) + (-0.10303763 * vgo0_p1_p1[3]));
    }
    {
        let sgo1_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo1_n1_n1 = relu_neg(sgo1_n1_n1);
        out0 = out0 + ((-0.10842604 * vgo1_n1_n1[0]) + (0.042055763 * vgo1_n1_n1[1]) + (0.29702973 * vgo1_n1_n1[2]) + (-0.07409644 * vgo1_n1_n1[3]));
        out1 = out1 + ((-0.030164458 * vgo1_n1_n1[0]) + (-0.012098744 * vgo1_n1_n1[1]) + (-0.06396587 * vgo1_n1_n1[2]) + (-0.08787527 * vgo1_n1_n1[3]));
        out2 = out2 + ((0.051854923 * vgo1_n1_n1[0]) + (0.12997511 * vgo1_n1_n1[1]) + (0.11468497 * vgo1_n1_n1[2]) + (0.15022379 * vgo1_n1_n1[3]));
        out3 = out3 + ((0.007814715 * vgo1_n1_n1[0]) + (0.014517445 * vgo1_n1_n1[1]) + (0.025484756 * vgo1_n1_n1[2]) + (0.01078619 * vgo1_n1_n1[3]));
    }
    {
        let sgo1_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo1_n1_z = relu_neg(sgo1_n1_z);
        out0 = out0 + ((-0.29229385 * vgo1_n1_z[0]) + (0.040265664 * vgo1_n1_z[1]) + (-0.15376821 * vgo1_n1_z[2]) + (0.075579196 * vgo1_n1_z[3]));
        out1 = out1 + ((-0.05593569 * vgo1_n1_z[0]) + (-0.045405343 * vgo1_n1_z[1]) + (0.12099204 * vgo1_n1_z[2]) + (0.1571252 * vgo1_n1_z[3]));
        out2 = out2 + ((0.17841713 * vgo1_n1_z[0]) + (0.04673325 * vgo1_n1_z[1]) + (0.14550509 * vgo1_n1_z[2]) + (0.08603346 * vgo1_n1_z[3]));
        out3 = out3 + ((-0.049786013 * vgo1_n1_z[0]) + (0.06121843 * vgo1_n1_z[1]) + (-0.16273825 * vgo1_n1_z[2]) + (-0.13857752 * vgo1_n1_z[3]));
    }
    {
        let sgo1_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo1_n1_p1 = relu_neg(sgo1_n1_p1);
        out0 = out0 + ((0.06903744 * vgo1_n1_p1[0]) + (0.2628764 * vgo1_n1_p1[1]) + (-0.13582836 * vgo1_n1_p1[2]) + (-0.35678583 * vgo1_n1_p1[3]));
        out1 = out1 + ((-0.13821034 * vgo1_n1_p1[0]) + (-0.019381443 * vgo1_n1_p1[1]) + (-0.19570538 * vgo1_n1_p1[2]) + (-0.09298511 * vgo1_n1_p1[3]));
        out2 = out2 + ((0.08965436 * vgo1_n1_p1[0]) + (0.09745909 * vgo1_n1_p1[1]) + (0.20055099 * vgo1_n1_p1[2]) + (0.024967568 * vgo1_n1_p1[3]));
        out3 = out3 + ((0.08144204 * vgo1_n1_p1[0]) + (0.004633625 * vgo1_n1_p1[1]) + (0.12809834 * vgo1_n1_p1[2]) + (-0.009431525 * vgo1_n1_p1[3]));
    }
    {
        let sgo1_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo1_z_n1 = relu_neg(sgo1_z_n1);
        out0 = out0 + ((0.09784006 * vgo1_z_n1[0]) + (0.010729353 * vgo1_z_n1[1]) + (0.046643205 * vgo1_z_n1[2]) + (-0.110926524 * vgo1_z_n1[3]));
        out1 = out1 + ((-0.21556224 * vgo1_z_n1[0]) + (0.00016300633 * vgo1_z_n1[1]) + (0.122175336 * vgo1_z_n1[2]) + (0.15004392 * vgo1_z_n1[3]));
        out2 = out2 + ((0.013864355 * vgo1_z_n1[0]) + (0.24767809 * vgo1_z_n1[1]) + (0.13865592 * vgo1_z_n1[2]) + (0.0155424485 * vgo1_z_n1[3]));
        out3 = out3 + ((-0.1450483 * vgo1_z_n1[0]) + (-0.15688781 * vgo1_z_n1[1]) + (-0.06195043 * vgo1_z_n1[2]) + (-0.13745981 * vgo1_z_n1[3]));
    }
    {
        let sgo1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo1_z_z = relu_neg(sgo1_z_z);
        out0 = out0 + ((0.018991318 * vgo1_z_z[0]) + (0.55401963 * vgo1_z_z[1]) + (0.11709872 * vgo1_z_z[2]) + (-0.028442185 * vgo1_z_z[3]));
        out1 = out1 + ((-0.46035343 * vgo1_z_z[0]) + (-0.10215539 * vgo1_z_z[1]) + (-0.60193926 * vgo1_z_z[2]) + (0.47882316 * vgo1_z_z[3]));
        out2 = out2 + ((-0.23346989 * vgo1_z_z[0]) + (0.037200127 * vgo1_z_z[1]) + (0.22814943 * vgo1_z_z[2]) + (-0.08231696 * vgo1_z_z[3]));
        out3 = out3 + ((-0.36430013 * vgo1_z_z[0]) + (-0.011152757 * vgo1_z_z[1]) + (0.48752213 * vgo1_z_z[2]) + (0.29796222 * vgo1_z_z[3]));
    }
    {
        let sgo1_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo1_z_p1 = relu_neg(sgo1_z_p1);
        out0 = out0 + ((-0.07258066 * vgo1_z_p1[0]) + (-0.023222538 * vgo1_z_p1[1]) + (0.23230423 * vgo1_z_p1[2]) + (-0.30317304 * vgo1_z_p1[3]));
        out1 = out1 + ((0.03942911 * vgo1_z_p1[0]) + (-0.06899803 * vgo1_z_p1[1]) + (0.23778579 * vgo1_z_p1[2]) + (0.07418621 * vgo1_z_p1[3]));
        out2 = out2 + ((-0.17443737 * vgo1_z_p1[0]) + (0.33387753 * vgo1_z_p1[1]) + (0.007354842 * vgo1_z_p1[2]) + (-0.123447575 * vgo1_z_p1[3]));
        out3 = out3 + ((-0.1745315 * vgo1_z_p1[0]) + (0.11071779 * vgo1_z_p1[1]) + (-0.11949625 * vgo1_z_p1[2]) + (-0.22832453 * vgo1_z_p1[3]));
    }
    {
        let sgo1_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo1_p1_n1 = relu_neg(sgo1_p1_n1);
        out0 = out0 + ((-0.024909232 * vgo1_p1_n1[0]) + (-0.0308135 * vgo1_p1_n1[1]) + (0.12170621 * vgo1_p1_n1[2]) + (-0.13298757 * vgo1_p1_n1[3]));
        out1 = out1 + ((0.045828197 * vgo1_p1_n1[0]) + (-0.1532345 * vgo1_p1_n1[1]) + (-0.06633672 * vgo1_p1_n1[2]) + (0.23591088 * vgo1_p1_n1[3]));
        out2 = out2 + ((0.04964077 * vgo1_p1_n1[0]) + (0.14091493 * vgo1_p1_n1[1]) + (0.038343724 * vgo1_p1_n1[2]) + (-0.029780807 * vgo1_p1_n1[3]));
        out3 = out3 + ((0.05762822 * vgo1_p1_n1[0]) + (-0.048930667 * vgo1_p1_n1[1]) + (-0.02434709 * vgo1_p1_n1[2]) + (0.07109019 * vgo1_p1_n1[3]));
    }
    {
        let sgo1_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo1_p1_z = relu_neg(sgo1_p1_z);
        out0 = out0 + ((-0.16039175 * vgo1_p1_z[0]) + (0.3004474 * vgo1_p1_z[1]) + (-0.17278233 * vgo1_p1_z[2]) + (0.13677922 * vgo1_p1_z[3]));
        out1 = out1 + ((0.18838613 * vgo1_p1_z[0]) + (0.15054552 * vgo1_p1_z[1]) + (0.32901475 * vgo1_p1_z[2]) + (-0.1288333 * vgo1_p1_z[3]));
        out2 = out2 + ((0.26378244 * vgo1_p1_z[0]) + (-0.05119892 * vgo1_p1_z[1]) + (0.34533516 * vgo1_p1_z[2]) + (0.25180495 * vgo1_p1_z[3]));
        out3 = out3 + ((0.19452183 * vgo1_p1_z[0]) + (0.0843233 * vgo1_p1_z[1]) + (-0.08029368 * vgo1_p1_z[2]) + (0.39877903 * vgo1_p1_z[3]));
    }
    {
        let sgo1_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo1_p1_p1 = relu_neg(sgo1_p1_p1);
        out0 = out0 + ((-0.07097129 * vgo1_p1_p1[0]) + (-0.26492423 * vgo1_p1_p1[1]) + (-0.055032317 * vgo1_p1_p1[2]) + (-0.093516104 * vgo1_p1_p1[3]));
        out1 = out1 + ((-0.11795062 * vgo1_p1_p1[0]) + (0.04086253 * vgo1_p1_p1[1]) + (-0.07989471 * vgo1_p1_p1[2]) + (0.059686553 * vgo1_p1_p1[3]));
        out2 = out2 + ((0.09378249 * vgo1_p1_p1[0]) + (0.45851848 * vgo1_p1_p1[1]) + (0.2510942 * vgo1_p1_p1[2]) + (0.19599153 * vgo1_p1_p1[3]));
        out3 = out3 + ((0.019765077 * vgo1_p1_p1[0]) + (-0.02920918 * vgo1_p1_p1[1]) + (-0.04125142 * vgo1_p1_p1[2]) + (-0.13859107 * vgo1_p1_p1[3]));
    }
    out0 = out0 + (0.04400571);
    out1 = out1 + (-0.04015565);
    out2 = out2 + (0.0140529545);
    out3 = out3 + (0.05474095);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
