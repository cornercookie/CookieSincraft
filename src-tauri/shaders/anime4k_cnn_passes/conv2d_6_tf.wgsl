// Anime4K CNN x2 (M) — pass 7/9: conv2d_6_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x1x1x56
// GLSL BIND: conv2d_5_tf
// GLSL SAVE: conv2d_6_tf

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
        out0 = out0 + ((-0.014236042 * vgo0_n1_n1[0]) + (-0.0031431736 * vgo0_n1_n1[1]) + (-0.1551387 * vgo0_n1_n1[2]) + (0.12515116 * vgo0_n1_n1[3]));
        out1 = out1 + ((-0.28528872 * vgo0_n1_n1[0]) + (0.36161992 * vgo0_n1_n1[1]) + (0.15750743 * vgo0_n1_n1[2]) + (-0.17111474 * vgo0_n1_n1[3]));
        out2 = out2 + ((0.13792591 * vgo0_n1_n1[0]) + (-0.0657419 * vgo0_n1_n1[1]) + (-0.17471549 * vgo0_n1_n1[2]) + (0.14650472 * vgo0_n1_n1[3]));
        out3 = out3 + ((0.034169197 * vgo0_n1_n1[0]) + (-0.019157575 * vgo0_n1_n1[1]) + (0.23520657 * vgo0_n1_n1[2]) + (-0.20358163 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((0.02015035 * vgo0_n1_z[0]) + (0.12993371 * vgo0_n1_z[1]) + (0.11199667 * vgo0_n1_z[2]) + (-0.09854378 * vgo0_n1_z[3]));
        out1 = out1 + ((0.5001741 * vgo0_n1_z[0]) + (0.03462961 * vgo0_n1_z[1]) + (0.24919736 * vgo0_n1_z[2]) + (0.08505297 * vgo0_n1_z[3]));
        out2 = out2 + ((-0.20902094 * vgo0_n1_z[0]) + (-0.24141377 * vgo0_n1_z[1]) + (-0.15360375 * vgo0_n1_z[2]) + (0.049974803 * vgo0_n1_z[3]));
        out3 = out3 + ((-0.037157424 * vgo0_n1_z[0]) + (-0.048510186 * vgo0_n1_z[1]) + (0.20106035 * vgo0_n1_z[2]) + (-0.118480384 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((0.086798504 * vgo0_n1_p1[0]) + (-0.009607818 * vgo0_n1_p1[1]) + (0.034812123 * vgo0_n1_p1[2]) + (-0.005187592 * vgo0_n1_p1[3]));
        out1 = out1 + ((0.0351509 * vgo0_n1_p1[0]) + (0.021755 * vgo0_n1_p1[1]) + (-0.04996161 * vgo0_n1_p1[2]) + (-0.041231696 * vgo0_n1_p1[3]));
        out2 = out2 + ((0.0020545553 * vgo0_n1_p1[0]) + (0.015730752 * vgo0_n1_p1[1]) + (-0.07507172 * vgo0_n1_p1[2]) + (0.018597523 * vgo0_n1_p1[3]));
        out3 = out3 + ((-0.02393343 * vgo0_n1_p1[0]) + (0.07624775 * vgo0_n1_p1[1]) + (0.03892451 * vgo0_n1_p1[2]) + (-0.0025574185 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((0.035725456 * vgo0_z_n1[0]) + (0.06809103 * vgo0_z_n1[1]) + (0.51926994 * vgo0_z_n1[2]) + (-0.39983147 * vgo0_z_n1[3]));
        out1 = out1 + ((-0.16402833 * vgo0_z_n1[0]) + (-0.1243394 * vgo0_z_n1[1]) + (-0.25922915 * vgo0_z_n1[2]) + (0.28285915 * vgo0_z_n1[3]));
        out2 = out2 + ((0.15959994 * vgo0_z_n1[0]) + (-0.2351732 * vgo0_z_n1[1]) + (0.2650535 * vgo0_z_n1[2]) + (-0.30193794 * vgo0_z_n1[3]));
        out3 = out3 + ((-0.11468332 * vgo0_z_n1[0]) + (0.050777763 * vgo0_z_n1[1]) + (-0.51894253 * vgo0_z_n1[2]) + (0.4408367 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((-0.27042082 * vgo0_z_z[0]) + (0.22243942 * vgo0_z_z[1]) + (0.14902467 * vgo0_z_z[2]) + (0.38428563 * vgo0_z_z[3]));
        out1 = out1 + ((0.46612173 * vgo0_z_z[0]) + (0.5169912 * vgo0_z_z[1]) + (-0.22330502 * vgo0_z_z[2]) + (-0.11300288 * vgo0_z_z[3]));
        out2 = out2 + ((-0.36141354 * vgo0_z_z[0]) + (0.0668681 * vgo0_z_z[1]) + (0.2984152 * vgo0_z_z[2]) + (0.1275798 * vgo0_z_z[3]));
        out3 = out3 + ((-0.24121419 * vgo0_z_z[0]) + (0.2952039 * vgo0_z_z[1]) + (-0.45109174 * vgo0_z_z[2]) + (-0.3822957 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((0.26543504 * vgo0_z_p1[0]) + (-0.05742226 * vgo0_z_p1[1]) + (-0.052103903 * vgo0_z_p1[2]) + (-0.013124308 * vgo0_z_p1[3]));
        out1 = out1 + ((-0.14358385 * vgo0_z_p1[0]) + (-0.04024543 * vgo0_z_p1[1]) + (0.07665455 * vgo0_z_p1[2]) + (-0.012301872 * vgo0_z_p1[3]));
        out2 = out2 + ((-0.18752757 * vgo0_z_p1[0]) + (-0.03913891 * vgo0_z_p1[1]) + (0.038205814 * vgo0_z_p1[2]) + (-0.006583095 * vgo0_z_p1[3]));
        out3 = out3 + ((-0.25550908 * vgo0_z_p1[0]) + (-0.25725332 * vgo0_z_p1[1]) + (-0.12454206 * vgo0_z_p1[2]) + (-0.0058936924 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((-0.0018946569 * vgo0_p1_n1[0]) + (0.019746022 * vgo0_p1_n1[1]) + (-0.13080788 * vgo0_p1_n1[2]) + (0.11450627 * vgo0_p1_n1[3]));
        out1 = out1 + ((-0.013743845 * vgo0_p1_n1[0]) + (-0.027179785 * vgo0_p1_n1[1]) + (-0.14425103 * vgo0_p1_n1[2]) + (0.07109661 * vgo0_p1_n1[3]));
        out2 = out2 + ((0.023703793 * vgo0_p1_n1[0]) + (0.086905524 * vgo0_p1_n1[1]) + (0.03151253 * vgo0_p1_n1[2]) + (0.0132474145 * vgo0_p1_n1[3]));
        out3 = out3 + ((0.041018624 * vgo0_p1_n1[0]) + (0.04548913 * vgo0_p1_n1[1]) + (0.2718715 * vgo0_p1_n1[2]) + (-0.20008296 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((-0.076830454 * vgo0_p1_z[0]) + (0.11652955 * vgo0_p1_z[1]) + (0.5068201 * vgo0_p1_z[2]) + (-0.3082819 * vgo0_p1_z[3]));
        out1 = out1 + ((0.058615055 * vgo0_p1_z[0]) + (-0.006765798 * vgo0_p1_z[1]) + (-0.057522714 * vgo0_p1_z[2]) + (0.049981344 * vgo0_p1_z[3]));
        out2 = out2 + ((-0.006897243 * vgo0_p1_z[0]) + (-0.21763432 * vgo0_p1_z[1]) + (0.16896053 * vgo0_p1_z[2]) + (-0.21176189 * vgo0_p1_z[3]));
        out3 = out3 + ((-0.061227098 * vgo0_p1_z[0]) + (0.03566485 * vgo0_p1_z[1]) + (0.08901554 * vgo0_p1_z[2]) + (-0.050980624 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((0.02327798 * vgo0_p1_p1[0]) + (0.07662976 * vgo0_p1_p1[1]) + (0.034811985 * vgo0_p1_p1[2]) + (-0.03238033 * vgo0_p1_p1[3]));
        out1 = out1 + ((-0.0021881019 * vgo0_p1_p1[0]) + (-0.030997375 * vgo0_p1_p1[1]) + (-0.069672935 * vgo0_p1_p1[2]) + (0.04040273 * vgo0_p1_p1[3]));
        out2 = out2 + ((-0.1217442 * vgo0_p1_p1[0]) + (0.104173124 * vgo0_p1_p1[1]) + (0.09862539 * vgo0_p1_p1[2]) + (0.020557549 * vgo0_p1_p1[3]));
        out3 = out3 + ((-0.022286594 * vgo0_p1_p1[0]) + (0.10287763 * vgo0_p1_p1[1]) + (-0.021694934 * vgo0_p1_p1[2]) + (0.07542515 * vgo0_p1_p1[3]));
    }
    {
        let sgo1_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo1_n1_n1 = relu_neg(sgo1_n1_n1);
        out0 = out0 + ((0.124069154 * vgo1_n1_n1[0]) + (-0.08579466 * vgo1_n1_n1[1]) + (-0.07816314 * vgo1_n1_n1[2]) + (0.11332851 * vgo1_n1_n1[3]));
        out1 = out1 + ((-0.034682628 * vgo1_n1_n1[0]) + (-0.11038275 * vgo1_n1_n1[1]) + (0.04750615 * vgo1_n1_n1[2]) + (-0.096100725 * vgo1_n1_n1[3]));
        out2 = out2 + ((0.039588403 * vgo1_n1_n1[0]) + (-0.15149672 * vgo1_n1_n1[1]) + (-0.05529172 * vgo1_n1_n1[2]) + (0.034304325 * vgo1_n1_n1[3]));
        out3 = out3 + ((-0.022520235 * vgo1_n1_n1[0]) + (-0.05023852 * vgo1_n1_n1[1]) + (-0.2674731 * vgo1_n1_n1[2]) + (0.21886522 * vgo1_n1_n1[3]));
    }
    {
        let sgo1_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo1_n1_z = relu_neg(sgo1_n1_z);
        out0 = out0 + ((-0.1948599 * vgo1_n1_z[0]) + (-0.14946899 * vgo1_n1_z[1]) + (-0.39548838 * vgo1_n1_z[2]) + (0.18042913 * vgo1_n1_z[3]));
        out1 = out1 + ((-0.007919619 * vgo1_n1_z[0]) + (0.19826505 * vgo1_n1_z[1]) + (0.23789087 * vgo1_n1_z[2]) + (0.009140256 * vgo1_n1_z[3]));
        out2 = out2 + ((0.11857748 * vgo1_n1_z[0]) + (0.18215668 * vgo1_n1_z[1]) + (0.13606293 * vgo1_n1_z[2]) + (-0.09209675 * vgo1_n1_z[3]));
        out3 = out3 + ((-0.080678545 * vgo1_n1_z[0]) + (-0.020431137 * vgo1_n1_z[1]) + (-0.07728839 * vgo1_n1_z[2]) + (-0.051353537 * vgo1_n1_z[3]));
    }
    {
        let sgo1_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo1_n1_p1 = relu_neg(sgo1_n1_p1);
        out0 = out0 + ((-0.07616472 * vgo1_n1_p1[0]) + (-0.0032800382 * vgo1_n1_p1[1]) + (-0.045657665 * vgo1_n1_p1[2]) + (-0.039144326 * vgo1_n1_p1[3]));
        out1 = out1 + ((-0.37786487 * vgo1_n1_p1[0]) + (-0.08877774 * vgo1_n1_p1[1]) + (0.053579114 * vgo1_n1_p1[2]) + (-0.070886396 * vgo1_n1_p1[3]));
        out2 = out2 + ((0.011311804 * vgo1_n1_p1[0]) + (0.107276045 * vgo1_n1_p1[1]) + (0.013236154 * vgo1_n1_p1[2]) + (0.009832061 * vgo1_n1_p1[3]));
        out3 = out3 + ((0.08292063 * vgo1_n1_p1[0]) + (0.12258811 * vgo1_n1_p1[1]) + (0.0005569043 * vgo1_n1_p1[2]) + (-0.009806432 * vgo1_n1_p1[3]));
    }
    {
        let sgo1_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo1_z_n1 = relu_neg(sgo1_z_n1);
        out0 = out0 + ((-0.28062925 * vgo1_z_n1[0]) + (0.15946878 * vgo1_z_n1[1]) + (-0.1021801 * vgo1_z_n1[2]) + (-0.06471589 * vgo1_z_n1[3]));
        out1 = out1 + ((-0.26999477 * vgo1_z_n1[0]) + (0.21230288 * vgo1_z_n1[1]) + (-0.14243907 * vgo1_z_n1[2]) + (0.2555922 * vgo1_z_n1[3]));
        out2 = out2 + ((-0.09608517 * vgo1_z_n1[0]) + (0.26339412 * vgo1_z_n1[1]) + (0.20891234 * vgo1_z_n1[2]) + (-0.23538485 * vgo1_z_n1[3]));
        out3 = out3 + ((0.33958244 * vgo1_z_n1[0]) + (-0.12569186 * vgo1_z_n1[1]) + (0.43289876 * vgo1_z_n1[2]) + (-0.33462036 * vgo1_z_n1[3]));
    }
    {
        let sgo1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo1_z_z = relu_neg(sgo1_z_z);
        out0 = out0 + ((0.16265294 * vgo1_z_z[0]) + (0.2625464 * vgo1_z_z[1]) + (-0.34452894 * vgo1_z_z[2]) + (0.2233622 * vgo1_z_z[3]));
        out1 = out1 + ((0.13850005 * vgo1_z_z[0]) + (-0.42999864 * vgo1_z_z[1]) + (-0.5385177 * vgo1_z_z[2]) + (-0.11035979 * vgo1_z_z[3]));
        out2 = out2 + ((0.51662 * vgo1_z_z[0]) + (-0.78238726 * vgo1_z_z[1]) + (-0.09422375 * vgo1_z_z[2]) + (0.83759475 * vgo1_z_z[3]));
        out3 = out3 + ((0.44468537 * vgo1_z_z[0]) + (0.14301361 * vgo1_z_z[1]) + (0.108906105 * vgo1_z_z[2]) + (1.1596143 * vgo1_z_z[3]));
    }
    {
        let sgo1_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo1_z_p1 = relu_neg(sgo1_z_p1);
        out0 = out0 + ((-0.73757625 * vgo1_z_p1[0]) + (-0.12369605 * vgo1_z_p1[1]) + (0.23523071 * vgo1_z_p1[2]) + (0.006587637 * vgo1_z_p1[3]));
        out1 = out1 + ((-0.15445381 * vgo1_z_p1[0]) + (0.22757277 * vgo1_z_p1[1]) + (0.052819528 * vgo1_z_p1[2]) + (0.10183905 * vgo1_z_p1[3]));
        out2 = out2 + ((-0.07912228 * vgo1_z_p1[0]) + (-0.16998893 * vgo1_z_p1[1]) + (-0.13360223 * vgo1_z_p1[2]) + (0.014348178 * vgo1_z_p1[3]));
        out3 = out3 + ((-0.17778571 * vgo1_z_p1[0]) + (-0.41047302 * vgo1_z_p1[1]) + (0.10241381 * vgo1_z_p1[2]) + (-0.08526306 * vgo1_z_p1[3]));
    }
    {
        let sgo1_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo1_p1_n1 = relu_neg(sgo1_p1_n1);
        out0 = out0 + ((0.14712952 * vgo1_p1_n1[0]) + (0.048995696 * vgo1_p1_n1[1]) + (0.05299946 * vgo1_p1_n1[2]) + (-0.06817572 * vgo1_p1_n1[3]));
        out1 = out1 + ((0.1498064 * vgo1_p1_n1[0]) + (-0.079825334 * vgo1_p1_n1[1]) + (0.40354064 * vgo1_p1_n1[2]) + (-0.31789717 * vgo1_p1_n1[3]));
        out2 = out2 + ((-0.1998377 * vgo1_p1_n1[0]) + (0.00955295 * vgo1_p1_n1[1]) + (-0.32318407 * vgo1_p1_n1[2]) + (0.30898204 * vgo1_p1_n1[3]));
        out3 = out3 + ((-0.039571725 * vgo1_p1_n1[0]) + (-0.026203401 * vgo1_p1_n1[1]) + (-0.16292085 * vgo1_p1_n1[2]) + (0.08574385 * vgo1_p1_n1[3]));
    }
    {
        let sgo1_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo1_p1_z = relu_neg(sgo1_p1_z);
        out0 = out0 + ((-0.6353329 * vgo1_p1_z[0]) + (-0.56000775 * vgo1_p1_z[1]) + (-0.17279743 * vgo1_p1_z[2]) + (0.18198174 * vgo1_p1_z[3]));
        out1 = out1 + ((-0.19555812 * vgo1_p1_z[0]) + (0.056538377 * vgo1_p1_z[1]) + (0.34365895 * vgo1_p1_z[2]) + (-0.07799055 * vgo1_p1_z[3]));
        out2 = out2 + ((0.19011354 * vgo1_p1_z[0]) + (-0.13952748 * vgo1_p1_z[1]) + (0.029196098 * vgo1_p1_z[2]) + (-0.19596763 * vgo1_p1_z[3]));
        out3 = out3 + ((-0.069196045 * vgo1_p1_z[0]) + (-0.17402656 * vgo1_p1_z[1]) + (0.07948411 * vgo1_p1_z[2]) + (-0.016226962 * vgo1_p1_z[3]));
    }
    {
        let sgo1_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo1_p1_p1 = relu_neg(sgo1_p1_p1);
        out0 = out0 + ((0.25592864 * vgo1_p1_p1[0]) + (0.083498634 * vgo1_p1_p1[1]) + (-0.28515807 * vgo1_p1_p1[2]) + (0.10789751 * vgo1_p1_p1[3]));
        out1 = out1 + ((0.0043962947 * vgo1_p1_p1[0]) + (0.07085363 * vgo1_p1_p1[1]) + (0.048724182 * vgo1_p1_p1[2]) + (-0.025131436 * vgo1_p1_p1[3]));
        out2 = out2 + ((-0.0049440865 * vgo1_p1_p1[0]) + (-0.033094388 * vgo1_p1_p1[1]) + (-0.032935806 * vgo1_p1_p1[2]) + (0.04266025 * vgo1_p1_p1[3]));
        out3 = out3 + ((0.20026933 * vgo1_p1_p1[0]) + (0.0927841 * vgo1_p1_p1[1]) + (-0.006839351 * vgo1_p1_p1[2]) + (-0.013012285 * vgo1_p1_p1[3]));
    }
    out0 = out0 + (0.02021373);
    out1 = out1 + (0.0014037411);
    out2 = out2 + (0.0012718709);
    out3 = out3 + (0.017278494);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
