# 修正 v0.1.0 放大流程與安全設定缺失

> 本文件為規劃文件，不完全是實作依據。

---

## Description

修正整理 v0.1.0 現況（`Agents/`）時發現的五項問題：

1. 畫面上選 GPU「FSR 1.0」時放大一定失敗。
2. GPU 失敗後改用 CPU 的備援一定失敗。
3. GPU 著色器有錯誤，會讓程式 panic，而不是回傳錯誤。
4. 放大期間視窗無法回應；進度條是放大完成後才由前端模擬；取消按鈕沒有實際作用。
5. 權限面沒有任何限制：內容安全政策（Content Security Policy，CSP）關閉、沒有 Tauri capability 設定、IPC 指令接受任意檔案路徑、發行版也能開啟開發者工具。

現況細節與程式位置見 `Agents/service-architecture.md`、`Agents/system-architecture.md`、`Agents/business-logic.md`、`Agents/user-permissions.md`。

### User Story

無補充

## To Reproduce

環境：Windows 11，本機有 DX12 介面卡，程式基準 `main`（`cb55693`，v0.1.0）。第 1、3 項於 2026-11-22 以暫時測試直接呼叫後端 `upscale` 指令重現；第 2、4、5 項依程式碼判讀，尚未在執行中的 App 實際操作。

1. **FSR 1.0：** 載入任一圖片 → 執行端選 GPU → 演算法選「FSR 1.0」→ 按 Start Upscale。後端回傳「invalid algorithm 'fsr 1.0': expected nearest, bilinear, catmullrom, lanczos3, fsr1_0, anime4k, or nis」。原因：前端送出 `FSR 1.0`（`src/app.js:38`），後端只接受 `fsr1_0`／`fsr-1-0`／`fsr1`（`src-tauri/src/commands.rs:87`）。
2. **CPU 備援：** 在 GPU 建立裝置或執行失敗而回傳錯誤時，`upscale` 以同一個工作改用 CPU（`commands.rs:256`），但演算法仍是 GPU 演算法，CPU 放大器拒絕（`src-tauri/src/cpu.rs:50`），結果為「GPU upscaler failed: …; CPU fallback also failed: unsupported algorithm …」。整合測試在備援時自行改用 Lanczos3（`src-tauri/tests/integration_test.rs:164`），因此測不出此問題。
3. **GPU 著色器 panic：** 執行端選 GPU → 演算法選 Anime4K 或 NIS → 按 Start Upscale。後端在建立著色器時 panic：「Shader 'gpu upscaler shader' parsing error: name `sampler` is a reserved keyword」。`commands.rs` 未攔截 panic，不會進入 CPU 備援。執行中 App 在此情況下的畫面表現尚未實際操作確認。
4. **視窗無回應與假進度／取消：** 選大尺寸目標（如 7680×4320）放大。所有指令是同步函式，在主執行緒執行，放大期間視窗無法操作；`upscale` 回傳後前端才以計時器把進度條從 0% 跑到 100%（`app.js:232`）；`cancel_job` 為空實作（`commands.rs:292`）。
5. **權限：** `src-tauri/tauri.conf.json` 設定 `"csp": null`；專案沒有 `src-tauri/capabilities/`；`load_image`、`upscale`、`export_image`、`convert_image` 接受前端傳入的任意絕對路徑，後端不檢查範圍；`src-tauri/Cargo.toml` 的 `tauri` 啟用 `devtools` 功能旗標，發行版可開啟開發者工具。

## Expected behavior

1. 畫面上選 GPU「FSR 1.0」可以正常執行放大。
2. GPU 失敗時，CPU 備援能成功輸出圖檔，回傳結果標示已改用 CPU。
3. GPU 著色器錯誤不再使程式 panic；選 GPU 的三種演算法時，結果為 GPU 成功輸出，或進入 CPU 備援。
4. 放大在背景執行，視窗保持可回應；進度條顯示後端回報的實際進度；取消可中斷進行中的放大。
5. IPC 指令只能存取經檔案對話框選過的路徑，例外：`upscale` 可寫入由已允許輸入路徑衍生的 `<原檔名>_upscaled.<原副檔名>`，該路徑加入可讀清單後可供 `export_image` 讀取；設定嚴格 CSP 與 capability；發行版關閉開發者工具；移除未使用的外掛。

## Acceptance Criteria

- [ ] 選 GPU「FSR 1.0」放大，不再出現「invalid algorithm」錯誤。
- [ ] GPU 的 FSR 1.0 以 AMD FSR 1.0 的 EASU 加 RCAS 實作，NIS 以 NVIDIA Image Scaling 實作，Anime4K 以 Anime4K 官方著色器實作；著色器檔放在 `src-tauri/shaders/`，`gpu.rs` 不再內嵌著色器原始碼。
- [ ] 選 GPU 的 FSR 1.0、Anime4K、NIS 放大，程式都不再 panic：GPU 成功輸出，或進入 CPU 備援。
- [ ] GPU 失敗時，CPU 以 Lanczos3 成功寫出目標尺寸的圖檔，`upscale` 回傳 `used_fallback` 為 `true`，狀態列顯示「GPU 失敗，已改用 CPU（Lanczos3）」。
- [ ] 放大期間視窗可以操作（可移動、可點按鈕），不會卡住到放大結束。
- [ ] 進度條顯示後端回報的實際進度，不再於放大完成後才模擬跑進度。
- [ ] 放大進行中按取消，放大停止、不寫出輸出檔，畫面回到可重新開始的狀態。
- [ ] 前端以未經檔案對話框選取的路徑呼叫 `load_image`、`upscale`、`export_image`、`convert_image`，後端拒絕。例外：`upscale` 可寫入由已允許輸入路徑衍生的 `<原檔名>_upscaled.<原副檔名>`，該路徑加入可讀清單後可供 `export_image` 讀取（2026-11-22 使用者確認，見 Design Q5）。
- [ ] `tauri.conf.json` 的 CSP 只允許程式內建資源，專案具備 capability 設定。
- [ ] 發行版無法開啟開發者工具（`tauri` 不再啟用 `devtools` 功能旗標）。
- [ ] 移除未使用的 `tauri-plugin-dialog`、`tauri-plugin-shell`。

## Additional context

本 Planning 由 Agent 依使用者 2026-11-22 指示「修這些」起草，問題清單來自同日整理 `Agents/` 時回報的五項發現。各待確認事項由使用者於 2026-11-22 回覆「依你建議」，採用 Agent 提出的建議選項（第一順位；權限一題四項全選），已寫入 Expected behavior 與 Acceptance Criteria。接續的設計與實作依 `dev-workflow` 技能產出同資料夾的 `Design.md`。

**已確認（2026-11-22，使用者回覆「依你建議」）**

- **GPU 著色器：** 改成符合原始演算法（FSR 1.0 EASU＋RCAS、NIS、Anime4K），沿用並修正 `src-tauri/shaders/*.wgsl`，刪除 `gpu.rs` 內嵌版本。
- **CPU 備援：** 固定用 Lanczos3，狀態列提示「GPU 失敗，已改用 CPU（Lanczos3）」。
- **進度條與取消：** 實作真實進度與可中斷的取消（放大改在背景執行緒，後端回報進度）。
- **權限：** IPC 指令只接受經檔案對話框選過的路徑（後端記錄允許清單）；例外：`upscale` 可寫入由已允許輸入路徑衍生的 `<原檔名>_upscaled.<原副檔名>`，該路徑加入可讀清單後可供 `export_image` 讀取；設定嚴格 CSP（只允許本機內建資源）並建立 capability 檔；發行版關閉開發者工具（移除 `devtools` 功能旗標）；移除未使用的 `tauri-plugin-dialog`、`tauri-plugin-shell`。
- **範圍與拆分：** 五項同一個任務 [#1]；其他已知落差列為非本期範圍。

**待確認**

- 無

**非本期範圍**

- `Agents/service-architecture.md`「已知落差」中的其他項目：拖放不讀取拖入的檔案、未使用的 `#fileInput`、除錯輸出（`eprintln!`、`console.log`）、範本殘留的 `greet` 指令。
