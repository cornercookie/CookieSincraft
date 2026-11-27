# 系統架構

| 文件資訊 | 內容 |
| --- | --- |
| 版本／建立／更新 | v1.5／2026-11-27／2026-11-27 |
| 變更摘要 | v1.5 依 opencode-gpu-port.md 增加 GPU 演算法：FSR 1.0（EASU＋RCAS，`fsr_easu.wgsl`／`fsr_rcas.wgsl`，上游 commit a21ffb8）、NIS（`nis_scaler.wgsl`，上傳 64×8 係數並由 NVScaler 索引，上游 commit 35e13ba）、Anime4K CNN x2 M（`anime4k_cnn_passes/` 9 個 pass，上游 commit 7684e95），並與 Bilinear 共用 wgpu 裝置、錯誤旗標與錯誤區域（`gpu.rs`）；新增 `scripts/glsl_to_wgsl_anime4k.py` 轉換腳本與其產物。`error_flag` 與 `on_uncaptured_error` 為同一 Arc；GPU 失敗回 `Err`，由 `commands.rs` 改跑 CPU Lanczos3。測試 1–3 已通過，視為「已移植」。 |
| 權威來源 | `Cargo.toml`、`src-tauri/Cargo.toml`、`Cargo.lock`、`src-tauri/tauri.conf.json`、`src/package.json`、`src/vite.config.js`、`src-tauri/src/main.rs`、`src-tauri/src/gpu.rs`、`src-tauri/src/cpu.rs`、`src-tauri/shaders/bilinear.wgsl`、`src-tauri/shaders/fsr_easu.wgsl`、`src-tauri/shaders/fsr_rcas.wgsl`、`src-tauri/shaders/nis_scaler.wgsl`、`src-tauri/shaders/anime4k_cnn_passes/*.wgsl`、`scripts/glsl_to_wgsl_anime4k.py` |

## 技術堆疊

| 層 | 技術（`Cargo.lock`／`node_modules` 實際版本） | 選用理由 |
| --- | --- | --- |
| 桌面框架 | Tauri 2（`tauri` 2.12.7，CLI 2.12.6） | 以 Rust 處理影像、系統 WebView 顯示介面，執行檔小 |
| 前端 | Vite 5.4.21、原生 JavaScript、`@tauri-apps/api` 2.12.5 | 介面只有單一頁面，不需框架 |
| 影像編解碼 | `image` 0.24.9（啟用 png、jpeg、gif、webp、pnm、tiff、bmp、ico、hdr） | 一個套件涵蓋所有支援格式 |
| CPU 放大 | `fast_image_resize` 0.7.0 | 提供 Nearest／Bilinear／Catmull-Rom／Lanczos3 |
| GPU 放大 | `wgpu` 0.19.4（啟用 dx12、vulkan-portability）＋ WGSL 運算著色器 | 不依賴特定廠商 SDK 即可在 GPU 執行 |
| 檔案對話框 | `rfd` 0.13.0 | 後端直接取得實體路徑 |
| 預覽傳輸 | `base64` 0.22 | 預覽以 data URL 傳給前端，免開檔案存取權限 |

`Cargo.toml` 中的 Tauri 相依宣告為 `2.0.0-beta`，實際解析版本以 `Cargo.lock` 為準。

## 目錄結構

| 路徑 | 內容 |
| --- | --- |
| `Cargo.toml` | Cargo 工作區，成員只有 `src-tauri` |
| `src/` | 前端原始碼與 npm 設定；`npm run build` 輸出到 `src-tauri/gen/dist` |
| `src-tauri/src/` | `main.rs`（啟動與指令註冊）、`commands.rs`、`converter.rs`、`cpu.rs`、`gpu.rs`、`upscaler.rs` |
| `src-tauri/shaders/` | 著色器：`bilinear.wgsl`（雙線性）、`fsr_easu.wgsl`／`fsr_rcas.wgsl`（FSR 1.0 兩 pass）、`nis_scaler.wgsl`（NIS NVScaler，binding 4/5 為係數緩衝）、`anime4k_cnn_passes/`（Anime4K CNN x2 M，9 個 pass，由 `scripts/glsl_to_wgsl_anime4k.py` 從上游 GLSL 機械產生）。程式以 `include_str!` 嵌入 `gpu.rs`。 |
| `src-tauri/tests/integration_test.rs` | 整合測試，不再宣告全域配置器；對齊配置器測試移至 `aligned_allocator_tests.rs` |
| `src-tauri/tests/aligned_allocator_tests.rs` | 新增：獨立測試二進制檔，宣告 `#[global_allocator]` 僅作用於本二進制 |
| `src-tauri/icons/` | 應用程式與系統匣圖示 |
| `src-tauri/gen/` | 前端建置產物與 Tauri 產生的 schema，不進版控 |
| `target/` | Rust 建置產物與安裝檔，不進版控 |

## 建置與發布

```mermaid
flowchart LR
    a["src/ 執行 npm run build"] --> b["src-tauri/gen/dist"]
    b --> c["根目錄執行 tauri build"]
    c --> d["target/release/cookie-sincraft.exe"]
    c --> e["target/release/bundle/ NSIS 與 MSI 安裝檔"]
    d --> f["手動上傳 GitHub Releases"]
    e --> f
```

- `tauri.conf.json` 只設定 `frontendDist: gen/dist`，沒有 `devUrl`、`beforeDevCommand`、`beforeBuildCommand`；打包前必須先手動建置前端，否則會包到舊的或不存在的前端。CI 工作流（`.github/workflows/ci.yml`）在打包前自動執行 `npm run build`。
- 發布物：NSIS 安裝檔、MSI 安裝檔、免安裝執行檔（需系統已有 WebView2），見 `README.md`。GitHub Actions CI 自動執行格式檢查、clippy、測試、前端與 Tauri 建置及版本一致性檢查；發布仍為手動上傳 GitHub Releases。
- 應用程式識別碼 `com.cookiesincraft.dev`；版本 0.2.0 同時寫在 `src-tauri/Cargo.toml`、`tauri.conf.json`、`src/package.json`。版本號同步腳本（`scripts/sync-version.js`）可在 CI 中以 `--check` 模式驗證三處一致性。

## 執行期設定

- 單一視窗，標題「CookieSincraft 曲奇芯刻」，1200×800，可調整大小；啟用系統匣圖示（`icons/icon.png`）。
- 發行版以 `windows_subsystem = "windows"` 隱藏主控台（`main.rs:1`）。除錯版啟動時自動開啟開發者工具（`main.rs:23`）。
- 安全相關設定（CSP、capability、開發者工具功能旗標）見[使用者權限](user-permissions.md)。

## 放大子系統

```mermaid
flowchart TB
    cmd["commands::upscale"] --> sel{"backend"}
    sel -->|cpu| cpu["CpuUpscaler<br/>fast_image_resize"]
    sel -->|gpu| new["GpuUpscaler::new<br/>建立 wgpu 實例與裝置"]
    new --> sel2{"algorithm"}
    sel2 -->|Bilinear| bil["bilinear pipeline"]
    sel2 -->|FSR 1.0| fsr["EASU (rgba16float) → RCAS"]
    sel2 -->|NIS| nis["NVScaler pipeline<br/>coef_scale／coef_usm 64×8"]
    sel2 -->|Anime4K| anime["9-pass CNN x2 M<br/>conv×7 → conv2d_last → depth2space"]
    bil --> shader["上傳材質 → 運算著色器 → 讀回"]
    fsr --> shader
    nis --> shader
    anime --> shader
    new -.->|"失敗"| fb["改用 CpuUpscaler<br/>Lanczos3"]
    shader -.->|"失敗"| fb
```

- **GPU：** 每次放大都重新建立 wgpu 實例、介面卡與裝置，不快取（`gpu.rs:215`）。只列舉 DX12 介面卡並取第一個（`gpu.rs:219`）；沒有 DX12 介面卡時回傳「backend unavailable」。來源列以 256 bytes 對齊上傳，16×16 工作群組運算後複製到暫存緩衝區讀回。三個演算法共用同一套繫結配置與參數緩衝區。
- **GPU：** 每次放大都重新建立 wgpu 實例、介面卡與裝置，不快取（`gpu.rs`）。只列舉 DX12 介面卡並取第一個；沒有 DX12 介面卡時回傳「backend unavailable」。來源列以 256 bytes 對齊上傳，16×16 工作群組運算後複製到暫存緩衝區讀回。各演算法共用同一套繫結配置（單紋理 pass 與 NIS 雙係數緩衝 pass、Anime4K 雙來源 pass）與參數緩衝區。
- **GPU 著色器：** `bilinear.wgsl`、`fsr_easu.wgsl`、`fsr_rcas.wgsl`、`nis_scaler.wgsl` 與 `anime4k_cnn_passes/` 9 個 pass 的 `@binding` 與 Rust bind group layout 對齊（binding 0 材質、1 sampler、2 storage、3 uniform；NIS 額外 binding 4/5 為唯讀係數儲存緩衝；Anime4K 7/8 用雙來源繫結）。FSR 1.0 EASU 以 `textureLoad`＋邊界夾取實作 12-tap（非 3×3 銳化），中繼 rgba16float，RGB 走 FSR、alpha 以原圖雙線性；RCAS sharpness＝exp2(-stops)，預設 `RCAS_DEFAULT_STOPS`（非上游指定預設），alpha 穿透。NIS 上傳 64×8 係數並由 WGSL 索引（非死表），單軸 1×–2×。Anime4K 權重機械複製自上游 GLSL（如 `0.16265294`）。`create_shader_module_safe` / `create_compute_pipeline_safe` 以 `catch_unwind` 將 wgpu 驗證 panic 轉成 `Err`，`error_flag` 為 `on_uncaptured_error` 寫入的同一 Arc。GPU 失敗時備援 CPU Lanczos3。
- **CPU：** 強制使用純量路徑 `CpuExtensions::None`（`cpu.rs:111`），避開 `fast_image_resize` 的 AVX2／SSE4 路徑在 Lanczos3 讀取未對齊係數時 panic；來源像素另複製到 16-byte 對齊緩衝區（`cpu.rs:71`）。
- **執行緒：** 大部分指令為同步函式（未標 `async`），依 Tauri 2 規則在主執行緒執行；`upscale` 已為非同步（`pub async fn`），透過 `tokio::task::spawn_blocking` 在背景執行放大，並以 `Channel<UpscaleProgress>` 回報進度，放大期間視窗可回應。`batch_upscale` 亦為非同步，透過 `batch_progress` 事件回報批次總進度。一次處理一個放大工作（`running_jobs` 互斥），期間無法發起新的單次 `upscale`。

## 非功能性特性

- **記憶體：** 整張圖解碼進記憶體並轉為 RGBA8，放大過程同時存在來源、對齊副本與輸出緩衝區。目標尺寸上限 4096×4096（對應約 64 MB RGBA8 緩衝區），超出尺寸會被 `validate_dimensions` 拒絕；8K 輸出（7680×4320）單一 RGBA8 緩衝區約 133 MB，超過 4096 上限。未勾選保持長寬比時，前端會在送出前拒絕 8K（`src/app.js:638-646`），不會讓後端尺寸檢查失敗。
- **效能：** CPU 放大使用純量路徑，沒有 SIMD 加速；GPU 每次重建裝置有固定啟動成本。沒有量測基準。
- **併發：** 沒有工作佇列；一次處理一個放大工作，期間主執行緒阻塞。
- **可用性：** 純本機運作，不需網路。GPU 不可用時的設計是改用 CPU，實際限制見[業務邏輯](business-logic.md)的「GPU→CPU 備援」。
