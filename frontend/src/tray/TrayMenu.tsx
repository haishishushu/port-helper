import { AppWindow, PlugZap, Power, type LucideIcon } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent } from "react";
import { api } from "@/lib/api";
import { darkQuery, loadSettings, onThemeChange, resolveTheme, type ThemeMode } from "@/lib/settings";
import { cn } from "@/lib/utils";

/**
 * 托盘右键菜单（独立的无边框小窗口，由 backend/src/tray.rs 创建与定位）。
 * 布局总高度必须与 tray.rs 的 MENU_HEIGHT（121）一致：
 * 上下内边距 6 + 6，品牌行 36，分隔线 1 + 上下间距 4 + 4，两个菜单项各 32。
 */
export function TrayMenu() {
  const listRef = useRef<HTMLDivElement>(null);

  // 主题：首次读取设置，之后跟随主窗口的设置变更与系统明暗变化
  useEffect(() => {
    let mode: ThemeMode = "system";
    const apply = () => {
      const dark = resolveTheme(mode, darkQuery().matches) === "dark";
      document.documentElement.classList.toggle("dark", dark);
      document.documentElement.style.colorScheme = dark ? "dark" : "light";
    };
    const mq = darkQuery();
    mq.addEventListener("change", apply);

    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      mode = (await loadSettings()).theme;
      apply();
      const off = await onThemeChange((m) => {
        mode = m;
        apply();
      });
      if (disposed) return off();
      unlisten = off;
      // 主题就绪后再报到，后端这时才显示窗口，避免首次弹出时闪一下错误的主题
      void api.trayMenuReady();
    })();
    return () => {
      disposed = true;
      unlisten?.();
      mq.removeEventListener("change", apply);
    };
  }, []);

  // 窗口失焦被收起时清掉焦点，下次弹出不残留上次的键盘高亮
  useEffect(() => {
    const onBlur = () => (document.activeElement as HTMLElement | null)?.blur();
    window.addEventListener("blur", onBlur);
    return () => window.removeEventListener("blur", onBlur);
  }, []);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      void api.trayHideMenu();
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = Array.from(listRef.current?.querySelectorAll("button") ?? []);
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "ArrowDown" ? (i + 1) % items.length : (i <= 0 ? items.length : i) - 1;
    items[next]?.focus();
  };

  return (
    <div role="menu" aria-label="port-helper" onKeyDown={onKeyDown} className="flex h-full flex-col bg-surface p-1.5">
      <div className="flex h-9 shrink-0 items-center gap-[9px] px-2.5">
        <span className="flex size-5 items-center justify-center rounded-[6px] bg-ink">
          <PlugZap className="size-3 text-on-ink" />
        </span>
        <span className="text-[13px] font-semibold">port-helper</span>
      </div>
      <div className="mx-1 my-1 h-px shrink-0 bg-line" />
      <div ref={listRef} className="flex flex-col">
        <MenuItem icon={AppWindow} label="打开主界面" onClick={() => void api.trayOpenMain()} />
        <MenuItem icon={Power} label="退出" danger onClick={() => void api.quitApp()} />
      </div>
    </div>
  );
}

function MenuItem({ icon: Icon, label, danger, onClick }: { icon: LucideIcon; label: string; danger?: boolean; onClick: () => void }) {
  return (
    <button
      role="menuitem"
      onClick={onClick}
      className={cn(
        "group flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] text-fg outline-none transition-colors",
        danger
          ? "hover:bg-danger-bg hover:text-danger focus-visible:bg-danger-bg focus-visible:text-danger"
          : "hover:bg-surface-hover focus-visible:bg-surface-hover",
      )}
    >
      <Icon className={cn("size-4 text-fg-2", danger && "group-hover:text-danger group-focus-visible:text-danger")} />
      {label}
    </button>
  );
}
