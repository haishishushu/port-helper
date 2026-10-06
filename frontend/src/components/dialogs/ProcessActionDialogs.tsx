import {
  AppWindow, CircleCheck, ClockAlert, Clock3, Container, Copy, Cpu, Folder, Loader, LoaderCircle, Plug, Power,
  RotateCcw, Shield, ShieldAlert, TriangleAlert, Unplug, Zap,
} from "lucide-react";
import { useEffect, useState } from "react";
import { ConfirmDialog, type InfoRow } from "@/components/dialogs/ConfirmDialog";
import { Note } from "@/components/dialogs/Note";
import { Button } from "@/components/ui/button";
import type { ActionHandlers, ActionTarget, Step } from "@/hooks/use-process-actions";
import { formatDateTime } from "@/lib/format";
import { notify } from "@/lib/notify";

const procLabel = (t: ActionTarget) => `${t.process.name} · PID ${t.process.pid}`;

function targetRows(t: ActionTarget, withApp = false): InfoRow[] {
  const rows: InfoRow[] = [];
  if (withApp) rows.push({ icon: AppWindow, label: "应用", value: t.process.appName });
  rows.push({ icon: Cpu, label: "进程", value: procLabel(t) });
  if (t.process.startTime != null) rows.push({ icon: Clock3, label: "启动时间", value: formatDateTime(t.process.startTime) });
  if (t.endpoints.length > 0) rows.push({ icon: Plug, label: "端口", value: t.endpoints.slice(0, 2).join("，") });
  return rows;
}

/** 剩余秒数：对齐到整秒边界更新，每秒只重渲染一次（perf-todo P2-5） */
function useRemainingSeconds(deadline: number) {
  const [remain, setRemain] = useState(() => Math.max(0, Math.ceil((deadline - Date.now()) / 1000)));
  useEffect(() => {
    let timer = 0;
    const tick = () => {
      const left = deadline - Date.now();
      setRemain(Math.max(0, Math.ceil(left / 1000)));
      if (left > 0) timer = window.setTimeout(tick, left % 1000 || 1000);
    };
    tick();
    return () => window.clearTimeout(timer);
  }, [deadline]);
  return remain;
}

function ClosingDialog({ step, h }: { step: Extract<Step, { kind: "closing" }>; h: ActionHandlers }) {
  const remain = useRemainingSeconds(step.deadline);
  // 进度条用一次性 CSS 动画（transform，走合成层），负的 delay 让重新挂载时从当前进度接着走
  const [totalMs] = useState(() => step.timeoutSec * 1000);
  const [elapsedMs] = useState(() => Math.max(0, totalMs - (step.deadline - Date.now())));
  const t = step.target;
  const rows: InfoRow[] = [
    { icon: CircleCheck, label: "校验 PID 与启动时间", value: "通过" },
    { icon: CircleCheck, label: "发送关闭信号（窗口关闭 / Ctrl+C）", value: "已发送" },
    { icon: Loader, label: "等待进程退出", value: `剩余 ${remain} 秒` },
  ];
  return (
    <ConfirmDialog
      open
      onClose={() => {}}
      dismissible={false}
      tone="neutral"
      icon={LoaderCircle}
      spinning
      title={`正在关闭 ${t.process.appName}`}
      description={`已请求 ${t.process.name}（PID ${t.process.pid}）自行退出，等待中……`}
      rows={rows}
      actions={
        <Button variant="danger" onClick={() => h.forceNow(t, step.critical)}>
          <Zap />
          立即强制结束
        </Button>
      }
    >
      <div className="flex flex-col gap-1.5">
        <div className="h-1 overflow-hidden rounded-full bg-surface-hover">
          <div
            className="close-progress h-full rounded-full bg-ink"
            style={{ animationDuration: `${totalMs}ms`, animationDelay: `-${elapsedMs}ms` }}
          />
        </div>
        <span className="text-[11.5px] text-fg-3">超过 {step.timeoutSec} 秒未退出，将提示是否强制结束</span>
      </div>
    </ConfirmDialog>
  );
}

function CriticalDialog({ step, h }: { step: Extract<Step, { kind: "critical" }>; h: ActionHandlers }) {
  const [typed, setTyped] = useState("");
  const t = step.target;
  const ok = typed.trim().toLowerCase() === t.process.name.toLowerCase();
  return (
    <ConfirmDialog
      open
      onClose={h.cancel}
      tone="danger"
      icon={ShieldAlert}
      title="这是 Windows 关键进程"
      description={`${step.next === "close" ? "关闭" : "结束"} ${t.process.name} 可能导致系统不稳定、立即重启或无法登录。除非你非常清楚后果，否则请不要继续。`}
      rows={[
        { icon: Cpu, label: "进程", value: procLabel(t) },
        { icon: Folder, label: "路径", value: t.process.exePath ?? "无法读取" },
        { icon: Shield, label: "类型", value: t.process.appName },
      ]}
      actions={
        <>
          <Button variant="outline" onClick={h.cancel} autoFocus>
            取消
          </Button>
          <Button variant="destructive" disabled={!ok} onClick={() => h.confirmCritical(t, step.next)}>
            <Zap />
            {step.next === "close" ? "关闭进程" : "继续强制结束"}
          </Button>
        </>
      }
    >
      <label className="flex flex-col gap-1.5">
        <span className="text-xs text-fg-2">请输入进程名 {t.process.name} 以确认</span>
        <input
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          placeholder={t.process.name}
          spellCheck={false}
          className="selectable h-9 rounded-lg border border-line-strong bg-surface px-3 font-mono text-[13px] text-fg outline-none placeholder:text-fg-3 focus:border-fg-3"
        />
      </label>
    </ConfirmDialog>
  );
}

function CommandLine({ text }: { text: string }) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-lg bg-surface-muted px-3 py-2">
      <code className="selectable truncate font-mono text-xs text-fg">{text}</code>
      <button
        className="text-fg-3 hover:text-fg"
        aria-label="复制命令"
        onClick={() => navigator.clipboard.writeText(text).then(() => notify.success("已复制命令"))}
      >
        <Copy className="size-[13px]" />
      </button>
    </div>
  );
}

function ForwarderDialog({ step, h }: { step: Extract<Step, { kind: "forwarder" }>; h: ActionHandlers }) {
  const t = step.target;
  const wsl = t.process.name.toLowerCase().startsWith("wsl");
  const port = t.port ?? "<端口>";
  return (
    <ConfirmDialog
      open
      onClose={h.cancel}
      tone="warning"
      icon={Container}
      title={wsl ? `端口 ${port} 由 WSL 转发` : `端口 ${port} 由 ${t.process.appName} 转发`}
      description={
        wsl
          ? `${t.process.name} 负责把 WSL 内的端口映射到 Windows，结束它会影响 WSL 的网络。建议在 WSL 内结束真正占用端口的进程。`
          : `${t.process.name} 负责把容器端口映射到本机，结束它会影响 Docker Desktop 的正常运行。建议停止对应的容器。`
      }
      actions={
        <>
          <Button variant="outline" onClick={h.cancel} autoFocus>
            取消
          </Button>
          <Button variant="danger" onClick={() => h.confirmForwarderKill(t)}>
            <Zap />
            仍要强制结束
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-1.5">
        <span className="text-xs text-fg-2">
          {wsl ? `在 WSL 终端中查找占用 ${port} 的进程：` : `在终端中查找并停止占用 ${port} 的容器：`}
        </span>
        {wsl ? (
          <CommandLine text={`ss -ltnp | grep :${port}`} />
        ) : (
          <>
            <CommandLine text={`docker ps --filter "publish=${port}"`} />
            <CommandLine text="docker stop <容器名>" />
          </>
        )}
      </div>
    </ConfirmDialog>
  );
}

export function ProcessActionDialogs({ step, busy, handlers: h }: { step: Step | null; busy: boolean; handlers: ActionHandlers }) {
  if (!step) return null;
  switch (step.kind) {
    case "closeConfirm": {
      const t = step.target;
      return (
        <ConfirmDialog
          open
          onClose={h.cancel}
          tone="neutral"
          icon={Power}
          title={`关闭 ${t.process.appName}？`}
          description="会先请求进程自行退出（有窗口的发送关闭消息，控制台进程发送 Ctrl+C），超时仍未退出时，再询问是否强制结束。"
          rows={targetRows(t)}
          actions={
            <>
              <Button variant="outline" onClick={h.cancel} autoFocus>
                取消
              </Button>
              <Button onClick={() => h.confirmClose(t)}>
                <Power />
                关闭进程
              </Button>
            </>
          }
        >
          {t.note && (
            <Note tone="warning" icon={RotateCcw}>
              {t.note}
            </Note>
          )}
        </ConfirmDialog>
      );
    }
    case "closing":
      return <ClosingDialog step={step} h={h} />;
    case "closeTimeout": {
      const t = step.target;
      return (
        <ConfirmDialog
          open
          onClose={h.cancel}
          tone="warning"
          icon={ClockAlert}
          title={step.unsupported ? "无法正常关闭该进程" : `进程未在 ${step.timeoutSec} 秒内退出`}
          description={
            step.unsupported
              ? `${t.process.name}（PID ${t.process.pid}）没有可见窗口，也无法接收 Ctrl+C，只能强制结束。`
              : `${t.process.name}（PID ${t.process.pid}）没有响应关闭请求。可能正在执行清理，也可能忽略了关闭信号。`
          }
          rows={targetRows(t)}
          actions={
            <>
              <Button variant="outline" onClick={h.cancel} autoFocus>
                暂不处理
              </Button>
              <Button variant="danger" disabled={busy} onClick={() => h.forceAfterClose(t, step.critical)}>
                <Zap />
                强制结束
              </Button>
            </>
          }
        />
      );
    }
    case "killConfirm": {
      const t = step.target;
      return (
        <ConfirmDialog
          open
          onClose={h.cancel}
          tone="danger"
          icon={TriangleAlert}
          title={`强制结束 ${t.process.name}？`}
          description="进程会被立即终止，不会执行任何清理逻辑，未保存的数据可能丢失。"
          rows={targetRows(t, true)}
          actions={
            <>
              <Button variant="outline" onClick={h.cancel} autoFocus>
                取消
              </Button>
              <Button variant="destructive" disabled={busy} onClick={() => h.confirmKill(t, step.critical)}>
                {busy ? <LoaderCircle className="animate-spin" /> : <Zap />}
                确认强制结束
              </Button>
            </>
          }
        />
      );
    }
    case "critical":
      return <CriticalDialog key={step.target.process.pid} step={step} h={h} />;
    case "forwarder":
      return <ForwarderDialog step={step} h={h} />;
    case "release":
      return (
        <ConfirmDialog
          open
          onClose={h.cancel}
          tone="neutral"
          icon={Unplug}
          title={`释放端口 ${step.port}？`}
          description="将按“正常关闭”流程依次关闭占用该端口的进程，完成后自动重新查询。"
          rows={step.targets.map((t) => ({ icon: Zap, label: `${t.process.name}  ${t.process.pid}`, value: t.process.appName, monoLabel: true }))}
          actions={
            <>
              <Button variant="outline" onClick={h.cancel} autoFocus>
                取消
              </Button>
              <Button onClick={() => h.confirmRelease(step.port, step.targets)}>
                <Unplug />
                释放端口
              </Button>
            </>
          }
        >
          {step.skipped.length > 0 && (
            <Note tone="warning" icon={TriangleAlert}>
              {step.skipped.map((t) => t.process.name).join("、")} 是关键进程或端口转发进程，不会自动关闭，请在详情中单独处理。
            </Note>
          )}
        </ConfirmDialog>
      );
  }
}
