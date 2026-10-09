import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 开发模式固定使用 5173 端口（见 backend/tauri.conf.json 的 devUrl）
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // 两个页面：主界面与托盘菜单（托盘菜单是独立的小窗口，见 backend/src/tray.rs）
  input: {
    main: path.resolve(import.meta.dirname, "index.html"),
    tray: path.resolve(import.meta.dirname, "tray.html"),
  },
  // 运行平台（见 src/lib/platform.ts）：各平台安装包都在对应系统上构建
  define: { __PLATFORM__: JSON.stringify(process.platform) },
  resolve: {
    alias: { "@": path.resolve(import.meta.dirname, "./src") },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/backend/**"] },
  },
  build: {
    target: "chrome120",
  },
});
