# CookieSincraft 曲奇芯刻

Tauri 2 + Rust 桌面圖片放大 / 格式轉換工具。支援 GPU（wgpu：Anime4K、FSR EASU/RCAS、NIS shader）與 CPU（Lanczos3）放大，輸出 PNG / JPEG / WebP 等格式。

## 下載

到 [Releases](../../releases/latest) 下載 Windows 版：

- `CookieSincraft_x.y.z_x64-setup.exe` — NSIS 安裝檔
- `CookieSincraft_x.y.z_x64_en-US.msi` — MSI 安裝檔
- `cookie-sincraft.exe` — 免安裝執行檔（需系統已有 WebView2）

## 從原始碼建置

需求：Rust (stable)、Node.js、Windows 需 WebView2。

```bash
cd src
npm install
npm run build          # 前端輸出到 src-tauri/gen/dist
cd ..
./src/node_modules/.bin/tauri build
```

產物位於 `target/release/` 與 `target/release/bundle/`。

## 測試

```bash
cargo test -p cookie-sincraft
```
