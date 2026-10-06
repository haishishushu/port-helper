import { ChevronRight } from "lucide-react";
import type { ProcessSummary } from "@/bindings/ProcessSummary";
import type { SocketBinding } from "@/bindings/SocketBinding";
import type { SocketState } from "@/bindings/SocketState";
import { AppIconView } from "@/components/query/AppIconView";
import { memo } from "react";
import { RefreshVeil } from "@/components/query/Refresh";
import { Tag, type TagTone } from "@/components/query/Tag";
import { localEndpoint, remoteEndpoint, stateLabel } from "@/lib/format";
import { cn } from "@/lib/utils";

const STATE_TONE: Partial<Record<SocketState, TagTone>> = { listen: "success", established: "info" };

export function BindingRow({ b }: { b: SocketBinding }) {
  const remote = remoteEndpoint(b);
  return (
    <div className="flex items-center gap-2 border-t border-line px-4 py-2.5">
      <Tag>{b.protocol.toUpperCase()}</Tag>
      <Tag tone={b.family === "ipv6" ? "info" : "neutral"}>{b.family === "ipv6" ? "IPv6" : "IPv4"}</Tag>
      <span className="selectable truncate font-mono text-[13px]">
        {localEndpoint(b)}
        {remote && <span className="text-fg-3"> ↔ {remote}</span>}
      </span>
      <span className="flex-1" />
      {b.state && <Tag tone={STATE_TONE[b.state] ?? "neutral"}>{stateLabel(b.state)}</Tag>}
    </div>
  );
}

export function KindTag({ process }: { process: ProcessSummary }) {
  if (!process.actionable) return <Tag tone="warning" mono={false}>不可结束</Tag>;
  switch (process.kind) {
    case "critical":
      return <Tag tone="danger" mono={false}>关键进程</Tag>;
    case "forwarder":
      return <Tag tone="warning" mono={false}>端口转发</Tag>;
    case "service":
      return <Tag mono={false}>Windows 服务</Tag>;
    default:
      return null;
  }
}

/** 结果卡片。props 都是稳定引用或基本类型，memo 后选中切换只重渲染变化的两张卡片。 */
export const ProcessCard = memo(function ProcessCard({ process, bindings, selected, onSelect, refreshKey, refreshing }: {
  process: ProcessSummary;
  bindings: SocketBinding[];
  selected: boolean;
  /** 传入稳定的回调（如 setState），卡片自行带上 PID */
  onSelect: (pid: number) => void;
  refreshKey: number;
  refreshing: boolean;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onSelect(process.pid)}
      onKeyDown={(e) => e.key === "Enter" && onSelect(process.pid)}
      className={cn(
        "relative flex cursor-pointer flex-col rounded-xl bg-surface outline-none",
        selected ? "border-[1.5px] border-ink" : "border border-line hover:border-line-strong",
      )}
    >
      <div className="flex items-center gap-3 px-4 py-3.5">
        <AppIconView icon={process.appIcon} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-semibold">{process.appName}</span>
          <span className="truncate font-mono text-xs text-fg-2">
            {process.name} · PID {process.pid}
          </span>
        </div>
        <KindTag process={process} />
        <ChevronRight className="size-4 text-fg-3" />
      </div>
      {bindings.map((b, i) => (
        <BindingRow key={i} b={b} />
      ))}
      <RefreshVeil refreshKey={refreshKey} refreshing={refreshing} />
    </div>
  );
});
