import { defineConfig } from "vite"

export default defineConfig({
  build: {
    outDir: "../src-tauri/gen/dist",
    emptyOutDir: true,
  },
  server: {
    port: 1420,
    strictPort: true,
  },
})
