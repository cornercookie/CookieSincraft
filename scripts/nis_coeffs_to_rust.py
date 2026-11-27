#!/usr/bin/env python3
"""Extract coef_scale / coef_usm from NIS_Config.h and emit Rust fns."""
import re
from pathlib import Path

H = Path(
    r"C:\tempDevelopmenyEnviroment\CookieSincraft\docs\001-project-review-and-gap-fix"
    r"\upstream\NIS_Config.h"
)
text = H.read_text(encoding="utf-8")


def extract_array(name):
    # Find `constexpr float coef_scale[kPhaseCount][kFilterSize] = { ... };`
    marker = f"constexpr float {name}[kPhaseCount][kFilterSize] = {{"
    idx = text.find(marker)
    if idx == -1:
        raise SystemExit(f"array {name} not found")
    # start at the opening brace
    start = text.index("{", idx + len(marker) - 1)
    # brace-match to find the closing brace
    depth = 0
    end = start
    for i in range(start, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                end = i
                break
    body = text[start + 1:end]
    rows = []
    for row in re.findall(r"\{([^}]*)\}", body):
        nums = [float(x) for x in re.findall(r"-?\d+\.?\d*(?:e[+-]?\d+)?", row)]
        if len(nums) != 8:
            raise SystemExit(f"row in {name} has {len(nums)} nums: {row}")
        rows.append(nums)
    if len(rows) != 64:
        raise SystemExit(f"{name} has {len(rows)} rows, expected 64")
    return rows


scale = extract_array("coef_scale")
usm = extract_array("coef_usm")

# Sanity: first row of coef_scale == [0,0,1,0,0,0,0,0]
assert scale[0] == [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], scale[0]


def fmt_row(row):
    return "    [" + ", ".join(f"{v:.7g}f32" for v in row) + "],"


out = []
out.append("")
out.append("/// NIS coef_scale table (64 phases x 8 taps) from NIS_Config.h:274")
out.append("/// (commit 35e13ba). Shader indexes [phase][0..5] (kFilterSize=6).")
out.append("const NIS_COEF_SCALE: [[f32; 8]; 64] = [")
for row in scale:
    out.append(fmt_row(row))
out.append("];")
out.append("")
out.append("/// NIS coef_usm table (64 phases x 8 taps) from NIS_Config.h:341")
out.append("const NIS_COEF_USM: [[f32; 8]; 64] = [")
for row in usm:
    out.append(fmt_row(row))
out.append("];")
out.append("")
out.append("/// Return a reference to the NIS coef_scale table.")
out.append("fn nis_coef_scale() -> &'static [[f32; 8]; 64] {")
out.append("    &NIS_COEF_SCALE")
out.append("}")
out.append("")
out.append("/// Return a reference to the NIS coef_usm table.")
out.append("fn nis_coef_usm() -> &'static [[f32; 8]; 64] {")
out.append("    &NIS_COEF_USM")
out.append("}")
out.append("")

Path("src-tauri/src/gpu.rs").write_text(
    Path("src-tauri/src/gpu.rs").read_text(encoding="utf-8") + "\n".join(out),
    encoding="utf-8",
)
print("appended NIS coeffs:", len(scale), "scale rows", len(usm), "usm rows")
