import { lazy, Suspense, useRef, useState } from "react";
import type { AppError } from "@/bindings/AppError";
import type { ProcessSummary } from "@/bindings/ProcessSummary";
import { useRelaunchAdmin } from "@/hooks/use-relaunch";
import { useSettings } from "@/hooks/use-settings";
import { api, toAppError } from "@/lib/api";
import { notify, notifyError } from "@/lib/notify";

const ProcessActionDialogs = lazy(() =>
  import("@/components/dialogs/ProcessActionDialogs").then((m) => ({ default: m.ProcessActionDialogs })),
);

/** 一次进程操作的目标：进程摘要 + 展示用的端口与提示。 */
export interface ActionTarget {
  process: ProcessSummary;
  /** 展示用，如 “TCP [::1]:5173” */
  endpoints: string[];
  /** 触发操作时所在的端口（转发进程提示里的命令要用） */
  port?: number;
  /** 父进程可能自动重启等提示 */
  note?: string | null;
}

export type Step =
  | { kind: "closeConfirm"; target: ActionTarget }
  | { kind: "closing"; target: ActionTarget; critical: boolean; deadline: number; timeoutSec: number }
  | { kind: "closeTimeout"; target: ActionTarget; critical: boolean; unsupported: boolean; timeoutSec: number }
  | { kind: "killConfirm"; target: ActionTarget; critical: boolean }
  | { kind: "critical"; target: ActionTarget; next: "close" | "kill" }
  | { kind: "forwarder"; target: ActionTarget }
  | { kind: "release"; port: number; targets: ActionTarget[]; skipped: ActionTarget[] };

export type Settled =
  | { action: "closed" | "killed"; target: ActionTarget }
  | { action: "released"; port: number; closed: ActionTarget[] };

/**
 * 进程操作状态机（实现文档 7.3）：确认 → 正常关闭（倒计时）→ 超时降级 → 强制结束。
 * 后端会再次做保护进程、关键进程确认和 PID 复用校验，这里只是第一道防线。
 */
export function useProcessActions({ onSettled, onRequery }: { onSettled: (s: Settled) => void; onRequery: () => void }) {
  const { settings } = useSettings();
  const requestRelaunch = useRelaunchAdmin();
  const [step, setStep] = useState<Step | null>(null);
  const [busy, setBusy] = useState(false);
  /** 每次发起新操作递增，用于丢弃已被“立即强制结束”取代的关闭结果 */
  const token = useRef(0);
  /** 立即释放端口时的待关闭队列 */
  const release = useRef<{ port: number; queue: ActionTarget[]; closed: ActionTarget[] } | null>(null);

  const fail = (err: AppError, target: ActionTarget, op: "close" | "kill") => {
    release.current = null;
    setStep(null);
    if (err.code === "CRITICAL_CONFIRM_REQUIRED") {
      setStep({ kind: "critical", target, next: op });
      return;
    }
    notifyError(err, op === "close" ? "关闭失败" : "结束失败", { onRelaunchAdmin: requestRelaunch, onRequery });
  };

  const succeeded = (action: "closed" | "killed", target: ActionTarget) => {
    const r = release.current;
    if (r) {
      r.closed.push(target);
      const next = r.queue.shift();
      if (next) {
        void close(next, false);
        return;
      }
      release.current = null;
      setStep(null);
      onSettled({ action: "released", port: r.port, closed: r.closed });
      return;
    }
    setStep(null);
    onSettled({ action, target });
  };

  const close = async (target: ActionTarget, critical: boolean) => {
    const my = ++token.current;
    const timeoutSec = settings.closeTimeoutSec;
    setStep({ kind: "closing", target, critical, deadline: Date.now() + timeoutSec * 1000, timeoutSec });
    try {
      const { pid, startTime } = target.process;
      const r = await api.closeProcess(pid, startTime ?? 0, timeoutSec * 1000, critical);
      if (my !== token.current) return;
      if (r.outcome === "exited") succeeded("closed", target);
      else setStep({ kind: "closeTimeout", target, critical, unsupported: r.outcome === "unsupported", timeoutSec });
    } catch (err) {
      if (my === token.current) fail(toAppError(err), target, "close");
    }
  };

  const kill = async (target: ActionTarget, critical: boolean) => {
    const my = ++token.current;
    setBusy(true);
    try {
      const { pid, startTime } = target.process;
      const r = await api.killProcess(pid, startTime ?? 0, critical);
      if (my !== token.current) return;
      if (!r.exited) notify.warning(`已发送结束请求，但 ${target.process.name} 仍在退出中`);
      succeeded("killed", target);
    } catch (err) {
      if (my === token.current) fail(toAppError(err), target, "kill");
    } finally {
      setBusy(false);
    }
  };

  const cancel = () => {
    token.current++;
    if (release.current) {
      release.current = null;
      onRequery();
    }
    setStep(null);
  };

  const requestClose = (target: ActionTarget) => {
    const { kind } = target.process;
    if (kind === "forwarder") setStep({ kind: "forwarder", target });
    else if (kind === "critical") setStep({ kind: "critical", target, next: "close" });
    else setStep({ kind: "closeConfirm", target });
  };

  const requestKill = (target: ActionTarget) => {
    const { kind } = target.process;
    if (kind === "forwarder") setStep({ kind: "forwarder", target });
    else if (kind === "critical") setStep({ kind: "critical", target, next: "kill" });
    else setStep({ kind: "killConfirm", target, critical: false });
  };

  /** 立即释放端口：只自动处理普通进程与服务；关键进程与转发进程需单独操作。 */
  const requestRelease = (port: number, targets: ActionTarget[]) => {
    const actionable = targets.filter((t) => t.process.actionable);
    const auto = actionable.filter((t) => t.process.kind === "normal" || t.process.kind === "service");
    const skipped = actionable.filter((t) => !auto.includes(t));
    if (auto.length === 0) {
      notify.info("没有可以自动释放的进程，请在详情中单独处理");
      return;
    }
    setStep({ kind: "release", port, targets: auto, skipped });
  };

  const handlers = {
    cancel,
    confirmClose: (t: ActionTarget, critical = false) => void close(t, critical),
    confirmKill: (t: ActionTarget, critical: boolean) => void kill(t, critical),
    /** 关闭超时或无法正常关闭 → 用户选择强制结束 */
    forceAfterClose: (t: ActionTarget, critical: boolean) => void kill(t, critical),
    /** 关闭等待中点击“立即强制结束” */
    forceNow: (t: ActionTarget, critical: boolean) => void kill(t, critical),
    confirmCritical: (t: ActionTarget, next: "close" | "kill") =>
      next === "close" ? void close(t, true) : setStep({ kind: "killConfirm", target: t, critical: true }),
    confirmForwarderKill: (t: ActionTarget) => void kill(t, false),
    confirmRelease: (port: number, targets: ActionTarget[]) => {
      const [first, ...rest] = targets;
      release.current = { port, queue: rest, closed: [] };
      void close(first, false);
    },
  };

  // 确认框按需加载：没有进行中的操作时不挂载，也不加载对应代码
  const element = step && (
    <Suspense fallback={null}>
      <ProcessActionDialogs step={step} busy={busy} handlers={handlers} />
    </Suspense>
  );
  return { requestClose, requestKill, requestRelease, element };
}

export type ActionHandlers = {
  cancel: () => void;
  confirmClose: (t: ActionTarget, critical?: boolean) => void;
  confirmKill: (t: ActionTarget, critical: boolean) => void;
  forceAfterClose: (t: ActionTarget, critical: boolean) => void;
  forceNow: (t: ActionTarget, critical: boolean) => void;
  confirmCritical: (t: ActionTarget, next: "close" | "kill") => void;
  confirmForwarderKill: (t: ActionTarget) => void;
  confirmRelease: (port: number, targets: ActionTarget[]) => void;
};
