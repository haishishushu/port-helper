import { useEffect, useRef } from "react";

export const isRefreshShortcut = (e: KeyboardEvent) =>
  e.key === "F5" || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "r");

/** 当前页面可见时，F5 / Ctrl+R 触发页面内刷新（WebView 的整页重载已在 main.tsx 全局拦截）。 */
export function useRefreshShortcut(active: boolean, onRefresh: () => void) {
  const handler = useRef(onRefresh);
  handler.current = onRefresh;
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (isRefreshShortcut(e) && !e.repeat) handler.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active]);
}
