# 專案審查：先前提交沒有讓程式可用

審查者：Hermes（Reviewer）。實作不在本次審查內，交給 OpenCode。
審查基準：`docs/001-project-review-and-gap-fix/planning.md` 的 Definition of Done，以及「使用者按得下去、檔案真的寫得出來、GPU 路徑不是假綠燈」。
驗證：2026-11-27 於倉庫根目錄執行 `cargo test -p cookie-sincraft`，結束碼 0（單元 7、aligned 9、integration 13 皆通過）。測試通過不能當成功能可用。

## 結論

不通過。CPU 放大的函式層測試能跑，但主流程在進放大之前就會斷。GPU「優化」是註解與簡化銳化，綁定組與著色器不一致，整合測試用 CPU Lanczos3 把失敗判成通過。

## 阻擋（程式不可用）

| # | 問題 | 證據 | 使用者會看到 |
| --- | --- | --- | --- |
| B1 | `load_image_from_base64`、`check_path_exists` 有 `#[tauri::command]`，未進 `generate_handler!` | `commands.rs:510`、`commands.rs:542`；`main.rs:23-37` 沒有這兩個；`app.js:313`、`app.js:405` 會呼叫 | 拖放載入失敗。按開始放大前先走 `confirmOverwrite` → `check_path_exists`，主流程直接失敗 |
| B2 | 衍生輸出路徑從未 `allow_write`，`check_write` 是完整字串比對 | `allow_write` 只在 `commands.rs:713`、`commands.rs:728`；`execute_upscale` 於 `commands.rs:781` 檢查輸出路徑；`app.js:509-513` 自行組 `_upscaled` 路徑 | 即使 B1 修好，第一次放大與批次仍是 `access denied`。選輸出資料夾只允許資料夾字串，不等於允許裡面的檔 |
| B3 | 進度 Channel 當 DOM event 讀 | Tauri `Channel.onmessage` 直接收到 payload（`src/node_modules/@tauri-apps/api/core.d.ts:84-88`）。`app.js:684-686` 讀 `event.data`。invoke 回傳後 `app.js:751-752` 不再更新 UI | 放大即使成功，進度、預覽、按鈕狀態也不會依事件更新 |
| B4 | 批次 invoke 形狀錯誤 | Rust 要 `dto: BatchUpscaleDto` + `on_event: Channel`（`commands.rs:976-979`）。前端 `app.js:272-280` 把欄位攤平，並把 Channel 當 `invoke` 第三參數。第三參數是 `InvokeOptions`，不是 Channel | 批次無法開始 |
| B5 | GPU 綁定與著色器相反，錯誤旗標是另一份 | Rust layout：0 材質、1 sampler、2 storage、3 uniform（`gpu.rs:66-105`、`gpu.rs:474-490`）。著色器：0 uniform、1 材質、2 sampler、3 storage（`shaders/fsr_easu.wgsl:16-20`）。`on_uncaptured_error` 寫的 `error_flag` 與 `Self` 裡新配置的不是同一份（`gpu.rs:57-63` 對 `gpu.rs:167`）。`create_compute_pipeline` 不在 `catch_unwind` 內 | GPU 驗證失敗。若 pipeline 建立 panic，`spawn_blocking` 變成 `upscale task panicked`，CPU 備援分支走不到（`commands.rs:824-844`、`commands.rs:894-898`） |

## 優化目標未達成

| 目標（planning DoD） | 現況 |
| --- | --- |
| FSR 1.0 EASU→RCAS | 兩個 pass 都是 3×3 鄰域銳化，不是 AMD EASU/RCAS（`fsr_easu.wgsl:56-67`、`fsr_rcas.wgsl:33-41`）。且僅接受單軸 1×–2×（`gpu.rs:249-255`），畫面目標是絕對解析度 1080p/4K/8K，小圖放大到 4K 會直接被拒 |
| Anime4K CNN 多 pass | 註解寫 8 個卷積，實作是單一 `main` 的雙邊銳化（`anime4k_cnn_x2_m.wgsl:34-68`）。`gpu.rs:334-336` 寫「Simplified: single pass for now」 |
| NIS 64 階係數 | 表有宣告，`main` 註明未使用，走雙線性加對比銳化（`nis_scaler.wgsl:105-133`） |
| 進度每 10% | 只有 5 / 10 / 90（`commands.rs:793-798`、`commands.rs:816-821`、`commands.rs:858-863`） |
| 取消進演算法內部 | 只在開始前與整段放大結束後檢查（`commands.rs:811-814`、`commands.rs:853-856`）。`Cancelled` 變體存在，但這條路徑送的是 `Failed` |
| 整合測試能證明 GPU | `aligned_allocator_tests.rs:157-181` 仍 `catch_unwind` 後改跑 Lanczos3 即通過 |
| Capability 最小權限涵蓋全部指令 | `build.rs:4-12` 只列 7 個舊指令。`capabilities/default.json` 沒有 settings、batch、多檔、輸出資料夾。未列入 app manifest 的指令不會被 ACL 擋，等於新指令繞過權限 |

`parse_algorithm` 已接受 `fsr 1.0`（`commands.rs:411`）。這是少數真的補上的舊落差，不能代表 GPU 可用。

## 文件落差

`Agents/business-logic.md`、`Agents/service-architecture.md` 仍描述 v0.1.0（取消空實作、拖放不讀檔、備援沿用 GPU 演算法）。程式已改但沒接上。以程式為準，文件在實作修正後要回寫，不能先改文件假裝修好。

## 測試不能當作驗收

`cargo test -p cookie-sincraft` 全綠，覆蓋的是格式往返、尺寸檢查、CPU Lanczos3 輸出尺寸、以及會把 GPU panic 吞掉的備援測試。沒有測：未註冊指令、衍生路徑 `check_write`、Channel 契約、批次 DTO、著色器與 bind group 是否一致。

## 複審（OpenCode 修改後）

複審者仍是 Hermes。OpenCode 摘要不當證據。複審讀了 `main.rs`、`build.rs`、`capabilities/default.json`、`commands.rs` 允許清單與取消分支、`gpu.rs`、`fsr_easu.wgsl` 綁定、`app.js` 的 Channel／批次／8K，並自行重跑 `cargo test -p cookie-sincraft`（結束碼 0；integration 16 通過、1 項 GPU 測試被 `--skip`／`#[ignore]`）。

| 原阻擋 | 複審 |
| --- | --- |
| B1 指令未註冊 | 關閉。`main.rs:29,38` 已註冊；`build.rs` 與 `default.json` 已列對應 allow |
| B2 衍生路徑 | 關閉到測試層。`execute_upscale` 在 `check_write` 前呼叫 `is_derived_output_allowed` 並 `allow_write`。同資料夾、`../`、其他磁碟有測試。未拒 Windows 保留裝置名 |
| B3 Channel | 關閉到程式層。`onmessage` 直接讀 payload；invoke 成功後會恢復按鈕。回傳值沒有補上預覽圖，漏事件時預覽仍可能不更新 |
| B4 批次 | 關閉到程式層。`invoke` 改為 `{ dto, onEvent }` |
| B5 GPU 綁定／panic | 部分關閉。綁定已對齊；`error_flag` 是同一份 Arc；pipeline 建立有 `catch_unwind` 轉 `Err`。畫面只留 Bilinear。FSR／NIS／Anime4K 回傳「尚未實作」並走 CPU 備援 |

優化目標（官方 FSR EASU、NIS 64-tap、Anime4K CNN）仍未達成，這次沒有再假裝達成。`shaders/fsr_easu.wgsl` 檔頭仍寫 AMD EASU，與實際用途不符，屬殘留不實註解。`aligned_allocator_tests.rs` 仍用 `catch_unwind` 測備援，不是 GPU 通過證明。進度仍只有 5／10／90。取消只在階段邊界生效，但失敗字串含 `cancelled` 時會送 `Cancelled`。

GUI、原生對話框、真實 GPU 出圖未實機驗證。未 commit。

## 複審：FSR／NIS／Anime4K 移植

naga 0.19.2 `parse_str` 對目前 `src-tauri/shaders` 下 14 個 wgsl 全部通過（含 FSR EASU／RCAS、NIS、Anime4K 9 個 pass、Bilinear）。`cargo test -p cookie-sincraft` 結束碼 0，其中 `every_included_wgsl_parses_with_naga` 通過。EASU 使用獨立的 `Rgba16Float` bind group（`gpu.rs` `fsr_easu_bind_group_layout`），不再跟 Bilinear 的 `Rgba8Unorm` layout 混用。

這只證明著色器能被解析、係數與 12-tap／9 pass 結構在。沒有實機 dispatch，不能證明輸出等於上游。Anime4K depth-to-space 只取 R 通道，不是上游的通道選擇。未 commit。

## 複審：DX12 出圖（2026-11-27）

先前 `gpu_dispatch_writes_target_size_png` 結束碼 101。原因是著色器過了 naga，但 DX12 建管線失敗，裝置留在 error state，連 Bilinear 也出不了圖。Hermes 直接修，沒有再交給 OpenCode。

已對上並修掉的驗證錯誤：

- FSR EASU 不再回傳 `array<vec4<f32>, 4>`。`load_offset` 回傳單一 `vec4`。殘留的 `ijfe`／`klhg` 改成上游對應的四個最近 tap：`f`、`g`、`j`、`k`（`ffx_fsr1.h:416-419`）。
- RCAS 補上缺失的 `saturate`。三參數 `max` 已改成巢狀兩參數。
- NIS 係數改成 row-major `array<f32, 512>`，索引用 `phase * 8 + tap`。Rust 表仍是 `[[f32; 8]; 64]`。這是 DX12 不能對 `array<array<...>>` 做動態索引，不是改回 3×3。
- Anime4K pass 7（`conv2d_last_tf`）改綁七張 feature map（binding 0–6）、sampler（7）、storage（8）、uniform（9）。pass 8 維持 dual-source。中間 feature map 加上 `TEXTURE_BINDING`。

驗證：倉庫根目錄 `cargo test -p cookie-sincraft --test integration_test gpu_dispatch_writes_target_size_png -- --nocapture --exact` 結束碼 0（1.48 秒，不是 skip）。四張 PNG 都在 `src-tauri/tests/`，Pillow 讀回皆為 128×96 RGBA：`gpu_dispatch_bilinear.png`、`gpu_dispatch_fsr.png`、`gpu_dispatch_nis.png`、`gpu_dispatch_anime4k.png`。測試另要求輸出不是單色。

這仍不證明像素等同上游。Anime4K depth-to-space 仍只取 R（`MAIN.wgsl`）。GUI、原生對話框、拖放、預覽、存檔未實機走。`cargo build -p cookie-sincraft` 已更新 `target/debug/cookie-sincraft.exe`（2026-11-27 10:43）。`target/release/cookie-sincraft.exe` 仍是 2026-11-23 21:20，雙擊舊發行檔不會帶這次修補。未 commit。

## 後續複審：Windows 真實輸出與修復基準（2026-11-27）

上節是當時的紀錄，並非最新產物狀態。Reviewer 用隔離 release 的 Windows WebView2 路徑實跑 960×540 RGBA → 1920×1080：CPU Lanczos3 及 DX12 Bilinear、FSR、NIS、Anime4K 都從畫面觸發並讀回 PNG，GPU 未顯示 CPU 備援。WebP 原生「另存」的類型為 WebP；`qa-input.webp` 讀回 960×540 RGBA WebP。此輪未驗收物理拖放、跨程序最近檔重開、批次、中途取消；先前逾時的桌面操作沒有重試。

尺寸正確不等於成品正確：該輪輸入 alpha 64–255，GPU Bilinear／NIS 維持該範圍，FSR 僅 64–65，Anime4K 僅 0–24。FSR 把 normalized gather 座標直接轉成整數 texel，重複取左上區域。獨立重跑後續工作樹 `cargo test -p cookie-sincraft` 退出 101，23 單元與 25 對齊案例通過，整合為 36 通過／2 失敗／1 忽略；兩個失敗為 Anime4K alpha 與輸出單色。這是本輪實作修正前的基準。

Codex 首輪設計審查 B1–B4 不通過：浮點中繼超過 64 MiB、取消未傳遞、非整數 CNN 倍率裁切、完整解碼前無法拒絕超限。修訂 [Design.md](Design.md) 的資源、取消、固定 ×2 重取樣與解碼前限制後，Codex 複查三階段通過；不等於實作審查或 release 驗收。

### 本輪已取得的回歸證據

- `cargo test -p cookie-sincraft --test fsr_pixels -- --nocapture`：6 通過、無忽略／skip；含兩項 Windows DX12 實際 FSR，960×540 → 1920×1080 alpha 漸層、右下顏色與常色保持。FSR 修正仍未在新 release GUI 重跑，也沒有宣稱與上游所有像素等價。
- 同一測試二進位含 CPU 四種濾波器與 Native 整圖逐位元對照、非零工作量後取消、進度與資源上限。這證明 CPU 核心，不代表 GUI／批次與 GPU 中途取消已建立完整契約。
- Reviewer 另跑 `cargo test -p cookie-sincraft --bin cookie-sincraft converter::tests -- --nocapture`：13 通過。涵蓋 PNG／JPEG 超限 metadata、RGBA16 解碼緩衝與原子不覆寫。`image` 0.24 的 WebP 建構子會先解碼，尚未接上可在建構階段只讀 metadata 的解碼入口，B4 實作不能關閉。
- 目前尚需完成 Anime4K 非整數實圖驗證、GPU 全部資源上限檢查、取消／發布的原子判定，以及最新 release GUI。未提交也未發布。

### R1 增量實作複查

- 合入 OpenCode 通過複查的原始修補，未改寫候選。`commands.rs` 的讀取、輸出資料夾和確切另存授權分離。先以隔離副本驗證，再於整合後的 Windows 工作樹重跑 `cargo test --locked -p cookie-sincraft --bin cookie-sincraft commands::tests -- --test-threads=1`：18 通過、無忽略，包含真正建立 Windows 硬連結。
- Codex 唯讀實作審查：Spec 確認縮小授權範圍；Code 確認保留同資料夾、已選輸出資料夾及正常匯出鏈；Security 同意作為增量合入。尚不保證 junction／目錄替換競爭或保護所有批次輸入；此輪不是 Windows GUI 驗收。

### Anime4K 非整數倍率複查

- 舊版既有 1.5× 測試只驗 alpha 未超出範圍，純 Bilinear 也能通過。Reviewer 的隔離 DX12 探測在基準版確認兩項失敗：1.5× 未經完整 CNN 中繼，精確 2× 的 `(1,0)` alpha 為 0 而非 20。
- OpenCode 候選以完整 ×2 CNN 中繼，再於 GPU 全圖重取樣；殘差仍以整數座標讀取，原圖 RGB／alpha 使用像素中心插值。Reviewer 實機驗證 96×64 → 144×96、97×63 → 143×113、1280×720 → 1920×1080，三張 PNG 存檔、讀回、驗四角 alpha 與整張 RGBA；符合顯式 ×2 CNN 再 Bilinear，RGB 不等於原圖直接 Bilinear。精確 ×2 可手算的 alpha 表亦通過。無 skip 或 CPU 備援。
- 合入原始候選後 `cargo test --locked -p cookie-sincraft --quiet -- --test-threads=1`：138 通過、0 失敗、1 忽略。這批證據不等同上游像素等效或發行版 GUI 驗收。批次、GPU 中途取消、WebP 解碼前限制等仍未全部關閉。
- Codex `gpt-6.1-sol` 對 `gpu.rs`、`MAIN.wgsl` 及 `integration_test.rs` 的 B3 與相關 B1 差異完成唯讀複查，Spec／Code／Security 均通過；Reviewer 核對三份檔案的 SHA-256 與審查版本一致。實機 DX12 與整合測試沿用 Reviewer 上述證據，Codex 未另跑測試，結論不涵蓋其他 GPU 路徑、上游等效或 GUI。
- Windows 本機 release：`npm run build`、版本 `0.2.0` 同步檢查、`cargo build --locked --release -p cookie-sincraft`、`git diff --check` 均成功。確認 `target/release/cookie-sincraft.exe` 已更新，檔案 22,937,600 bytes，Git blob hash `58ade190768d933a3c8ea2d29bcb21fca314c4a5`。這次只產出執行檔，未打包安裝程式、未發布、未驗收新版 GUI。

### R2 Windows 批次回歸測試候選複查

- OpenCode 返工候選把損毀檔移到兩張有效 PNG 中間，但 Reviewer 將原樣片段放入隔離副本執行 `cargo test --locked -p cookie-sincraft --bin cookie-sincraft --no-run`，結束碼 101：兩處 `ImageBuffer::save` 多傳格式參數、兩處 `u64` 加總指定成 `usize`、兩處未解開 `image::open` 的 `Result` 就呼叫 `to_rgba8`。
- 另經靜態核對，候選讀取 `tag` 而不是 `BatchProgress` 實際序列化的 `event`，且要求 256×256 卻斷言輸出 1×1。未合入，未執行批次案例；不能據此認定批次功能通過或失敗。
- 下一版改為解碼實際 `BatchProgress`、核對八個事件及兩個 5×3 PNG，前述六個編譯錯誤不再出現。隔離副本以 `cargo test --locked -p cookie-sincraft --bin cookie-sincraft batch_valid_corrupt_valid -- --nocapture --test-threads=1` 複驗仍結束 101：`E0382`，候選移除共享的 `Arc` 後把 `recv` 移進 Channel closure，又在 closure 外借用。仍未合入、未執行；需要保留共享事件收集器並修正測試，不能把工作程序結束碼 0 當成測試通過。
- 另一個 OpenCode 返工程序遇到供應端 HTTP 429，最後因輸出截斷以結束碼 1 結束。核對其 `batch_test.rs` 與返工前 Git blob hash 同為 `ed64e2f31a258ce757b9b3d2061393ecb526b3f5`，沒有新候選可驗證；工作樹 `commands.rs` 未變更。
- OpenCode 經 OpenRouter 修正共享事件收集器後，Reviewer 在隔離 `APPDATA`、`TEMP`、`TMP` 的 Windows 副本以真 Wry 建立 `AppHandle`，直接執行 `commands::tests::batch_valid_corrupt_valid`：1 通過、0 失敗、0 忽略。實際收到八個事件，正常 → 損毀 → 正常得到成功 2、失敗 1；兩個 5×3 PNG 讀回顏色正確，無損毀檔成品，登記已清除。Codex 唯讀複查確認 Spec／Code 此切片無阻塞；指出測試若直接在非隔離環境呼叫 `AppState::new()` 會碰共用日誌與暫存清理，須修正後再合入。這不是 WebView IPC 或 GUI 驗收。
- OpenCode 將真 Wry 測試移至主程式專用的 `batch_test.rs`，初始化不寫共用日誌、不掃共用暫存。先前放在共用 `commands.rs` 時，入口 39 項通過但 `aligned_allocator_tests` 啟動即報 `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`；撤除該測試後基準回復 138／0／1，測試移位後隔離合併再跑全套通過。
- 批次取消 Reviewer 探測：在第二項的 `Progress` 回呼提出取消。舊版實際先報完成數 1、2，再送錯誤 `ItemFailed` 與 `Completed`。OpenCode 候選實測改為完成數 0、1、唯一 `Cancelled`，已發佈的 5×3 PNG 仍完整、第二項無成品、登記清除；Codex 限定 Spec／Code／Security 複查未見阻塞。這未驗證已提交 GPU 工作的取消或輸出提交競爭。
- WebP 隔離測試：4097×1 只有 VP8L 標頭、沒有像素資料，基準版先錯在位元流，候選先報尺寸超限；3×2 無損 RGBA 逐 byte 相同。候選的 converter 套件 15／0／0，與批次候選一起的隔離全套 145／0／1；這不代表 `image-webp` 內部工作記憶體都有嚴格 64 MiB 上限。
- 批次與 WebP 差異確認和隔離測試版本一致後合入。整合樹 Windows `cargo test --locked -p cookie-sincraft --quiet -- --test-threads=1`：145 通過、0 失敗、1 忽略。前端建置、版本同步和 `git diff --check` 通過；新版 release 使用單獨建置流程，GUI 尚未驗收。
- DX12 FSR 的 Reviewer 探測（960×540 → 1920×1080）實際出圖，但 `upscale_with_control` 未回報任何中間工作，測試因此失敗。這是 GPU 真實進度的缺口，不是 shader 無法 dispatch；已交 OpenCode 處理 EASU／RCAS 間的完成與取消檢查。
- 上述整合後本機 `cargo build --locked --release -p cookie-sincraft` 成功，`target/release/cookie-sincraft.exe` Git blob hash 為 `0df799cd9a6b2a94ff1ec355000dbba69e0090c6`。這個產物不包含仍在開發的 GPU 進度修補，尚未完成 GUI 驗收，未發布。同一 EXE 本機產出未簽署 MSI（7,184,384 bytes）和 NSIS（4,875,929 bytes），打包後 EXE hash 未變；未執行安裝。
- 隔離 QA release 編入獨立的 Tauri identifier 和視窗標題；Windows WebView2 實際啟動，原生多選 PNG 對話框可開啟。送出檔案選擇的 GUI 操作未獲核准，未重試或改用其他途徑。未以 Wry command 測試冒稱 GUI／跨程序驗收。
- 批次保持比例：同一個 Wry 探測由 16×8 錯誤拉伸改為保比例的 8×20，另一張來源及其真正的 Windows 硬連結位元組未變，工作登記清除；原有正常／損毀／正常測試也通過。OpenCode 產出、Codex Spec／Code／Security 限定通過，差異已合入；8K 批次選項與提交競賽未由此驗證。
- FSR 獨立 GPU 探測確認候選完成 EASU 後才通知（DX12、無備援、`fsr_pixels` 7 通過）；舊命令層實測仍錯把取消當成 GPU 失敗並發送 fallback，已將診斷送 OpenCode。唯讀複查另指出 GPU 成功計數、取消優先次序與 FSR 浮點中繼預檢，本段 7 項結果不代表已合入或完成全部 B1／B2。
- WebP 動畫全畫布首幀與舊路徑逐 byte 相同，但正常的 8×6、首幀 4×2 置於 (2,2) 的動畫重現背景從 `[15,25,45,255]` 變成黑色。初版背景修補仍有 BGRA/RGBA 反序，不能採用；已接續驗證轉序修補。
- 轉序修補實測 2／0／0，兩種 VP8X 首幀均與舊解碼器完整 RGBA 逐 byte 相同。Codex 限定 Spec／Code／Security 通過，與批次比例修補一併合入。不宣稱 `image-webp` 的內部分配具全工作集硬上限。
- 最終 OpenCode FSR／指令差異通過 Codex 限定 Spec／Code／Security 複查後合入。隔離 Windows 合併全套 **153 通過、0 失敗、1 忽略**，含 8 個 Reviewer 探測（未作為正式測試檔合入）：真 DX12 完成 EASU、尚未送 RCAS 時可取消，Wry 指令不發 fallback、沒有成品且清除登記；成功路徑在 64×48 依序回報 `(12,24)`、`(24,24)` 個完成工作群組；4096×2049 的浮點中繼超 64 MiB 在分配前拒絕，正確備援 CPU Lanczos3、讀回 PNG 尺寸和像素、收到 Completed 時成品已存在。未測已送入 GPU 指令的強制中斷、其他 GPU 演算法的細進度或各種發布競賽。
- 上述 Rust 差異合入後，正式工作樹 145／0／1，前端版本檢查與鎖版 release 通過。本機 EXE SHA-256 `136c681c2646092d92de543c3dae09d881a83144d7c3192408246fea74165a38`；未簽署的 MSI 和 NSIS 安裝檔分別 7,192,576 與 4,859,281 bytes。未安裝或發布，未完成原生 GUI 驗收。
- 前端批次單獨的取消入口在此 release 仍缺漏（Node Reviewer 探測在實際 `index.html` 和 `app.js` 片段上 RED：「batch has no cancel button」）。後端可取消不代表使用者能從批次畫面取消。
- 後續合入 OpenCode 的兩份前端取消差異；不更改單張取消、檔案操作或 ACL。Reviewer 對隔離候選及整合工作樹皆實跑 Node VM：開始事件提供批次 ID、取消命令參數、三個終態、請求在途防重複、錯誤可重試、舊請求晚到不影響下一批。Codex 唯讀 Spec／Code／Security 限定通過；Vite 產物 HTML、JS 已確認包含按鈕。此證據不等於原生 WebView 操作。
- 獨立 Windows Wry 探測：既有 PNG 目的檔遇碰撞時位元組不變，批次仍完成另一張正確的 5×4 PNG；在第一項 `ItemCompleted` 回呼取消時，第一張已可讀且保留、第二張不產出、只收到批次 `Cancelled`、登記與暫存清除。測試沒有在取消檢查與檔案系統提交之間操控競爭，不把此項當成提交原子性的驗收。
- 含批次取消按鈕的 Windows 0.2.0 已重建：工作樹鎖版測試 145／0／1；EXE SHA-256 `f18b6a5a80e1189563b1e61a246542232a71f730e62c02d810dae6c6966802f7`，未簽署 MSI 7,192,576 bytes、NSIS 4,862,580 bytes，皆於 `target/release/`。這版尚未含批次撞名修補；未安裝、未發布、未驗收原生 GUI。
- 批次命名：原版缺 `{name}` 的樣板及兩種觸發備用檔名的案例均撞名；首個候選先加序號，反而改掉無 `{name}` 時的備用檔名判定，未採用。最終候選先清理，再依適用分支保留序號，四種批次案例通過、單張命名不變，且獨立 Spec／Code／Security 複查通過。已原樣合入；這並非磁碟既有檔案的自動改名功能。
- 含取消按鈕及批次命名兩項修正的 Windows 0.2.0 已重新建置，取代上一版純取消產物。前端三份 Reviewer 探測、Vite、三處版號、`git diff --check` 與鎖版回歸（145／0／1）通過；本機 EXE SHA-256 `b166f1fd0baa8a5eb7d8556e20f7368285b713da72bda3e142cfc81e7077d234`，未簽署 MSI／NSIS 同步重建。未安裝或發布，原生 GUI 與取消／發布臨界競爭未驗收。
- 取消／發布臨界競爭已重現：在隔離 Windows `converter.rs` 中僅以測試探針安排「讀取取消旗標後、檔案系統提交前」設定同一旗標；執行 `reviewer_cancel_before_filesystem_publication` 實際退出 101。no-replace 路徑新增了本不該發布的 PNG，已核准覆寫路徑替換了舊 PNG，兩路均回 `Ok(())`。此為排程受控的 converter 探測，不是自然使用頻率或原生 GUI 證據；產品碼及已打包版未修這個臨界問題。詳 `cs-batch-real-qa-gp1oh_7t/publish-window-red.log`。
- B2 另一個 Windows Wry RED：使用既存目的檔 `cancelled-target.png` 且不允許覆寫，舊檔 bytes 不變、API 回傳「output file already exists」，但終態誤發 `Cancelled`。實測退出 101（`publish-error-kind-red.log`）。提交鎖與型別化終態設計通過 Codex 三軸複查，未代替實作與複驗。
- 非 FSR 的 DX12 回歸 `reviewer_non_fsr_dx12_progress_and_cancellation` 實機讀回 Bilinear、NIS、Anime4K 各一張非單色 192×128 PNG，三者都沒發真實工作進度，也無法在進度回呼觸發 typed `Cancelled`，因此測試退出 101（`non-fsr-control-red.log`）。這是進度和取消阻塞，不是無法 dispatch 或上游等效結論。
- Bilinear／NIS：OpenCode 差異已併入工作樹，未重打包；與 Codex 裁定的候選僅差一處未採用的 FSR 區域變數型別註記。隔離副本 `gpu.rs` 的 Git blob 和工作樹一致，Reviewer 在 Windows 重跑 `fsr_pixels`：7／0／0（含一份 QA 探測）。兩算法走真 DX12、各讀回一張 192×128 PNG，等待 dispatch 後各回報 `(96,96)`；回呼提出取消，結果為 typed `Cancelled`，不代表能中止已提交的 kernel。Codex 只審本次差異，沒有重跑 GPU。Anime4K 未列入這次通過結果。
- WebP B4 靜態補查：`image-webp 0.2.4` 的 `set_memory_limit` 接在 ICCP／EXIF／XMP 的 chunk 讀取（`decoder.rs:644-654`），不會限制 VP8 壓縮資料的內部暫存（`vp8.rs:1047-1052`）。因此尺寸檢查與 `try_reserve_exact` 只能證明應用程式輸出緩衝受限，不能證明 codec 每個配置皆不超 64 MiB，更不能證明整個行程的峰值。此項仍待修驗；未新增整個行程必須低於 64 MiB 的需求。
- B2 的 converter 候選已在隔離 Windows 排程探測通過（`cs-gate-qa-GEkT03/gate-converter-only.log`）：no-replace 和已核准覆寫各驗取消先到、發布先到及同一把鎖阻止後續項目；讀回新 PNG、比較舊檔，無 `.part` 遺留。這是 converter 候選本身，不是 Wry 路徑；commands 草案快照在鎖版編譯退出 101（`cs-gate-qa-k3lYi3/gate-compile.log`），尚未合入。終態仍須用 Windows Wry 驗證，不以此 converter 探測外推。
- Anime4K 進度修補前的像素基準已在真 DX12 保存四張 RGBA PNG：96×64 → 108×72（Bilinear 條件路徑）、144×96（CNN 後重取樣）、192×128（精確 ×2）、384×256（兩輪 CNN）。四路皆無進度事件。新候選須同時對照完整 RGBA、固定分母的真工作群組計數及回呼取消；此基準不是上游像素等效或候選通過證據。
- 隔離 Windows 後端設定儲存探測：第一個測試行程寫入預設值再原子替換，第二個行程以實際 `read_settings_file` 讀回，設定 JSON 完整相同，無 `.tmp` 遺留（`cs-settings-storage-DvjU7r/prefs.log`）。未讀寫正式設定、未碰最近檔、未呼叫 GUI／WebView IPC；不代表發行版重啟驗收。
- 最新工作樹序列執行 locked 回歸為 **145／0／1**（41＋42＋6＋56，`cs-dx12-serial-uyRrcV/regression.log`）。先前在共用 Cargo target 同時執行隔離探測，`fsr_pixels` 錯跑 22 項而非工作樹的 6 項，該次總數作廢。B2 converter 單模組通過 Codex 三軸複查，最新 commands 候選仍有 10 個編譯錯誤；Anime4K 候選有未閉合 delimiter，兩份未合入。原 EXE SHA-256 仍為 `b166f1fd0baa8a5eb7d8556e20f7368285b713da72bda3e142cfc81e7077d234`。
- 上述 commands 的 OpenRouter 返工受免費配額及付費金鑰上限阻擋；另以 NVIDIA smoke 通過後才依序備援。NVIDIA 的實際 `commands.rs` 快照配對已驗 converter 再跑 `--no-run`，仍因 `registry` 作用域、把 `Arc<PublicationGate>` 當成 `Option`、終態事件 `String` 型別等問題退出 101（7 個編譯錯誤，`cs-gate-qa-Tymxfu/gate-nv-compile.log`）。退回候選、不合入、不重打包，未執行 Wry 終態探測；不以 OpenCode 自述取代編譯結果。
- B4 WebP 獨立設計首輪（`cs-batch-real-qa-gp1oh_7t/webp-bound-design-verdict.md`）：Spec 通過，Code／Security 需修正同一項阻塞。須明定預算在 `WebPDecoder` 建構前生效，列出每個受控配置點的元素型別、checked bytes 計算、可失敗的容量成長及錯誤傳遞；說明 64 MiB 約束的是請求配置，不是配置器額外空間或全行程峰值。未開始 codec 實作，未執行配置限制測試。
- Anime4K NVIDIA 候選（blob `eadf1126446f0689880836158ceedcb28d30567f`）原樣置入隔離副本後，鎖版 `fsr_pixels` 探測在 `gpu.rs:2193` 因未閉合 delimiter 編譯失敗、退出 101（`cs-anime-nv-qa-VJnVz7/anime-nv.log`），未執行 DX12。差異另刪除檔尾 NIS 係數表及存取函式、修改 NIS 繫結，超出 Anime4K 範圍。Anime4K 仍用每階段變動的總數，且最後卷積及 depth-to-space 未加入實際完成等待；即使補括號也不能採用。候選不合入，工作樹 `gpu.rs` 仍與上次通過的 Bilinear／NIS 版本一致。
- 三個 OpenCode 局部返工遇 NVIDIA HTTP 429，初次只留下 `commands.rs` 兩段變更。再以逐一提交重做發布閘門指令接線，九段候選完整保存，但鎖版編譯 6 錯：registry 仍建 `JobEntry`、把 `PublicationGate` 當 `Arc`、讀私有旗標、移出 Arc 後再呼叫、IPC 巢狀 Result 未解開（`cs-gate-qa-m48Zwk/gate-plumbing-compile.log`）。未合入、未執行 Wry 臨界終態。這次只拆驗接線，沒有把 typed outcomes 算在候選內。
- Windows Wry 後端近期檔案儲存跨行程（`cs-recent-backend-Jg4hFp/backend-recent.log`）：兩個 PID 分別寫入／重開讀取，各 1 項通過。只有已獲授權的測試路徑進 canonical `recent.json`；拒絕未授權路徑、`write_settings` 注入的近期清單、legacy 偏好中的同名假路徑及 canonical 中的 app-temp；讀回品質 73／保持比例。QA 副本只替換兩處設定目錄解析為 scratch 子目錄，沒有啟動 GUI、呼叫開圖、使用正式 profile 或驗證 Windows 正式路徑解析。
- WebP `cs-webp-budget-review-AulOTh/verdict.md` 設計複查：建構前傳預算、checked／fallible 規則及錯誤傳遞已過；Code／Security 要求明列完整 VP8 macroblocks 數、移除分區轉盒及兩個 `u16` 編碼長度配置。已補到 Design 清冊，尚未證明產品 codec 有內部硬限。
- 再複查 `cs-webp-inventory-final-2LynNs/verdict.md`：Spec／Code／Security 皆在指定三項清冊範圍通過；讀者可據 Design 實作，但不能用設計判詞代替 codec 的實際配置覆蓋、相容像素測試或發行驗證。
- Anime4K 局部候選 `78915b7fec6c438f8f954db5220421a74e37e2d4` 將變更限制在既定七段；隔離鎖版編譯的 `fsr_pixels` 和主二進位各 9 個錯誤（`cs-anime-frag-qa-VTLEF9/anime-fragments.log`），未執行 DX12。候選接錯 method scope、漏傳 `report_progress` 第三個參數。Reviewer 的交辦也把實際 `Cancelled(String)` 誤寫為 unit variant，相關錯誤不能只歸給 OpenCode。候選不合入；下一份交辦應附 `upscaler.rs` 真實型別及完整函式邊界。
- 本機 `cargo fmt --all -- --check` 退出 1（`cs-fmt-gate-audit-hDCdbj/fmt.log`），在 12 個 Rust 檔案有格式差異；`.github/workflows/ci.yml:10–15` 的 `cargo fmt --check` 是獨立 gate。`git diff --check` 與版號同步通過不等於 CI 格式 gate 通過。此輪只檢查，未重排有未提交修改的 Rust 檔案。
- WebP 開始隔離的配置 helper 實作：初稿 OpenCode 超時，rustc 實際拒絕兩處 `quick_error` 語法（`cs-webp-budget-rustc-EM4ZoQ/compile.log`）。固定原始碼的 Cargo offline 測試缺上游 dev dependency `webp`；改以本機同版 `quick-error`／`byteorder-lite`（預設 `std`）編譯實際來源，不下載。窄稿僅新增 private module 及宣告，codec lib 已可編譯；五個 helper 測試通過，一個測試把 12-byte 上限內已有三個 `u32` 再請求兩個誤當成功，且誤認 reserve 改變長度（`cs-webp-budget-small-4XaugK/budget-tests.log`）。未接入 VP8／VP8L／VP8X 或應用程式，不算 codec 配置驗收。
- WebP helper 的兩處測試返工後，生產 helper preimage 不變且小規模預算、等於上限、超出上限、乘法溢位與既有 Vec 不被改寫等 6 項全過（`cs-webp-budget-tests-tcHYfZ/budget-tests.log`）；仍未接入實際解碼器的配置點。
- WebP 下一個隔離切片 `cs-webp-ctor-mODPSk` 把預算存進 decoder 再讀資料，僅約束 `read_chunk`；正常 VP8X 動畫的 `ANIM` 建構期 6-byte 成功、5-byte 拒絕，正常帶 XMP 的 WebP metadata 與 RGBA 讀回，共 2 項通過。差異審查看到 `as usize`、將配置器失敗誤對應成超額和文件誤稱全 decode 記憶體受限，已退回該切片補正；未合入且其他解碼工作配置尚無硬限。
- 另建確切工作樹副本 `cs-ci-recheck-vyMgwO`：`cargo clippy --locked --all-targets` 退出 0（有 warnings），鎖版序列 `cargo test` 依 target 計數 41／42／6／56 通過、整合 1 項忽略。前端三探測重跑通過，與發行版 QA 及新增候選分開計。EXE、MSI、NSIS 雜湊未變。
- `cs-webp-ctor-reviewed-xpYnyA` 的 WebP 建構切片返工只改 `decoder.rs`，拆出 typed 錯誤但改寫成 `u64::from(max_size)`，在 `decoder.rs:673` 因沒有 `From<usize>` 而編譯失敗（`compile.log`，E0277）。沒有重跑該候選的建構探測；兩個綠測只適用於前一個候選，不能把修訂版算成通過。兩版皆未整合。
- 後續 `cs-webp-ctor-checked-TO6Ru6` 只修正比較前的寬化轉型，已核對單行差異；原 ANIM 建構與 XMP／RGBA 探測皆在此版重跑通過（2／0／0），溢位、超出預算及配置失敗分型保持。這個切片仍只限制 `read_chunk`，未上產品，不涵蓋其餘 VP8／VP8L／VP8X 工作配置。
- B2 `cs-gate-qa-1FAywH`：OpenCode 只返修 gating 草案的兩段，配已核驗 converter 首次通過鎖版二進位測試編譯；兩發布模式的 converter 排程探針全過。Wry 既有兩測通過，另 `cancelled-target.png` 的真 I/O 錯誤仍會誤發 `Cancelled`（窗口內已有舊檔且 bytes 不變）。因此接線切片未合入，另交辦 typed 終態。
- B2 第二切片（commands blob `75153d8bbe8641d0a1fa9c364df4a155d285d7da`）：OpenCode 逾時但三段可組裝。Windows 鎖版入口 44／0／0（`cs-gate-qa-YnITyt/typed-main.log`），`cancelled-target.png` 回唯一 `Failed`；新 Wry 專用探針亦過，兩種單張模式在取鎖前取消、不改目標，批次在首張成功寫入後取消，僅保留該成品、唯一 `Cancelled`、清登記及 `.part`。無探針的確切候選全套 145／0／1（`cs-b2-uninstrumented-glMlqE/b2-product-regression.log`）；未以含 `crate::batch_test` 插樁副本的跨 crate 編譯失敗評判產品。Codex `cs-b2-impl-review-jvBAec/verdict.md` 判 Spec／Code 需修正、Security 限定通過：converter 仍用字串前綴判定，隔離綠測不足以確認設計；已退回聚焦修改，未合入。
- B2 最終 OpenCode 差異已原樣合入：工作樹 `commands.rs` blob `40e1d55bfbb8b246daf4143fd36b790ff6c20d07`，`converter.rs` blob `2f18a627ba436bf7e2d00ced306e12e2e0b99817`。Codex `cs-b2-type-final-review-CWEris/verdict.md`：Spec／Code 通過、Security 限定通過，取消依真正的錯誤 variant 分類；沒有 ACL／授權擴大。相同候選的 Windows 隔離入口 45／0／0（`cs-gate-qa-uPvBDH/b2-typed-verified.log`）；原工作樹重跑 locked 回歸為 145／0／1（41＋42＋6＋56，`cs-integrated-b2-EOXeBp/integrated.log`）。這是產品程式整合與後端證據，非 release GUI 驗收。
- 最新 Anime4K 完整方法候選仍未合入。`cs-anime-frag-qa-l7ZuPk/anime-methods.log`：Windows 鎖版的主程式和 `fsr_pixels` 各 6 個編譯錯誤，包含未解析控制型別、caller 漏傳控制、`?` 放在 `Some` 外及不存在的 `Queue::submission_index`；另外靜態檢查發現計數計畫把 2× 中繼尺寸以目標尺寸截斷，與原路線不符。未執行本候選的真 DX12 或像素探針。
- B4 WebP 畫布／影格中繼切片：對前版正常 3×2 RGB 與 8×6 動畫圖，目標為分別限制 24-byte RGBA 中繼、192-byte 畫布；少 1 byte 仍成功是 RED（`cs-webp-frames-red-Kn1k9s/red.log`）。OpenCode 第一稿 E0599；僅修正 free function 呼叫後 `cs-webp-canvas-checked-orJhKM` 編譯通過，24／192 恰好可用、23／191 在各階段回 typed 超額，完整 RGB（含偏移、背景）與預期一致，原建構兩項也重跑通過。只證明特定配置切片，不是完整 codec；未接產品。
- B4 WebP VP8X alpha 切片：正常 16×8 VP8+ALPH 的原始、壓縮兩版本，舊候選在 127／511 bytes 均仍可解碼（`cs-webp-alpha-qa-gjyYUH/pixels.log`）。OpenCode 第一稿漏兩個 import，回修後 `cs-webp-alpha-checked-Jj4vYQ` 編譯、5 項通過：兩種 alpha 路徑的標準像素與前版逐 byte 相等，127／511 各以 128／512-byte 請求回 typed 超額；畫布／中繼及 metadata／建構共 4 項回歸重過。這只約束 alpha 外層及前述切片，未約束 VP8／VP8L 內部配置、未接產品。
- Codex `cs-webp-outer-checked-review-j1QI25/verdict.md` 對畫布、影格和 alpha 外層的增量三軸限定通過，不外推到完整 codec。VP8 arithmetic storage 獨立切片 `cs-webp-vp8-storage-l0KRGW` 驗證只有三處 Box 轉 Vec 替換、不額外縮容、同樣 5 項 codec 回歸過；分區的配置預算尚未滿足。新 VP8 分區實測的 RED（`cs-webp-vp8-qa-Fw8IdE/red.log`）：合法 16×8 VP8 仍可在 31 bytes 解碼，應拒絕需 32 bytes 的對齊 token buffer。第一份分區候選 `cs-webp-vp8-partitions-8Kw4qG` 有不完整 constructor 邊界及預檢前配置，拒用，不修補成「通過」。
- VP8 分區接線及配置拆小後，`cs-webp-vp8-partition-checked-99n2k4` 鎖定同版依賴編譯通過。正常 16×8 VP8 以 32-byte 預算讀回全 RGB 與基準一致；31／29／15 bytes 分別拒絕 32-byte 對齊 token words、30-byte token bytes、16-byte 主分區 words。共 6 個隔離案例過（分區 1、alpha 1、畫布 2、建構 2）。VP8 平面、macroblock／VP8L 工作區與產品接線仍不在此結果內。
- Windows Wry 另重現單張載入損毀格式：IPC 回真正解碼錯誤，輸出不存在，但 Channel 只有 `Started`、沒有 `Failed`（`cs-decode-terminal-gKYqun/red.log`，測試 0／1）。發布排序的合入證據不受影響；此缺口已交 OpenCode 聚焦修回單一型別化終態。
- 單張解碼失敗修正已原樣整合（`commands.rs` blob `1b38b0a244fd5462edbb4d0e5fcd1e3575c2511c`）。OpenCode 內建免費 `opencode/beam-free` 候選在隔離 Wry 入口 42／0／0；實際發 `Started → Failed`、兩事件同 ID、IPC 錯誤相同、無成品且工作已清除（`cs-single-decode-retry-RkAsjB/green.log`）。Codex `cs-single-terminal-review-i7Ijp5/verdict.md` 三軸在此差異內通過；實際工作樹再次 locked 145／0／1（`cs-single-decode-integrated-5XArNm/integrated.log`）。
- Windows 本機中間 release：`cs-release-b2-checkpoint-aaDeqo` 成功執行 Vite、版號與 `tauri build --ci --no-bundle --no-sign -- --locked`，較早的 B2 EXE SHA-256 為 `3c0a62dedf168f3cd7d3416eddb83102942ead7d850a5146a64aa5b1cc256da5`。最後單張解碼失敗修正後，`cs-release-terminal-checkpoint-41GbpK` 又重建成功；目前 `target/release/cookie-sincraft.exe` SHA-256 `0b982322fb8347a91472e536a08f17d1cf782fddb5f08d1260bf9f36447b9eab`。未打包或啟動 GUI；原 `bundle/` 的 MSI／NSIS 時間早於兩版新 EXE，不當作當前版本。仍不含 scratch 中的 Anime4K／WebP 候選，不是正式交付。
- fmt 在工作樹仍為 12 檔格式差異；`cs-fmt-isolated-8UF4Md` 格式驗證副本通過 `cargo fmt --all -- --check` 及 145／0／1 回歸，但這是前一個 B2 工作樹快照，沒有格式化工作樹，也不替代目前 CI gate。
- WebP VP8 平面 RED `cs-webp-planes-qa-j25221/red.log`：合法 16×8 輸入在 255-byte 預算仍可解碼，應拒絕需要 256 bytes 的 luma 平面。第一個平面／macroblock 草稿逾時且無編輯，縮為三個平面後 `cs-webp-luma-AXdnHy/compile.log` 仍因將 `AllocationSizeOverflow(&'static str)` 當 unit variant 出現 E0277。此稿未通過、不合入；不抹去前一個 VP8 分區的隔離綠測。
- WebP VP8L 預算已傳到靜態、動畫與壓縮 alpha 三個解碼入口（`cs-webp-vp8l-wiring-stL82R`）；再只限制 predictor、color-transform 與 color-table 三個 Vec。正常 32×16 四色 RGBA VP8L 在前版 15 bytes 仍成功（`cs-webp-palette-qa-DpztHs/red.log`），新候選 `cs-webp-vp8l-transform-ZRdsxl` 以 15 bytes 拒絕精確 16-byte 色表，16 bytes 的完整 2048 bytes RGBA 與來源一致；共 7 項隔離探針通過。Huffman、cache、後處理、VP8 平面／macroblock 及產品接線未驗。
- `cs-webp-huffman-typed-hp0Us9` 接上 VP8L Huffman table 和節點的預算，前版正常圖不回上述錯誤（`cs-webp-huffman-qa-kBo2xj/red.log`），修正後 16 bytes 拒絕 64-byte Huffman table，正常預算完整 RGBA 與原圖一致。先前 RGB 中繼探針的多色圖在 24 bytes 先碰到 32-byte Huffman table（`cs-webp-huffman-matrix-xnDM4H`，矩陣該項失敗）；以相同 3×2 RGB 單色圖隔離該 24／23 門檻和原動畫 192／191 門檻，兩者在同一候選重跑過（`cs-webp-huffman-frame-qa-RSC4Yy/frame.log`）。未驗其他 Huffman 相關向量、VP8L 配置或產品整合。
