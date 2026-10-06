import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { TONE, type DialogTone } from "@/components/dialogs/Note";
import { cn } from "@/lib/utils";

export interface InfoRow {
  icon: LucideIcon;
  label: string;
  value: ReactNode;
  /** 行首缩进（进程树子进程） */
  indent?: boolean;
  /** 标签用等宽字体（进程名 + PID） */
  monoLabel?: boolean;
}

/** 设计稿“12 确认框”的统一外壳：图标 + 标题 + 说明 + 信息框 + 扩展内容 + 操作按钮。 */
export function ConfirmDialog({
  open,
  onClose,
  tone,
  icon: Icon,
  spinning,
  title,
  description,
  rows,
  children,
  actions,
  dismissible = true,
}: {
  open: boolean;
  onClose: () => void;
  tone: DialogTone;
  icon: LucideIcon;
  spinning?: boolean;
  title: string;
  description: ReactNode;
  rows?: InfoRow[];
  children?: ReactNode;
  actions: ReactNode;
  /** 进行中的操作不允许点遮罩关闭 */
  dismissible?: boolean;
}) {
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent
        showCloseButton={false}
        onInteractOutside={(e) => !dismissible && e.preventDefault()}
        className="gap-[18px] rounded-[14px] border-line bg-surface p-[22px] shadow-[0_16px_48px_rgba(0,0,0,0.16)] sm:max-w-[440px]"
      >
        <div className="flex gap-3">
          <span className={cn("flex size-9 shrink-0 items-center justify-center rounded-full", TONE[tone])}>
            <Icon className={cn("size-[18px]", spinning && "animate-spin")} />
          </span>
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <DialogTitle className="text-base leading-snug font-semibold text-fg">{title}</DialogTitle>
            <DialogDescription className="text-[13px] leading-normal text-fg-2">{description}</DialogDescription>
          </div>
        </div>
        {rows && rows.length > 0 && (
          <div className="flex flex-col gap-[9px] rounded-[10px] bg-surface-muted px-3.5 py-3">
            {rows.map((r, i) => (
              <div key={i} className={cn("flex items-center gap-2", r.indent && "pl-[18px]")}>
                <r.icon className="size-3.5 shrink-0 text-fg-3" />
                <span className={cn("text-[12.5px]", r.monoLabel ? "font-mono text-fg" : "text-fg-2")}>{r.label}</span>
                <span className="flex-1" />
                <span className={cn("truncate text-xs", r.monoLabel ? "text-fg-3" : "font-mono text-fg")}>{r.value}</span>
              </div>
            ))}
          </div>
        )}
        {children}
        <div className="flex justify-end gap-2">{actions}</div>
      </DialogContent>
    </Dialog>
  );
}
