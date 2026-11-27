#!/usr/bin/env python3
"""Convert Anime4K GLSL hooks to individual WGSL compute shaders.

Mechanically copies the weight matrices from the GLSL (e.g. the mat4 containing
0.16265294 at Anime4K_Upscale_CNN_x2_M.glsl:230) into the corresponding WGSL.
Each //!HOOK becomes one WGSL file with a single `main` entry point.

Outputs one file per hook into shaders/anime4k_cnn_passes/.
"""
import re
from pathlib import Path

GLSL = Path(
    r"C:\tempDevelopmenyEnviroment\CookieSincraft\docs\001-project-review-and-gap-fix"
    r"\upstream\Anime4K_Upscale_CNN_x2_M.glsl"
)
OUTDIR = Path(
    r"C:\tempDevelopmenyEnviroment\CookieSincraft\src-tauri\shaders\anime4k_cnn_passes"
)


def split_hooks(text):
    """Split into blocks, each starting at a //!HOOK MAIN line."""
    parts = re.split(r"(?=//!HOOK MAIN)", text)
    blocks = [p for p in parts if "//!HOOK MAIN" in p]
    return blocks


def parse_meta(block):
    meta = {}
    for key in ["DESC", "HOOK", "BIND", "SAVE", "WIDTH", "HEIGHT", "COMPONENTS", "WHEN"]:
        m = re.search(r"//!%s\s+(.*)" % key, block)
        if m:
            meta[key] = m.group(1).strip()
    # BIND may appear multiple times
    meta["BINDs"] = re.findall(r"//!BIND\s+(.*)", block)
    return meta


def extract_mat4s(block):
    """Extract each `mat4(...) * go_N(dx,dy)` or `mat4(...) * g_N` term
    plus the trailing `result += vec4(...)`.

    Returns list of tuples (nums, kind, index, dx, dy):
      kind="go": index = go function (0 or 1), dx/dy = texel offset
      kind="g" : index = g number (0..13), dx/dy = 0 (1x1 conv, no offset)
    plus bias vec4.
    """
    terms = []
    # mat4(...) * go_0(dx, dy)
    pat_go = re.compile(
        r"mat4\(\s*([\s\S]*?)\s*\)\s*\*\s*go_(\d)\(\s*(-?\d+\.?\d*)\s*,\s*(-?\d+\.?\d*)\s*\)"
    )
    for m in pat_go.finditer(block):
        nums = [float(x) for x in re.findall(r"-?\d+\.?\d*(?:e[+-]?\d+)?", m.group(1))]
        if len(nums) != 16:
            continue
        terms.append((nums, "go", int(m.group(2)), float(m.group(3)), float(m.group(4))))
    # mat4(...) * g_N   (conv2d_last 1x1x56, no texel offset)
    pat_g = re.compile(r"mat4\(\s*([\s\S]*?)\s*\)\s*\*\s*g_(\d+)")
    for m in pat_g.finditer(block):
        nums = [float(x) for x in re.findall(r"-?\d+\.?\d*(?:e[+-]?\d+)?", m.group(1))]
        if len(nums) != 16:
            continue
        terms.append((nums, "g", int(m.group(2)), 0.0, 0.0))
    # bias vec4(...)
    bias = None
    bm = re.search(r"result\s*\+=\s*vec4\(([\s\S]*?)\);", block)
    if bm:
        bias = [float(x) for x in re.findall(r"-?\d+\.?\d*(?:e[+-]?\d+)?", bm.group(1))]
    return terms, bias


def fmt_vec(vals):
    return "vec4<f32>(" + ", ".join(f"{v:.10g}" for v in vals) + ")"


def coord_suffix(d):
    i = int(round(d))
    if i < 0:
        return f"n{abs(i)}"
    if i > 0:
        return f"p{i}"
    return "z"


def column_major_columns(nums):
    """GLSL mat4 constructor args are column-major. Return 4 columns (each length 4)."""
    cols = []
    for c in range(4):
        cols.append([nums[c * 4 + r] for r in range(4)])
    return cols


def gen_wgsl(meta, terms, bias, idx, total):
    name = meta.get("SAVE", f"pass{idx}")
    binds = meta.get("BINDs", ["MAIN"])
    n_src = len(binds)

    lines = []
    lines.append("// Anime4K CNN x2 (M) — pass %d/%d: %s" % (idx, total, name))
    lines.append("// Source: docs/.../upstream/Anime4K_Upscale_CNN_x2_M.glsl (commit 7684e95)")
    lines.append("// License: MIT (bloc97). File header retained above; see upstream GLSL.")
    lines.append("// GLSL DESC: %s" % meta.get("DESC", ""))
    lines.append("// GLSL BIND: " + ", ".join(binds))
    lines.append("// GLSL SAVE: %s" % meta.get("SAVE", ""))
    lines.append("")
    lines.append("struct Params {")
    lines.append("    src_width: f32,")
    lines.append("    src_height: f32,")
    lines.append("    dst_width: f32,")
    lines.append("    dst_height: f32,")
    lines.append("    _pad: f32,")
    lines.append("}")
    lines.append("")

    # Source bindings
    for b in range(n_src):
        lines.append(f"@group(0) @binding({b}) var source{b}: texture_2d<f32>;")
    lines.append(f"@group(0) @binding({n_src}) var smp: sampler;")
    lines.append(f"@group(0) @binding({n_src + 1}) var dest: texture_storage_2d<rgba8unorm, write>;")
    lines.append(f"@group(0) @binding({n_src + 2}) var<uniform> params: Params;")
    lines.append("")

    lines.append("fn relu_pos(v: vec4<f32>) -> vec4<f32> {")
    lines.append("    return max(v, vec4<f32>(0.0));")
    lines.append("}")
    lines.append("fn relu_neg(v: vec4<f32>) -> vec4<f32> {")
    lines.append("    return max(-v, vec4<f32>(0.0));")
    lines.append("")

    lines.append("@compute @workgroup_size(16, 16)")
    lines.append("fn main(@builtin(global_invocation_id) gid: vec3<u32>) {")
    lines.append("    let dst_dims = textureDimensions(dest);")
    lines.append("    let pos = vec2<u32>(gid.xy);")
    lines.append("    if pos.x >= dst_dims.x || pos.y >= dst_dims.y {")
    lines.append("        return;")
    lines.append("    }")
    lines.append("")
    lines.append("    let src_dims = vec2<f32>(params.src_width, params.src_height);")
    lines.append("    let fpos = vec2<f32>(f32(pos.x), f32(pos.y));")
    lines.append("    let uv = fpos / vec2<f32>(f32(dst_dims.x), f32(dst_dims.y));")
    lines.append("    let step = vec2<f32>(1.0) / src_dims;")
    lines.append("")

    lines.append("    var out0 = 0.0;")
    lines.append("    var out1 = 0.0;")
    lines.append("    var out2 = 0.0;")
    lines.append("    var out3 = 0.0;")
    lines.append("")

    for (nums, kind, index, dx, dy) in terms:
        # Resolve source index + polarity.
        if kind == "g":
            # conv2d_last 1x1x56: g_{2i}->binds[i] pos, g_{2i+1}->binds[i] neg
            src_idx = index // 2
            if src_idx >= n_src:
                src_idx = n_src - 1
            gof = index % 2  # 0 = relu_pos, 1 = relu_neg
            offx, offy = 0.0, 0.0  # 1x1 conv: no texel offset
        else:
            # go_N(dx,dy): binds[N], N=0 pos / N=1 neg
            src_idx = index
            if src_idx >= n_src:
                src_idx = n_src - 1
            gof = index
            offx, offy = dx, dy
        sx = coord_suffix(offx)
        sy = coord_suffix(offy)
        cols = column_major_columns(nums)
        lines.append("    {")
        lines.append(f"        let s{kind}{index}_{sx}_{sy} = textureLoad(source{src_idx},")
        lines.append(
            f"            vec2<i32>(floor(uv * src_dims + vec2<f32>({offx:.1f}, {offy:.1f}))), 0);"
        )
        lines.append(
            f"        let v{kind}{index}_{sx}_{sy} = "
            f"{'relu_pos' if gof == 0 else 'relu_neg'}(s{kind}{index}_{sx}_{sy});"
        )
        for c in range(4):
            w = cols[c]
            terms_str = " + ".join(
                f"({w[k]:.10g} * v{kind}{index}_{sx}_{sy}[{k}])" for k in range(4)
            )
            lines.append(f"        out{c} = out{c} + ({terms_str});")
        lines.append("    }")

    if bias:
        lines.append(f"    out0 = out0 + ({bias[0]:.10g});")
        lines.append(f"    out1 = out1 + ({bias[1]:.10g});")
        lines.append(f"    out2 = out2 + ({bias[2]:.10g});")
        lines.append(f"    out3 = out3 + ({bias[3]:.10g});")

    lines.append("    textureStore(dest, pos, vec4<f32>(out0, out1, out2, out3));")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def main():
    text = GLSL.read_text(encoding="utf-8")
    blocks = split_hooks(text)
    OUTDIR.mkdir(parents=True, exist_ok=True)
    # Remove old outputs
    for f in OUTDIR.glob("*.wgsl"):
        f.unlink()

    total = len(blocks)
    print(f"Found {total} hooks")
    for i, block in enumerate(blocks):
        meta = parse_meta(block)
        terms, bias = extract_mat4s(block)
        save = meta.get("SAVE", f"pass{i}")
        safe = re.sub(r"[^A-Za-z0-9_]", "_", save)
        wgsl = gen_wgsl(meta, terms, bias, i + 1, total)
        out = OUTDIR / f"{safe}.wgsl"
        out.write_text(wgsl, encoding="utf-8")
        print(f"  {i+1:2d}/{total} {save:22s} -> {out.name}  terms={len(terms)} bias={'yes' if bias else 'no'}")


if __name__ == "__main__":
    main()
