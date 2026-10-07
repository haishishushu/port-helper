import { CircleCheck, CircleX, Info, TriangleAlert, X, type LucideIcon } from "lucide-react";
import { toast } from "sonner";
import type { AppError } from "@/bindings/AppError";
import { platform } from "@/lib/platform";

type Tone = "success" | "error" | "warning" | "info";
interface Action {
  label: string;
  onClick: () => void;
}

const TONES: Record<Tone, { icon: LucideIcon; color: string; duration: number }> = {
  success: { icon: CircleCheck, color: "text-success", duration: 3000 },
  info: { icon: Info, color: "text-info", duration: 3000 },
  // 警告与错误需手动关闭（自动消失时长待确认）
  warning: { icon: TriangleAlert, color: "text-warning", duration: Infinity },
  error: { icon: CircleX, color: "text-danger", duration: Infinity },
};

function show(tone: Tone, message: string, action?: Action) {
  const { icon: Icon, color, duration } = TONES[tone];
  toast.custom(
    (id) => (
      <div className="flex max-w-[700px] items-center gap-2.5 rounded-[10px] border border-line-strong bg-surface py-2 pr-2.5 pl-3 text-[13px] text-fg shadow-[0_8px_28px_rgba(0,0,0,0.12)]">
        <Icon className={`size-4 shrink-0 ${color}`} />
        <span className="truncate">{message}</span>
        {action && (
          <>
            <span className="h-3.5 w-px bg-line-strong" />
            <button
              className="shrink-0 font-semibold hover:underline"
              onClick={() => {
                toast.dismiss(id);
                action.onClick();
              }}
            >
              {action.label}
            </button>
          </>
        )}
        <button className="shrink-0 text-fg-3 hover:text-fg" aria-label="关闭提示" onClick={() => toast.dismiss(id)}>
          <X className="size-3.5" />
        </button>
      </div>
    ),
    { duration },
  );
}

export const notify = {
  success: (message: string) => show("success", message),
  info: (message: string) => show("info", message),
  warning: (message: string, action?: Action) => show("warning", message, action),
  error: (message: string, action?: Action) => show("error", message, action),
};

export interface ErrorHandlers {
  onRelaunchAdmin?: () => void;
  onRequery?: () => void;
}

/** 按错误码映射提示文案与可执行操作（见实现文档 5.3）。 */
export function notifyError(err: AppError, prefix: string, handlers: ErrorHandlers = {}) {
  switch (err.code) {
    case "ACCESS_DENIED":
      notify.error(
        `${prefix}：拒绝访问，需要${platform.admin}权限`,
        handlers.onRelaunchAdmin && { label: `以${platform.admin}身份重新启动`, onClick: handlers.onRelaunchAdmin },
      );
      break;
    case "PROCESS_CHANGED":
      notify.warning(err.message, handlers.onRequery && { label: "重新查询", onClick: handlers.onRequery });
      break;
    case "PROCESS_NOT_FOUND":
      notify.info("进程已退出，无需操作");
      handlers.onRequery?.();
      break;
    case "UAC_CANCELLED":
      notify.warning(err.message);
      break;
    default:
      notify.error(`${prefix}：${err.message}`);
  }
}
