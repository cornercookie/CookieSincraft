// Anime4K CNN x2 (M) — pass 4/9: conv2d_3_tf
// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)
// License: MIT (bloc97). File header retained above; see upstream GLSL.
// GLSL DESC: Anime4K-v3.2-Upscale-CNN-x2-(M)-Conv-4x3x3x8
// GLSL BIND: conv2d_2_tf
// GLSL SAVE: conv2d_3_tf

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
        out0 = out0 + ((-0.18038331 * vgo0_n1_n1[0]) + (0.21830973 * vgo0_n1_n1[1]) + (-0.10019419 * vgo0_n1_n1[2]) + (-0.022745568 * vgo0_n1_n1[3]));
        out1 = out1 + ((-0.14944611 * vgo0_n1_n1[0]) + (-0.15669158 * vgo0_n1_n1[1]) + (0.46361133 * vgo0_n1_n1[2]) + (-0.07289843 * vgo0_n1_n1[3]));
        out2 = out2 + ((0.02976627 * vgo0_n1_n1[0]) + (-0.09000817 * vgo0_n1_n1[1]) + (0.113060996 * vgo0_n1_n1[2]) + (0.05635241 * vgo0_n1_n1[3]));
        out3 = out3 + ((0.012762965 * vgo0_n1_n1[0]) + (-0.022688959 * vgo0_n1_n1[1]) + (0.01629751 * vgo0_n1_n1[2]) + (0.061114635 * vgo0_n1_n1[3]));
    }
    {
        let sgo0_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo0_n1_z = relu_pos(sgo0_n1_z);
        out0 = out0 + ((0.024338024 * vgo0_n1_z[0]) + (-0.10004009 * vgo0_n1_z[1]) + (-0.13709056 * vgo0_n1_z[2]) + (-0.0851965 * vgo0_n1_z[3]));
        out1 = out1 + ((0.23927099 * vgo0_n1_z[0]) + (-0.024349794 * vgo0_n1_z[1]) + (-0.16574804 * vgo0_n1_z[2]) + (0.084686354 * vgo0_n1_z[3]));
        out2 = out2 + ((-0.047885604 * vgo0_n1_z[0]) + (0.09688507 * vgo0_n1_z[1]) + (-0.12733915 * vgo0_n1_z[2]) + (0.06980246 * vgo0_n1_z[3]));
        out3 = out3 + ((0.11480734 * vgo0_n1_z[0]) + (0.014669346 * vgo0_n1_z[1]) + (-0.07505829 * vgo0_n1_z[2]) + (0.04676309 * vgo0_n1_z[3]));
    }
    {
        let sgo0_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo0_n1_p1 = relu_pos(sgo0_n1_p1);
        out0 = out0 + ((0.054203495 * vgo0_n1_p1[0]) + (0.011881634 * vgo0_n1_p1[1]) + (-0.036115017 * vgo0_n1_p1[2]) + (-0.0686298 * vgo0_n1_p1[3]));
        out1 = out1 + ((-0.13682245 * vgo0_n1_p1[0]) + (-0.15678032 * vgo0_n1_p1[1]) + (0.057050128 * vgo0_n1_p1[2]) + (-0.03368558 * vgo0_n1_p1[3]));
        out2 = out2 + ((0.13011025 * vgo0_n1_p1[0]) + (0.033391044 * vgo0_n1_p1[1]) + (-0.09841339 * vgo0_n1_p1[2]) + (-0.027057761 * vgo0_n1_p1[3]));
        out3 = out3 + ((-0.18701133 * vgo0_n1_p1[0]) + (0.20852546 * vgo0_n1_p1[1]) + (-0.13660902 * vgo0_n1_p1[2]) + (0.0005817616 * vgo0_n1_p1[3]));
    }
    {
        let sgo0_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo0_z_n1 = relu_pos(sgo0_z_n1);
        out0 = out0 + ((-0.08077834 * vgo0_z_n1[0]) + (0.35952288 * vgo0_z_n1[1]) + (-0.07647382 * vgo0_z_n1[2]) + (-0.0033230998 * vgo0_z_n1[3]));
        out1 = out1 + ((0.13929126 * vgo0_z_n1[0]) + (-0.09155619 * vgo0_z_n1[1]) + (0.14128102 * vgo0_z_n1[2]) + (0.16005981 * vgo0_z_n1[3]));
        out2 = out2 + ((0.18161216 * vgo0_z_n1[0]) + (-0.09485738 * vgo0_z_n1[1]) + (0.0029118075 * vgo0_z_n1[2]) + (0.052682754 * vgo0_z_n1[3]));
        out3 = out3 + ((0.03242074 * vgo0_z_n1[0]) + (0.08299826 * vgo0_z_n1[1]) + (0.073796146 * vgo0_z_n1[2]) + (-0.06446532 * vgo0_z_n1[3]));
    }
    {
        let sgo0_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo0_z_z = relu_pos(sgo0_z_z);
        out0 = out0 + ((-0.36655015 * vgo0_z_z[0]) + (0.4606936 * vgo0_z_z[1]) + (0.19073649 * vgo0_z_z[2]) + (0.31655258 * vgo0_z_z[3]));
        out1 = out1 + ((-0.006838053 * vgo0_z_z[0]) + (-0.579939 * vgo0_z_z[1]) + (0.089126326 * vgo0_z_z[2]) + (-0.14021218 * vgo0_z_z[3]));
        out2 = out2 + ((-0.3437716 * vgo0_z_z[0]) + (0.16714323 * vgo0_z_z[1]) + (0.17705944 * vgo0_z_z[2]) + (-0.22418492 * vgo0_z_z[3]));
        out3 = out3 + ((-0.3883696 * vgo0_z_z[0]) + (-0.2302651 * vgo0_z_z[1]) + (0.2581861 * vgo0_z_z[2]) + (0.21983066 * vgo0_z_z[3]));
    }
    {
        let sgo0_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo0_z_p1 = relu_pos(sgo0_z_p1);
        out0 = out0 + ((0.0992383 * vgo0_z_p1[0]) + (-0.014257871 * vgo0_z_p1[1]) + (-0.023896435 * vgo0_z_p1[2]) + (0.19868234 * vgo0_z_p1[3]));
        out1 = out1 + ((0.0408007 * vgo0_z_p1[0]) + (0.07995299 * vgo0_z_p1[1]) + (0.16102871 * vgo0_z_p1[2]) + (-0.11668251 * vgo0_z_p1[3]));
        out2 = out2 + ((0.22458278 * vgo0_z_p1[0]) + (-0.05587917 * vgo0_z_p1[1]) + (0.19373615 * vgo0_z_p1[2]) + (-0.016202094 * vgo0_z_p1[3]));
        out3 = out3 + ((-0.25106144 * vgo0_z_p1[0]) + (0.15634494 * vgo0_z_p1[1]) + (0.11624891 * vgo0_z_p1[2]) + (-0.2930768 * vgo0_z_p1[3]));
    }
    {
        let sgo0_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo0_p1_n1 = relu_pos(sgo0_p1_n1);
        out0 = out0 + ((0.024616942 * vgo0_p1_n1[0]) + (0.36248252 * vgo0_p1_n1[1]) + (-0.14779098 * vgo0_p1_n1[2]) + (-0.019894283 * vgo0_p1_n1[3]));
        out1 = out1 + ((-0.007111256 * vgo0_p1_n1[0]) + (0.010641561 * vgo0_p1_n1[1]) + (-0.09541178 * vgo0_p1_n1[2]) + (0.21236233 * vgo0_p1_n1[3]));
        out2 = out2 + ((0.009501827 * vgo0_p1_n1[0]) + (0.08132797 * vgo0_p1_n1[1]) + (-0.13983901 * vgo0_p1_n1[2]) + (0.027207611 * vgo0_p1_n1[3]));
        out3 = out3 + ((0.038444366 * vgo0_p1_n1[0]) + (-0.013995817 * vgo0_p1_n1[1]) + (-0.16242191 * vgo0_p1_n1[2]) + (0.03294123 * vgo0_p1_n1[3]));
    }
    {
        let sgo0_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo0_p1_z = relu_pos(sgo0_p1_z);
        out0 = out0 + ((0.0131698875 * vgo0_p1_z[0]) + (-0.18124102 * vgo0_p1_z[1]) + (-0.13503514 * vgo0_p1_z[2]) + (-0.06099072 * vgo0_p1_z[3]));
        out1 = out1 + ((0.07422735 * vgo0_p1_z[0]) + (-0.20906176 * vgo0_p1_z[1]) + (-0.049005672 * vgo0_p1_z[2]) + (0.08739405 * vgo0_p1_z[3]));
        out2 = out2 + ((-0.031758767 * vgo0_p1_z[0]) + (-0.1978915 * vgo0_p1_z[1]) + (0.23094437 * vgo0_p1_z[2]) + (0.54512614 * vgo0_p1_z[3]));
        out3 = out3 + ((0.21338555 * vgo0_p1_z[0]) + (-0.011205669 * vgo0_p1_z[1]) + (-0.23727885 * vgo0_p1_z[2]) + (-0.29533875 * vgo0_p1_z[3]));
    }
    {
        let sgo0_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo0_p1_p1 = relu_pos(sgo0_p1_p1);
        out0 = out0 + ((-0.0010255767 * vgo0_p1_p1[0]) + (-0.07168225 * vgo0_p1_p1[1]) + (-0.033568826 * vgo0_p1_p1[2]) + (0.22161655 * vgo0_p1_p1[3]));
        out1 = out1 + ((-0.087293416 * vgo0_p1_p1[0]) + (0.11350447 * vgo0_p1_p1[1]) + (0.13653576 * vgo0_p1_p1[2]) + (0.061226424 * vgo0_p1_p1[3]));
        out2 = out2 + ((-0.13074352 * vgo0_p1_p1[0]) + (0.058425818 * vgo0_p1_p1[1]) + (0.038460605 * vgo0_p1_p1[2]) + (0.2749964 * vgo0_p1_p1[3]));
        out3 = out3 + ((-0.012814839 * vgo0_p1_p1[0]) + (0.085885845 * vgo0_p1_p1[1]) + (-0.038151987 * vgo0_p1_p1[2]) + (-0.17960808 * vgo0_p1_p1[3]));
    }
    {
        let sgo1_n1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, -1.0))), 0);
        let vgo1_n1_n1 = relu_neg(sgo1_n1_n1);
        out0 = out0 + ((0.19728905 * vgo1_n1_n1[0]) + (-0.040724937 * vgo1_n1_n1[1]) + (-0.18270236 * vgo1_n1_n1[2]) + (0.046735186 * vgo1_n1_n1[3]));
        out1 = out1 + ((0.03507326 * vgo1_n1_n1[0]) + (0.119867206 * vgo1_n1_n1[1]) + (-0.12691991 * vgo1_n1_n1[2]) + (0.18119748 * vgo1_n1_n1[3]));
        out2 = out2 + ((-0.052895024 * vgo1_n1_n1[0]) + (0.11348764 * vgo1_n1_n1[1]) + (-0.043787055 * vgo1_n1_n1[2]) + (0.004703516 * vgo1_n1_n1[3]));
        out3 = out3 + ((0.006752757 * vgo1_n1_n1[0]) + (-0.06939761 * vgo1_n1_n1[1]) + (-0.009801806 * vgo1_n1_n1[2]) + (-0.075640485 * vgo1_n1_n1[3]));
    }
    {
        let sgo1_n1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 0.0))), 0);
        let vgo1_n1_z = relu_neg(sgo1_n1_z);
        out0 = out0 + ((0.051735226 * vgo1_n1_z[0]) + (0.1732299 * vgo1_n1_z[1]) + (-0.10672899 * vgo1_n1_z[2]) + (0.0320877 * vgo1_n1_z[3]));
        out1 = out1 + ((-0.4913656 * vgo1_n1_z[0]) + (0.2102274 * vgo1_n1_z[1]) + (0.43920282 * vgo1_n1_z[2]) + (0.059108034 * vgo1_n1_z[3]));
        out2 = out2 + ((0.08349019 * vgo1_n1_z[0]) + (-0.16517872 * vgo1_n1_z[1]) + (0.15436842 * vgo1_n1_z[2]) + (-0.1075667 * vgo1_n1_z[3]));
        out3 = out3 + ((0.022741623 * vgo1_n1_z[0]) + (-0.26693836 * vgo1_n1_z[1]) + (0.3645307 * vgo1_n1_z[2]) + (0.017874828 * vgo1_n1_z[3]));
    }
    {
        let sgo1_n1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(-1.0, 1.0))), 0);
        let vgo1_n1_p1 = relu_neg(sgo1_n1_p1);
        out0 = out0 + ((0.034464058 * vgo1_n1_p1[0]) + (0.014929155 * vgo1_n1_p1[1]) + (0.054227423 * vgo1_n1_p1[2]) + (0.14167373 * vgo1_n1_p1[3]));
        out1 = out1 + ((-0.0023630706 * vgo1_n1_p1[0]) + (-0.08904212 * vgo1_n1_p1[1]) + (0.11918041 * vgo1_n1_p1[2]) + (-0.034539603 * vgo1_n1_p1[3]));
        out2 = out2 + ((0.06048089 * vgo1_n1_p1[0]) + (-0.06807333 * vgo1_n1_p1[1]) + (0.14447778 * vgo1_n1_p1[2]) + (0.035260547 * vgo1_n1_p1[3]));
        out3 = out3 + ((0.09979546 * vgo1_n1_p1[0]) + (-0.1924939 * vgo1_n1_p1[1]) + (0.14596114 * vgo1_n1_p1[2]) + (-0.12069667 * vgo1_n1_p1[3]));
    }
    {
        let sgo1_z_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, -1.0))), 0);
        let vgo1_z_n1 = relu_neg(sgo1_z_n1);
        out0 = out0 + ((-0.04427228 * vgo1_z_n1[0]) + (-0.23673469 * vgo1_z_n1[1]) + (0.010357103 * vgo1_z_n1[2]) + (-0.2907043 * vgo1_z_n1[3]));
        out1 = out1 + ((-0.06845721 * vgo1_z_n1[0]) + (-0.078984015 * vgo1_z_n1[1]) + (0.06867713 * vgo1_z_n1[2]) + (-0.058163825 * vgo1_z_n1[3]));
        out2 = out2 + ((-0.12154615 * vgo1_z_n1[0]) + (0.08430951 * vgo1_z_n1[1]) + (0.1922373 * vgo1_z_n1[2]) + (0.030108064 * vgo1_z_n1[3]));
        out3 = out3 + ((-0.43081748 * vgo1_z_n1[0]) + (-0.38715646 * vgo1_z_n1[1]) + (-0.022240646 * vgo1_z_n1[2]) + (-0.15403675 * vgo1_z_n1[3]));
    }
    {
        let sgo1_z_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 0.0))), 0);
        let vgo1_z_z = relu_neg(sgo1_z_z);
        out0 = out0 + ((0.46885306 * vgo1_z_z[0]) + (-0.33421394 * vgo1_z_z[1]) + (-0.6695223 * vgo1_z_z[2]) + (-0.41841158 * vgo1_z_z[3]));
        out1 = out1 + ((0.30317923 * vgo1_z_z[0]) + (0.24244753 * vgo1_z_z[1]) + (-0.1047785 * vgo1_z_z[2]) + (-0.18656285 * vgo1_z_z[3]));
        out2 = out2 + ((0.06261881 * vgo1_z_z[0]) + (-0.4405616 * vgo1_z_z[1]) + (0.24233986 * vgo1_z_z[2]) + (0.40070608 * vgo1_z_z[3]));
        out3 = out3 + ((0.81440526 * vgo1_z_z[0]) + (0.11305212 * vgo1_z_z[1]) + (-0.8826317 * vgo1_z_z[2]) + (-0.023478031 * vgo1_z_z[3]));
    }
    {
        let sgo1_z_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(0.0, 1.0))), 0);
        let vgo1_z_p1 = relu_neg(sgo1_z_p1);
        out0 = out0 + ((-0.07879348 * vgo1_z_p1[0]) + (-0.024378026 * vgo1_z_p1[1]) + (-0.041883785 * vgo1_z_p1[2]) + (-0.17030984 * vgo1_z_p1[3]));
        out1 = out1 + ((0.23229122 * vgo1_z_p1[0]) + (-0.011237109 * vgo1_z_p1[1]) + (0.12058088 * vgo1_z_p1[2]) + (0.20766267 * vgo1_z_p1[3]));
        out2 = out2 + ((-0.36519575 * vgo1_z_p1[0]) + (0.09599417 * vgo1_z_p1[1]) + (-0.1271098 * vgo1_z_p1[2]) + (0.06990154 * vgo1_z_p1[3]));
        out3 = out3 + ((0.21161246 * vgo1_z_p1[0]) + (0.041002538 * vgo1_z_p1[1]) + (-0.36046275 * vgo1_z_p1[2]) + (0.007304667 * vgo1_z_p1[3]));
    }
    {
        let sgo1_p1_n1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, -1.0))), 0);
        let vgo1_p1_n1 = relu_neg(sgo1_p1_n1);
        out0 = out0 + ((0.10873893 * vgo1_p1_n1[0]) + (0.003872542 * vgo1_p1_n1[1]) + (-0.13476561 * vgo1_p1_n1[2]) + (-0.036068805 * vgo1_p1_n1[3]));
        out1 = out1 + ((-0.054637462 * vgo1_p1_n1[0]) + (0.02304618 * vgo1_p1_n1[1]) + (0.04707738 * vgo1_p1_n1[2]) + (-0.2856381 * vgo1_p1_n1[3]));
        out2 = out2 + ((0.07124422 * vgo1_p1_n1[0]) + (0.010866545 * vgo1_p1_n1[1]) + (0.20484549 * vgo1_p1_n1[2]) + (-0.008342406 * vgo1_p1_n1[3]));
        out3 = out3 + ((-0.43660247 * vgo1_p1_n1[0]) + (-0.041055538 * vgo1_p1_n1[1]) + (0.33536008 * vgo1_p1_n1[2]) + (-0.060022205 * vgo1_p1_n1[3]));
    }
    {
        let sgo1_p1_z = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 0.0))), 0);
        let vgo1_p1_z = relu_neg(sgo1_p1_z);
        out0 = out0 + ((0.1966458 * vgo1_p1_z[0]) + (0.0016302796 * vgo1_p1_z[1]) + (-0.25712642 * vgo1_p1_z[2]) + (-0.09639119 * vgo1_p1_z[3]));
        out1 = out1 + ((-0.006955351 * vgo1_p1_z[0]) + (0.10882133 * vgo1_p1_z[1]) + (0.1107341 * vgo1_p1_z[2]) + (0.062697805 * vgo1_p1_z[3]));
        out2 = out2 + ((-0.1074494 * vgo1_p1_z[0]) + (0.17361663 * vgo1_p1_z[1]) + (0.6429869 * vgo1_p1_z[2]) + (-0.39846307 * vgo1_p1_z[3]));
        out3 = out3 + ((-0.26302996 * vgo1_p1_z[0]) + (0.048710946 * vgo1_p1_z[1]) + (0.40387508 * vgo1_p1_z[2]) + (0.4299715 * vgo1_p1_z[3]));
    }
    {
        let sgo1_p1_p1 = textureLoad(source0,
            vec2<i32>(floor(uv * src_dims + vec2<f32>(1.0, 1.0))), 0);
        let vgo1_p1_p1 = relu_neg(sgo1_p1_p1);
        out0 = out0 + ((0.18948616 * vgo1_p1_p1[0]) + (0.24086732 * vgo1_p1_p1[1]) + (-0.064474985 * vgo1_p1_p1[2]) + (-0.11069709 * vgo1_p1_p1[3]));
        out1 = out1 + ((0.1279659 * vgo1_p1_p1[0]) + (-0.13438123 * vgo1_p1_p1[1]) + (-0.028438117 * vgo1_p1_p1[2]) + (0.125883 * vgo1_p1_p1[3]));
        out2 = out2 + ((0.018153818 * vgo1_p1_p1[0]) + (-0.21942288 * vgo1_p1_p1[1]) + (0.020390838 * vgo1_p1_p1[2]) + (-0.22797634 * vgo1_p1_p1[3]));
        out3 = out3 + ((-0.10821287 * vgo1_p1_p1[0]) + (-0.17175092 * vgo1_p1_p1[1]) + (0.122016855 * vgo1_p1_p1[2]) + (0.20699544 * vgo1_p1_p1[3]));
    }
    out0 = out0 + (-0.05101961);
    out1 = out1 + (-0.060740646);
    out2 = out2 + (-0.024465766);
    out3 = out3 + (0.058471628);
    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));
}
