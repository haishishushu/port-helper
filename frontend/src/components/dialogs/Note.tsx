import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export type DialogTone = "neutral" | "danger" | "warning" | "info";

export const TONE: Record<DialogTone, string> = {
  neutral: "bg-surface-muted text-fg",
  danger: "bg-danger-bg text-danger",
  warning: "bg-warning-bg text-warning",
  info: "bg-info-bg text-info",
};

/** 提示条。单独成文件：详情面板常驻使用它，不应因此把整个 Dialog 打进首屏包。 */
export function Note({ tone, icon: Icon, children }: { tone: DialogTone; icon: LucideIcon; children: ReactNode }) {
  return (
    <div className={cn("flex gap-2 rounded-lg px-2.5 py-2 text-xs leading-normal", TONE[tone])}>
      <Icon className="mt-0.5 size-[13px] shrink-0" />
      <span>{children}</span>
    </div>
  );
}
