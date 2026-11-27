# 程式碼清理與 CI 建置

> 本文件為規劃文件，不完全是實作依據。

---

## Description

清理 Task #1 未涵蓋的程式碼品質問題，並建立自動化 CI：

1. **`eprintln!` 除錯輸出殘留**：`commands.rs:322,332` 正式版輸出到主控台
2. **前端 `console.log` 殘留**：`app.js:315,319,349,353` 匯出/轉換流程有除錯輸出
3. **`greet` 指令殘留**：`main.rs` invoke_handler 已無實作，需同步更新 `Agents/service-architecture.md`
4. **測試用全域配置器污染**：`integration_test.rs:34-35` `#[global_allocator]` 影響其他測試
5. **無 CI/CD**：發布為手動流程，無自動化測試/建置驗證
6. **版本號三處手動同步**：`Cargo.toml`、`tauri.conf.json`、`package.json` 易不一致

---

## To Reproduce

1. 執行 `cargo build --release` → 主控台出現 `eprintln!` 訊息
2. 開啟開發者工具 → Console 有匯出/轉換的 `console.log`
3. `cargo test` → 全域配置器可能干擾測試隔離
4. 發布需手動：`npm run build` → `tauri build` → 上傳 GitHub Releases
5. 三檔案版本號需人工同步，易出錯

---

## Expected behavior

1. 移除所有 `eprintln!`/`console.log`，改用 `tracing`/`log` crate 分級記錄（僅 debug 模式輸出）
2. `Agents/service-architecture.md` 移除 `greet` 指令相關記載
3. 測試配置器限制在測試函數內部，或使用單獨測試二進制檔
4. GitHub Actions CI：`cargo test`、`cargo clippy`、`cargo fmt`、`tauri build` (Windows runner)
5. 版本號同步腳本（如 `cargo-workspace-version` 或自訂 script），納入 CI 檢查三處一致性

---

## Acceptance Criteria

- [ ] `cargo build --release` 執行靜默（無 `eprintln!` 輸出）
- [ ] 前端開發者工具 Console 無 `console.log` 雜訊
- [ ] `Agents/service-architecture.md` 無 `greet` 指令相關內容
- [ ] `cargo test -p cookie-sincraft` 通過且測試間無配置器干擾
- [ ] GitHub Actions Workflow 存在 `.github/workflows/ci.yml`，包含：test、clippy、fmt、tauri build
- [ ] 版本號同步腳本可執行，CI 檢查三處版本一致
- [ ] 現有功能不倒退

---

## Additional context

- Task #1 已移除 `tauri-plugin-dialog`、`tauri-plugin-shell`、關閉 `devtools` feature
- 建議引入 `tracing` + `tracing-subscriber`（Rust），`log` + 適配器（前端）或簡單移除
- 版本同步可參考 `cargo-workspace-version` 或撰寫簡單 Node.js/Python 腳本

---

## 已確認

- 無待確認事項

---

## 非本期範圍

- 重構模組結構、加入整合測試覆蓋率、效能基準測試