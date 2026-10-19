import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronLeft, Minus, Square, X } from "lucide-react";
import { cn } from "@/lib/utils";

const controls = [
  { key: "min", icon: Minus, label: "最小化", run: () => getCurrentWindow().minimize() },
  { key: "max", icon: Square, label: "最大化", run: () => getCurrentWindow().toggleMaximize() },
  { key: "close", icon: X, label: "关闭到托盘", run: () => getCurrentWindow().close() },
] as const;

interface TitleBarProps {
  title: string;
  /** 结果页的当前位置，如“端口 5173”；有值时标题显示为面包屑 */
  crumb?: string;
  /** 结果页返回当前页面的主页；不传则不显示返回按钮 */
  onBack?: () => void;
  backLabel?: string;
}

export function TitleBar({ title, crumb, onBack, backLabel }: TitleBarProps) {
  return (
    <div className="flex h-10 shrink-0 items-center">
      {onBack && (
        <div className="group relative ml-3">
          <button
            aria-label={backLabel ?? "返回"}
            onClick={onBack}
            className="flex size-[26px] items-center justify-center rounded-md text-fg-2 transition-colors hover:bg-surface-hover hover:text-fg"
          >
            <ChevronLeft className="size-4" />
          </button>
          {backLabel && (
            <span
              role="tooltip"
              className="pointer-events-none absolute top-8 left-0 z-50 rounded-md bg-ink px-2 py-1 text-xs whitespace-nowrap text-on-ink opacity-0 transition-opacity delay-300 group-hover:opacity-100"
            >
              {backLabel}
            </span>
          )}
        </div>
      )}
      <div
        data-tauri-drag-region
        className={cn("flex h-full min-w-0 flex-1 items-center gap-1.5 text-[13px]", onBack ? "pl-2" : "pl-5")}
      >
        <span className={cn("pointer-events-none", crumb ? "text-fg-3" : "text-fg-2")}>{title}</span>
        {crumb && (
          <>
            <span className="pointer-events-none text-fg-3">/</span>
            <span className="pointer-events-none truncate font-medium text-fg">{crumb}</span>
          </>
        )}
      </div>
      <div className="flex h-full">
        {controls.map(({ key, icon: Icon, label, run }) => (
          <button
            key={key}
            aria-label={label}
            title={label}
            onClick={() => isTauri() && void run()}
            className={cn(
              "flex h-full w-11 items-center justify-center text-fg-2 transition-colors hover:bg-surface-hover",
              key === "close" && "hover:bg-[#c42b1c] hover:text-white",
            )}
          >
            <Icon className={key === "max" ? "size-3" : "size-[15px]"} />
          </button>
        ))}
      </div>
    </div>
  );
}
