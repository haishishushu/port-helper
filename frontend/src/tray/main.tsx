import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { TrayMenu } from "@/tray/TrayMenu";
import "@/index.css";

// 托盘菜单不需要浏览器右键菜单
window.addEventListener("contextmenu", (e) => e.preventDefault());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <TrayMenu />
  </StrictMode>,
);
