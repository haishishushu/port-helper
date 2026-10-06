import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "@/App";
import { isRefreshShortcut } from "@/hooks/use-refresh-shortcut";
import "@/index.css";

// 桌面应用不需要浏览器默认右键菜单
window.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
});

// F5 / Ctrl+R 在 WebView 中会重载整页、丢失查询状态，统一拦截，交给页面内刷新处理
window.addEventListener("keydown", (e) => {
  if (isRefreshShortcut(e)) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
