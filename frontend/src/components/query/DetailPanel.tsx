import {
  AppWindow, Copy, CornerDownRight, Info, Lock, Power, RotateCcw, ScanSearch, ShieldCheck, SquareTerminal, Zap,
} from "lucide-react";
import type { ProcessDetail } from "@/bindings/ProcessDetail";
import { Note } from "@/components/dialogs/Note";
import { Button } from "@/components/ui/button";
import { usePrivilege } from "@/hooks/use-privilege";
import { useRelaunchAdmin } from "@/hooks/use-relaunch";
import { formatDateTime, formatUptime } from "@/lib/format";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";

const copy = (text: string, label: string) =>
  navigator.clipboard.writeText(text).then(() => notify.success(`已复制${label}`));

/** 根据父进程链与类型给出操作前提示（父进程自动重启、服务、转发等）。 */
export function processNote(d: ProcessDetail): { tone: "warning" | "info"; icon: typeof Info; text: string } | null {
  const ancestors = d.parentChain.slice(0, -1).map((p) => p.name.toLowerCase());
  if (d.kind === "service" && d.services.length > 0)
    return { tone: "info", icon: Info, text: `这是 Windows 服务 ${d.services.join("、")}，建议在“服务”中停止。` };
  if (d.kind === "forwarder")
    return { tone: "warning", icon: Info, text: `${d.name} 是端口转发进程，建议停止对应的容器或在 WSL 内处理。` };
  if (ancestors.some((n) => n.startsWith("idea")))
    return { tone: "info", icon: Info, text: "该进程由 IntelliJ IDEA 启动，在 IDE 中停止运行更稳妥。" };
  if (ancestors.includes("node.exe"))
    return { tone: "warning", icon: RotateCcw, text: "该进程由 Node.js 工具（如 npm）启动。若父进程带自动重启（如 nodemon），结束后可能被重新拉起。" };
  return null;
}

function Field({ label, value, mono = true, copyLabel }: { label: string; value: string | null; mono?: boolean; copyLabel?: string }) {
  const { elevated } = usePrivilege();
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <span className="text-xs text-fg-3">{label}</span>
        {value && copyLabel ? (
          <button aria-label={`复制${copyLabel}`} onClick={() => copy(value, copyLabel)} className="text-fg-3 hover:text-fg">
            <Copy className="size-[13px]" />
          </button>
        ) : (
          !value && <Lock className="size-[13px] text-fg-3" />
        )}
      </div>
      {value ? (
        <span className={cn("selectable text-[12.5px] leading-normal break-all", mono && "font-mono")}>{value}</span>
      ) : (
        <span className="text-[12.5px] text-fg-3">{elevated ? "无法读取" : "需要管理员权限才能读取"}</span>
      )}
    </div>
  );
}

function ParentChain({ detail }: { detail: ProcessDetail }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-xs text-fg-3">父进程链</span>
      <div className="flex flex-col rounded-[10px] border border-line bg-surface py-1">
        {detail.parentChain.map((p, i) => {
          const current = i === detail.parentChain.length - 1;
          const Icon = i === 0 ? (current ? AppWindow : SquareTerminal) : CornerDownRight;
          return (
            <div key={p.pid} className="flex items-center gap-2 py-[7px] pr-3" style={{ paddingLeft: 12 + Math.min(i, 4) * 16 }}>
              <Icon className="size-[13px] shrink-0 text-fg-3" />
              <span className={cn("truncate font-mono text-xs", current ? "font-semibold text-fg" : "text-fg-2")}>{p.name}</span>
              <span className="font-mono text-[11px] text-fg-3">{p.pid}</span>
              <span className="flex-1" />
              {current && <span className="text-[11px] text-fg-3">当前进程</span>}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function SystemExplain({ detail, port }: { detail: ProcessDetail; port?: number }) {
  const httpSys = detail.pid === 4 && detail.appName.startsWith("HTTP.sys");
  return (
    <>
      <div className="flex flex-col gap-2.5 rounded-[10px] border border-warning-border bg-warning-bg p-3.5">
        <div className="flex items-center gap-2 text-[13px] font-semibold text-warning">
          <Lock className="size-3.5" />
          此进程无法结束
        </div>
        <p className="text-[12.5px] leading-relaxed">{detail.blockedReason}</p>
      </div>
      {httpSys && (
        <div className="flex flex-col gap-2.5">
          <span className="text-xs text-fg-3">排查建议</span>
          <p className="text-[12.5px] leading-relaxed">1. 在终端运行下面的命令，查看是哪个服务注册了{port ? `端口 ${port}` : "该端口"}：</p>
          <div className="flex items-center justify-between rounded-lg border border-line bg-surface px-3 py-2.5">
            <code className="selectable font-mono text-xs">netsh http show servicestate</code>
            <button aria-label="复制命令" onClick={() => copy("netsh http show servicestate", "命令")} className="text-fg-3 hover:text-fg">
              <Copy className="size-[13px]" />
            </button>
          </div>
          <p className="text-[12.5px] leading-relaxed">
            2. 常见来源：IIS（W3SVC 服务）、SQL Server Reporting Services 等。请在“服务”中停止对应服务，而不是结束进程。
          </p>
        </div>
      )}
    </>
  );
}

/** 右侧进程详情面板：详情随查询结果一起返回（端口查询与进程反查都直接传入），不再单独请求。 */
export function DetailPanel({ detail, port, showAllPortsLink, onShowAllPorts, onClose, onKill }: {
  detail: ProcessDetail;
  port?: number;
  showAllPortsLink?: boolean;
  onShowAllPorts?: (pid: number) => void;
  onClose: (d: ProcessDetail) => void;
  onKill: (d: ProcessDetail) => void;
}) {
  const requestRelaunch = useRelaunchAdmin();
  const { elevated } = usePrivilege();
  const blocked = detail.kind === "system" || detail.kind === "idle" || detail.kind === "self";
  const note = processNote(detail);

  return (
    <aside className="flex w-[400px] shrink-0 flex-col border-l border-line bg-sidebar">
      <div className="flex flex-1 flex-col gap-5 overflow-y-auto p-5">
        <div className="flex flex-col gap-1.5">
          <span className="text-xs text-fg-3">进程详情</span>
          <span className="text-lg font-semibold">{detail.appName}</span>
          <div className="flex items-center gap-1.5 font-mono text-xs text-fg-2">
            {detail.name} <span className="text-fg-3">·</span> PID {detail.pid}
            <button aria-label="复制 PID" onClick={() => copy(String(detail.pid), ` PID ${detail.pid}`)} className="text-fg-3 hover:text-fg">
              <Copy className="size-[13px]" />
            </button>
          </div>
          {showAllPortsLink && onShowAllPorts && (
            <button
              onClick={() => onShowAllPorts(detail.pid)}
              className="flex w-fit items-center gap-1 pt-1 text-xs font-medium text-fg-2 underline hover:text-fg"
            >
              <ScanSearch className="size-[13px]" />
              查看该进程的全部端口
            </button>
          )}
        </div>
        {blocked ? (
          <SystemExplain detail={detail} port={port} />
        ) : (
          <>
            <div className="flex flex-col gap-3.5">
              <Field label="可执行文件" value={detail.exePath} copyLabel="路径" />
              <Field label="命令行" value={detail.commandLine} copyLabel="命令行" />
              <Field label="工作目录" value={detail.cwd} copyLabel="工作目录" />
              <Field
                label="启动时间"
                mono={false}
                value={detail.startTime != null ? `${formatDateTime(detail.startTime)} · ${formatUptime(detail.startTime)}` : null}
              />
            </div>
            {detail.parentChain.length > 1 && <ParentChain detail={detail} />}
            {note && (
              <Note tone={note.tone} icon={note.icon}>
                {note.text}
              </Note>
            )}
            {!detail.actionable && detail.blockedReason && (
              <Note tone="warning" icon={Lock}>
                {detail.blockedReason}
                {!elevated && (
                  <button onClick={requestRelaunch} className="ml-1 font-medium underline">
                    以管理员身份重新启动
                  </button>
                )}
              </Note>
            )}
          </>
        )}
      </div>
      <div className="flex flex-col gap-2.5 border-t border-line px-5 pt-3.5 pb-[18px]">
        <div className="flex gap-2">
          <Button variant="outline" size="lg" className="flex-1" disabled={!detail.actionable} onClick={() => onClose(detail)}>
            <Power />
            关闭进程
          </Button>
          <Button variant="danger" size="lg" className="flex-1" disabled={!detail.actionable} onClick={() => onKill(detail)}>
            <Zap />
            强制结束
          </Button>
        </div>
        <div className="flex items-center gap-1.5 text-[11px] text-fg-3">
          {blocked ? <Lock className="size-3" /> : <ShieldCheck className="size-3" />}
          {blocked ? "系统进程已受保护，关闭与强制结束不可用" : "操作前会校验 PID 与启动时间，防止误杀复用 PID 的新进程"}
        </div>
      </div>
    </aside>
  );
}
