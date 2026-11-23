# CookieSincraft 專案審查與缺失修復規劃

## 專案現況摘要

CookieSincraft 是 Tauri 2 + Rust 的 Windows 桌面圖片放大與格式轉換工具，目前版本 v0.2.0。專案已有基本架構、前後端通訊、CPU 放大（Lanczos3 等）、檔案對話框、預覽、匯出/轉換功能。

**但存在多項關鍵缺失，導致核心功能（GPU 放大）不可用、使用者體驗有缺口、安全設定不完整。**

---

## 已識別缺失清單（依嚴重度排序）

### 🔴 P0：核心功能失效

| # | 缺失 | 影響 | 位置 |
|---|------|------|------|
| 1 | **GPU 著色器全部無法通過 wgpu 驗證（語法錯誤）** | 所有 shader 含 `sampler` 保留字衝突、`let mut` 非 WGSL、HLSL 風格函數參數（`fn fx2(vec2 v: vec2<f32>)`），`create_shader_module` 直接 panic | `src-tauri/shaders/*.wgsl`、`src-tauri/src/gpu.rs:114-162` |
| 2 | **Anime4K 著色器缺少 `main` 入口點** | `gpu.rs:157` 建立 pipeline 指定 `entry_point: "main"`，但 shader 定義 4 個非 main 入口，建立即失敗 | `src-tauri/shaders/anime4k_cnn_x2_m.wgsl`、`src-tauri/src/gpu.rs:157` |
| 3 | **FSR 1.0 僅實作 RCAS 銳化，缺少 EASU 放大通道** | 輸出品質不符 FSR 1.0 規格；`fsr_easu.wgsl` 存在但未建立 pipeline | `src-tauri/src/gpu.rs:249`、`src-tauri/shaders/fsr_easu.wgsl` |
| 4 | **GPU 備援邏輯錯誤** | 備援時沿用 GPU 演算法，CPU 放大器拒絕 → 雙重失敗 | `src-tauri/src/commands.rs:420-426` |
| 5 | **GPU 著色器 panic 未被攔截** | `create_shader_module` 直接 panic，錯誤處理無效 | `src-tauri/src/gpu.rs:114-162` |
| 6 | **衍生輸出路徑（`_upscaled`）未加入寫入允許清單** | 首次放大即會因 `access denied` 失敗；`AppState` 僅在對話框加入路徑 | `src-tauri/src/commands.rs:369`、`src-tauri/src/app.js:138-143` |

### 🟠 P1：使用者體驗缺口

| # | 缺失 | 影響 | 位置 |
|---|------|------|------|
| 7 | **進度顆粒度不足** | `async upscale` + `Channel` 架構已存在（`commands.rs:346`、`app.js:225-270`），但僅 3 階段（loading/upscaling/saving），放大過程無細粒度進度 | `src-tauri/src/commands.rs:346`、`src/app.js:225-270` |
| 8 | **取消僅在階段邊界生效** | `commands.rs:399,437` 有兩個檢查點，但 `gpu.rs`、`cpu.rs` 內部無中途檢查，大圖放大不可中斷 | `src-tauri/src/commands.rs:399,437`、`src-tauri/src/gpu.rs`、`src-tauri/src/cpu.rs` |
| 9 | **拖放不讀取檔案** | 放下時改開對話框，`<input type="file">` 未使用 | `src/app.js:194-198`、`src/index.html:22` |
| 10 | **放大結果直接覆寫同名檔** | 不詢問確認，風險高 | `src/app.js:138-143` |
| 11 | **預覽不更新放大後結果** | 無法即時比對放大效果 | `src/app.js:121`、`src-tauri/src/commands.rs:273-276` |
| 12 | **`used_fallback` 前端僅狀態列顯示** | 無視覺化提示（如圖示、彈窗） | `src/app.js:242-246` |
| 13 | **無輸入/輸出尺寸上限檢查** | 8K 輸出單緩衝 133 MB，可能導致 OOM 或 DoS | `Agents/system-architecture.md:77` |

### 🟡 P2：安全與架構

| # | 缺失 | 影響 | 位置 |
|---|------|------|------|
| 14 | **無 Capability 設定** | 所有自訂指令無 ACL 保護，任何 WebView 腳本可任意讀寫檔案 | `src-tauri/tauri.conf.json`、無 `src-tauri/capabilities/` |
| 15 | **路徑允許清單機制存在但未嚴格執行** | `AppState` 有檢查但 `select_image`/`select_save_path` 才加入，衍生路徑（`_upscaled`）未加入寫入清單 | `src-tauri/src/commands.rs:78-146` |
| 16 | **CSP 過寬** | `img-src 'self' data:` 允許任意 data URL | `src-tauri/tauri.conf.json:21` |
| 17 | **NIS 著色器係數表不完整** | 僅含 8/64 階（`nis_scaler.wgsl:37` 註解承認截斷），生產環境需完整 64 階係數表上傳為 storage buffer | `src-tauri/shaders/nis_scaler.wgsl:28-38` |
| 18 | **整合測試用 `catch_unwind` 吞 GPU panic** | 手動改用 Lanczos3 判通過，無法驗證 GPU 路徑真正可用；CI 綠燈不代表 GPU 功能正常 | `src-tauri/tests/integration_test.rs:157-172` |

### 🔵 P3：程式碼品質與維護

| # | 缺失 | 影響 | 位置 |
|---|------|------|------|
| 19 | **`eprintln!` 除錯輸出殘留** | 正式版仍輸出到主控台 | `src-tauri/src/commands.rs:322, 332` |
| 20 | **前端 `console.log` 殘留** | 匯出/轉換流程有除錯輸出 | `src/app.js:315, 319, 349, 353` |
| 21 | **`greet` 指令殘留** | 未被前端呼叫 | `src-tauri/src/main.rs` invoke_handler 列表中無實作 |
| 22 | **測試用全域配置器污染** | `integration_test.rs` 設定 `#[global_allocator]` 影響其他測試 | `src-tauri/tests/integration_test.rs:34-35` |
| 23 | **無 CI/CD** | 發布為手動流程，無自動化測試/建置驗證 | 專案根目錄無 `.github/workflows/` |
| 24 | **版本號三處手動同步** | `Cargo.toml`、`tauri.conf.json`、`package.json` 易不一致；T05 未包含自動化同步腳本 | 三個檔案 |

### 🟣 P4：功能增強（非缺陷但建議）

| # | 建議 | 理由 |
|---|------|------|
| 25 | **支援保持長寬比放大** | 目前強制拉伸至目標解析度 |
| 26 | **支援自訂輸出資料夾/檔名樣板** | 目前固定 `_upscaled` 後綴 |
| 27 | **加入最近開啟檔案歷史** | 提升重複使用體驗 |
| 28 | **支援批次處理** | 單張處理效率低 |
| 29 | **加入設定檔持久化** | 記住上次使用的 backend/algorithm/quality |
| 30 | **支援 HDR 輸出** | 目前 `image` crate 僅支援讀取 |
| 31 | **WebP 有損編碼選項** | 目前僅無損 |

---

## 驗證基準（Definition of Done）

### P0 必須修復
- [ ] GPU 著色器能在 wgpu 通過驗證，`GpuUpscaler::new()` 不 panic
- [ ] Anime4K 重設計為單一 `main` 入口或多 pass pipeline 正確串接
- [ ] FSR 1.0 實作完整 EASU→RCAS 兩階段 pipeline
- [ ] NIS 上傳完整 64 階係數表（或標記已知限制並降級）
- [ ] GPU 失敗時正確備援到 CPU（Lanczos3），回傳 `used_fallback: true`
- [ ] 衍生輸出路徑（`_upscaled`）自動加入寫入允許清單，首次放大不失敗

### P1 必須修復
- [ ] 進度事件每 **10%**（或每 workgroup row）送出一次，最小間隔 100ms
- [ ] 取消在演算法內部加入檢查點（CPU/GPU 皆需支援 `Arc<AtomicBool>` token）
- [ ] 拖放能直接讀取檔案路徑（或綁定 `<input type="file">` change 事件）
- [ ] 放大輸出同名檔案時前端彈窗確認（或自動加流水號 `_upscaled_1` 等）
- [ ] 放大完成後預覽更新為放大結果（Completed 事件帶回 base64 preview）
- [ ] 加入輸入/輸出尺寸上限檢查：**max 4096×4096、單緩衝 64 MB**，超限直接回錯

### P2 必須修復
- [ ] 建立 `src-tauri/capabilities/main.json`，最小權限原則
- [ ] 路徑允許清單嚴格執行（含衍生輸出路徑）
- [ ] CSP 收斂：`img-src 'self'`（移除 `data:`），預覽改用 `blob:` URL 傳遞
- [ ] 修復整合測試：新增 shader module 驗證測試（不 dispatch、只驗證 create_shader_module 不 panic），移除誤導性 `catch_unwind`

### P3 建議修復
- [ ] 清除所有 `eprintln!`/`console.log`，改用 `tracing`/`log` crate 分級記錄
- [ ] **`greet` 指令已從 main.rs 移除**，需同步更新 `Agents/service-architecture.md` 移除相關記載
- [ ] 修復測試全域配置器問題（對齊配置器限制在測試函數內部或單獨測試二進制檔）
- [ ] 加入 GitHub Actions CI（`cargo test`、`cargo clippy`、`cargo fmt`、`tauri build` Windows runner）
- [ ] 版本號同步腳本（如 `cargo-workspace-version` 或自訂 script），納入 CI 檢查三處一致性

---

## 任務拆解建議

| 任務編號 | 任務名稱 | 包含缺失 | 預估工時 |
|----------|----------|----------|----------|
| T01a | GPU 著色器語法修復與通用管線驗證 | #1 (語法錯誤：sampler/let mut/HLSL 風格) | 3-5h |
| T01b | FSR 1.0 EASU→RCAS 兩階段 pipeline | #3 | 2-3h |
| T01c | Anime4K 重構為單一 main 入口或多 pass 正確串接 | #2 | 3-5h |
| T01d | NIS 完整 64 階係數表上傳 | #17 | 2-3h |
| T01e | GPU panic 攔截、備援邏輯修正、衍生路徑允許清單 | #4, #5, #6 | 2-3h |
| T01f | GPU 整合驗證與回歸測試 | #1, #18 (shader module 驗證測試) | 1-2h |
| T02 | 非同步放大進度細化、取消檢查點 | #7, #8 | 4-8h |
| T03 | 拖放支援、覆寫確認、預覽更新、尺寸限制 | #9, #10, #11, #12, #13 | 4-8h |
| T04 | Tauri Capability 與安全加固 | #14, #15, #16, #18 | 3-5h |
| T05 | 程式碼清理與 CI 建置 | #19, #20, #21, #22, #23, #24 | 2-4h |
| T06 | 功能增強（可選） | #25-#31 | 視優先級 |

---

## 依賴關係

```
T01a (語法修復) ──► T01b (FSR EASU/RCAS)
    │                    │
    │                    ▼
    │               T01c (Anime4K 重構)
    │                    │
    ▼                    ▼
T01d (NIS 係數表) ◄─── T01e (panic 攔截/備援/路徑)
    │                    │
    └───────────────────► T01f (整合驗證)
                              │
                              ▼
T02 (進度/取消) ──► T03 (UX 細節)
     │
     └─► T04 (安全) 可並行
           │
           └─► T05 (清理/CI) 可並行
                 │
                 └─► T06 (增強) 後續
```

### T01 內部依賴順序說明

1. **T01a 必須最先完成**：修正所有 shader 的 WGSL 語法錯誤（sampler 改名、移除 let mut、統一參數風格），建立可通過驗證的通用雙線性+銳化基礎管線
2. **T01b、T01c、T01d 可並行**：在通用管線通後，分別實作 FSR 兩階段、Anime4K 重構、NIS 係數表
3. **T01e 依賴 T01a**：需在 GPU 初始化流程加入 catch_unwind、修正 fallback 改用 Lanczos3、自動 allow_write 衍生路徑
4. **T01f 最後執行**：整合所有演算法驗證、新增 shader module 驗證測試（不 dispatch、只驗證 create_shader_module）

---

## 風險與對策

| 風險 | 可能性 | 影響 | 對策 |
|------|--------|------|------|
| GPU 著色器語法修復（T01a）比預期複雜，所有 shader 皆有 sampler/let mut/HLSL 風格錯誤 | 高 | T01a 延期阻塞後續 | 先建立最小可驗證管線（單一雙線性+銳化 shader），再逐步擴展 |
| FSR EASU 兩階段、Anime4K 重構、NIS 係數表（T01b/c/d）移植困難 | 中 | 個別演算法無法提供 | 以通用管線為兜底，缺失演算法標記「不可用」並隱藏選單項目 |
| GPU panic 攔截與備援邏輯（T01e）涉及非同步錯誤處理 | 中 | fallback 仍失敗 | 在 T01a 階段同步加入 catch_unwind 驗證，確保錯誤路徑可測試 |
| wgpu 在某些顯卡/驅動上行為不一致 | 中 | 發布後崩潰 | 保留 CPU 強制選項，GPU 失敗自動備援並明確提示 |
| Tauri 2 beta API 變更 | 低 | 建置失敗 | 鎖定 `tauri = "2.0.0-beta"`，升級前先在分支驗證 |
| 衍生路徑允許清單機制改動影響現有流程 | 中 | 放大/匯出失敗 | 在 T01e 優先修復 #6，並加入整合測試驗證首次放大 |
| T01 拆分導致整合驗證（T01f）延後發現介面不匹配 | 中 | 返工 | T01a 完成後即定義統一 pipeline 介面，各子任務依介面實作 |

---

## 後續步驟

1. 使用者確認優先級與範圍
2. 依序建立各任務的 `Design.md`（含 Mermaid 架構圖/時序圖）
3. 設計審查通過後開始實作
4. 每任務完成後驗證並更新 `Agents/` 對應文件