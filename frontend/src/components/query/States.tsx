import { CircleX, Copy, EyeOff, LoaderCircle, RefreshCw, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import type { AppError } from "@/bindings/AppError";
import type { HiddenState } from "@/bindings/HiddenState";
import { Button } from "@/components/ui/button";
import { hiddenSummary } from "@/lib/format";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";

export function LoadingState({ label }: { label: string }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2 text-[13px] text-fg-2">
        <LoaderCircle className="size-3.5 animate-spin" />
        {label}
      </div>
      <div className="flex flex-col gap-3 rounded-xl border border-line p-4">
        <div className="flex items-center gap-3">
          <div className="size-[34px] animate-pulse rounded-[9px] bg-surface-muted" />
          <div className="flex flex-col gap-2">
            <div className="h-2.5 w-36 animate-pulse rounded bg-surface-muted" />
            <div className="h-2 w-24 animate-pulse rounded bg-surface-muted" />
          </div>
        </div>
        <div className="h-2 w-full animate-pulse rounded bg-surface-muted" />
      </div>
    </div>
  );
}

export function QueryErrorState({ error, onRetry }: { error: AppError; onRetry: () => void }) {
  const detail = `${error.code} · ${error.message}`;
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-2 rounded-[10px] border border-danger-border bg-danger-bg p-3.5">
        <div className="flex items-center gap-2 text-[13px] font-semibold text-danger">
          <CircleX className="size-[15px]" />
          查询失败
        </div>
        <p className="text-[12.5px] leading-relaxed">读取系统端口表时出错，请稍后重试。若持续失败，请把错误信息反馈给开发者。</p>
        <code className="selectable font-mono text-[11px] text-fg-3">{detail}</code>
      </div>
      <div className="flex gap-2">
        <Button variant="outline" onClick={onRetry}>
          <RefreshCw />
          重试
        </Button>
        <Button variant="ghost" onClick={() => navigator.clipboard.writeText(detail).then(() => notify.success("已复制错误信息"))}>
          <Copy />
          复制错误信息
        </Button>
      </div>
    </div>
  );
}

const ICON_TONE = {
  neutral: "bg-surface-muted text-fg-2",
  success: "bg-success-bg text-success",
  danger: "bg-danger-bg text-danger",
};

export function EmptyState({ icon: Icon, tone = "neutral", title, description, children, className }: {
  icon: LucideIcon;
  tone?: keyof typeof ICON_TONE;
  title: string;
  description: string;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-1 flex-col items-center justify-center gap-3 pb-20 text-center", className)}>
      <span className={cn("flex size-[52px] items-center justify-center rounded-full", ICON_TONE[tone])}>
        <Icon className="size-6" />
      </span>
      <span className="text-lg font-semibold">{title}</span>
      <p className="max-w-[380px] text-[13px] leading-relaxed text-fg-2">{description}</p>
      {children}
    </div>
  );
}

export function HiddenNote({ hidden, onShowAll }: { hidden: HiddenState[]; onShowAll: () => void }) {
  const { total, detail } = hiddenSummary(hidden);
  if (total === 0) return null;
  return (
    <div className="flex items-center gap-1.5 px-1 text-xs text-fg-3">
      <EyeOff className="size-[13px]" />
      已隐藏 {total} 条非 LISTEN 记录（{detail}）
      <button onClick={onShowAll} className="font-medium text-fg underline">
        显示全部状态
      </button>
    </div>
  );
}
