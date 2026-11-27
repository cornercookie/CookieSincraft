# UX 改善：拖放、覆寫確認、預覽更新、尺寸限制

> 本文件為規劃文件，不完全是實作依據。

---

## Description

修正 Task #1 未涵蓋的使用者體驗缺口：

1. **拖放不讀取檔案**：放下時改開對話框，`<input type="file">` 未使用
2. **放大結果直接覆寫同名檔**：不詢問確認，風險高
3. **預覽不更新放大後結果**：無法即時比對放大效果
4. **`used_fallback` 前端僅狀態列顯示**：無視覺化提示（如圖示、彈窗）
5. **無輸入/輸出尺寸上限檢查**：8K 輸出單緩衝 133 MB，可能導致 OOM 或 DoS

現況細節見 `Agents/service-architecture.md`、`Agents/system-architecture.md`、`Agents/business-logic.md`。

---

## To Reproduce

環境：Windows 11，程式基準 `main`（v0.2.0）。

1. **拖放**：將圖片拖入放置區 → 彈出檔案對話框而非直接載入
2. **覆寫**：放大同一張圖片兩次 → 第二次直接覆寫 `_upscaled` 檔，無確認
3. **預覽**：完成放大 → 預覽仍顯示原圖，無法比對效果
4. **Fallback 提示**：GPU 失敗改用 CPU → 僅狀態列文字變色，無彈窗/圖示
5. **尺寸限制**：載入超大圖或設定 8K 目標 → 記憶體飆升，無防護

---

## Expected behavior

1. 拖放圖片直接載入（或綁定 `<input type="file">` change 事件作為替代）
2. 放大輸出同名檔案時，前端彈窗確認（或自動加流水號 `_upscaled_1` 等）
3. 放大完成後，預覽區更新為放大結果（Completed 事件帶回 base64 preview）
4. Fallback 時：狀態列圖示變色、或完成時彈出 toast 通知
5. 加入輸入/輸出尺寸上限檢查：**max 4096×4096、單緩衝 64 MB**，超限直接回錯

---

## Acceptance Criteria

- [ ] 拖放圖片到放置區，不彈對話框直接載入並顯示預覽
- [ ] 放大產生 `_upscaled` 檔案時，若同名已存在，前端彈窗確認覆寫（或自動改名）
- [ ] 放大完成（`upscale` 回傳 `completed`），預覽圖更新為放大結果
- [ ] GPU 失敗改用 CPU 時，除狀態列文字外，有視覺化提示（圖示變色或 toast）
- [ ] 目標尺寸超過 4096×4096 或預估緩衝 > 64 MB 時，`upscale` 回傳錯誤「target dimensions exceed limit」
- [ ] 現有功能不倒退：選檔、放大、匯出、轉換流程正常

---

## Additional context

- Task #1 已完成：非同步放大、Channel 進度、取消、路徑允許清單、CSP、Capability
- 本任務專注於前端 UX 細節與防護性檢查
- 需配合 Task #1 的 `upscale` 回傳格式（含 `used_fallback`、`output_path`、`preview` base64）

---

## 已確認

- 無待確認事項

---

## 非本期範圍

- 批次處理、保持長寬比、自訂輸出資料夾/檔名樣板、最近開啟歷史、設定檔持久化、HDR/WebP 有損編碼（屬 P4 功能增強）