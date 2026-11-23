# CookieSincraft 專案 Agent 指示

CookieSincraft（曲奇芯刻）是 Tauri 2 + Rust 的 Windows 桌面圖片放大與格式轉換工具。本檔只放跨任務都適用的通用規則；專案知識在 [`Agents/`](Agents/)，依下方「什麼情況要讀」選讀。

## 專案知識（依任務選讀）

| 文件 | 什麼情況要讀 |
| --- | --- |
| [服務架構](Agents/service-architecture.md) | 判斷功能該改前端（`src/`）還是後端（`src-tauri/`）；找某功能對應的畫面元素、IPC 指令與程式位置；新增或修改 IPC 指令 |
| [系統架構](Agents/system-architecture.md) | 改建置、打包、版本號、相依套件、視窗／系統匣設定、GPU（wgpu）或 CPU 放大子系統 |
| [資料庫結構](Agents/database-schema.md) | 改檔案讀寫、輸出檔名、暫存或任何持久化資料；考慮加入設定檔或資料庫 |
| [業務邏輯](Agents/business-logic.md) | 改放大流程、演算法、GPU→CPU 備援、輸出格式、品質參數、預覽或匯出／轉檔規則 |
| [使用者權限](Agents/user-permissions.md) | 新增 IPC 指令、改檔案路徑處理、CSP、Tauri capability、外掛權限或開發者工具設定 |

## 工作方式

- 依使用者撰寫的 `planning.md` 開發、撰寫 `Design.md`，或審查設計／實作時，開工前讀取並套用 `dev-workflow` 技能（Claude：`C:/Users/jake4/.claude/skills/dev-workflow/SKILL.md`；Codex：`C:/Users/jake4/.codex/skills/dev-workflow/SKILL.md`；其他 Agent 讀 Claude 路徑）。實際讀取後才宣稱已套用。
- 任務文件放 `docs/[#<編號>]<任務名稱>/`：使用者的 `planning.md` 與 Agent 產出的 `Design.md`。`Design.md` 每個任務各一份，合併設計與實作紀錄，不另寫 `Implementation.md`；整個專案不共用一份 Design。
- `Design.md` 盡可能以 Mermaid 圖表呈現：流程圖、時序圖或架構圖，依任務挑選最能說明變更的圖，不必每種都畫；文字只補充圖表無法表達的規則與契約。
- 審查分兩個時點：`Design.md` 產出後（實作前）、實作與驗證完成後。每個時點都依 Spec → Code → Security 三階段進行，各階段結論分開記錄在該任務的 `Design.md`。設計審查三階段皆通過才開始實作。
- `planning.md` 由使用者撰寫；Agent 未經明確指示不改寫、改名或刪除。需求由對話提供時引用該次指示，不自建 Planning。
- 任務改變架構、IPC 指令、檔案輸出規則、業務規則或權限時，交付前回寫 `Agents/` 對應主題並更新其文件基本資訊。`Agents/` 與程式不一致時，以程式為準修正文件，並在任務 Design 記錄差異。
- 本專案不使用 `CLAUDE.md` 與 `maintenance.md`：Agent 規則寫在本檔，專案知識寫在 `Agents/`。
- 文件使用台灣繁體中文；圖表用 Mermaid。

## Planning.md 撰寫指引（當開發者要求時）

**用途**：將開發者提供的需求整理成 `planning.md`，不自行新增需求，也不直接設計或實作。

**固定架構**（標題保留英文、內容使用台灣繁體中文）：

| 章節 | 內容 |
| --- | --- |
| `# [Topic]` | 一句話主題，沿用開發者用詞 |
| Description | 要解決什麼、需求範圍 |
| User Story | 角色、功能與目的；資訊不足就寫「無補充」 |
| To Reproduce | bug 的重現步驟與環境；非 bug 標示不適用 |
| Expected behavior | bug 修正後的預期行為；非 bug 指向需求與驗收章節 |
| Acceptance Criteria | 用 `- [ ]` 列出可判定通過與否的驗收條件 |
| Additional context | 補充資訊、待確認、非本期範圍 |

**主要限制**：
- 開頭只加一句註解：「本文件為規劃文件，不完全是實作依據。」
- 不增刪範本章節，不加版本表等額外區塊。
- **只整理開發者提供的需求、限制與驗收要求**；不自行補功能、邊界案例或驗收條件。
- 可以讀程式與文件了解現況，但不能把讀到的內容變成新需求。
- 影響範圍或驗收的模糊事項列為「待確認」，不自行定案。
- 沿用專案存放慣例；位置或 issue 編號不明時先詢問。
- 不擅自覆寫既有 Planning；開發者確認後，未經指示不得修改。
- 後續設計與實作交由 `dev-workflow` 技能處理。

## 程式庫與版本控制

- 單一 Git 倉庫，主要分支 `main`（追蹤 `origin/main`）。提交訊息沿用 Conventional Commits（如 `feat: …`、`fix: …`）。未經使用者要求不提交、不推送、不改寫歷史。
- 前端在 `src/`（Vite + 原生 JavaScript，無框架）；後端在 `src-tauri/`（Rust 二進位 crate，沒有 lib target，整合測試以 `#[path]` 引入模組）。
- 不提交建置產物與測試產生的圖檔：`target/`、`src-tauri/gen/`、`node_modules/`、`src-tauri/tests/*.png|jpg|webp`（見 `.gitignore`）。
- 版本號同時存在 `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src/package.json`，發版時三處一起改。

## 驗證

- 後端：在根目錄執行 `cargo test -p cookie-sincraft`。2026-11-22 基準：單元測試 6 項、整合測試 10 項全數通過（有既存 warnings）。
- 前端：在 `src/` 執行 `npm run build`（輸出到 `src-tauri/gen/dist`）。前端沒有自動化測試。
- 打包：先 `npm run build`，再於根目錄執行 `./src/node_modules/.bin/tauri build`；`tauri.conf.json` 沒有設定 `beforeBuildCommand`，不會自動建置前端。
- 整合測試的 GPU 案例在 GPU 失敗時改用 CPU Lanczos3 即判定通過，不能證明 GPU 路徑可用。GPU 實際輸出、原生檔案對話框與視窗操作沒有自動化測試；未實機操作的項目在 Design 標明未驗證及原因，不寫成已完成。
