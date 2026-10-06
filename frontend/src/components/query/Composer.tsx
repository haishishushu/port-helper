import { ChevronDown, CircleAlert, CircleX, Cpu, Globe, Hash, Layers, Radio, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/** 外部写入输入框的值：nonce 变化时覆盖内部输入（快捷端口、页面跳转、返回主页） */
export interface ComposerSeed {
  value: string;
  nonce: number;
}

interface ComposerProps {
  variant: "hero" | "compact";
  mode: "port" | "pid";
  seed: ComposerSeed;
  onSubmit: (raw: string) => void;
  error?: string | null;
  /** 有错误时用户重新输入，通知页面清除错误 */
  onErrorDismiss?: () => void;
  includeAll: boolean;
  onToggleIncludeAll: () => void;
  loading?: boolean;
}

function StateChip({ includeAll, onClick, compact }: { includeAll: boolean; onClick: () => void; compact?: boolean }) {
  return (
    <button
      onClick={onClick}
      title="切换是否显示 ESTABLISHED、TIME_WAIT 等非监听记录"
      className={cn(
        "flex items-center gap-1.5 rounded-lg text-xs text-fg-2 transition-colors hover:text-fg",
        compact ? "bg-surface-muted px-2 py-1" : "border border-line px-[9px] py-[5px] hover:bg-surface-hover",
        includeAll && "text-fg",
      )}
    >
      {!compact && <Radio className="size-[13px]" />}
      {includeAll ? "全部状态" : "仅 LISTEN"}
      {!compact && <ChevronDown className="size-3 text-fg-3" />}
    </button>
  );
}

function InfoChip({ icon: Icon, label, compact }: { icon: typeof Layers; label: string; compact?: boolean }) {
  return (
    <span
      className={cn(
        "flex items-center gap-1.5 rounded-lg text-xs text-fg-2",
        compact ? "bg-surface-muted px-2 py-1" : "border border-line px-[9px] py-[5px]",
      )}
    >
      {!compact && <Icon className="size-[13px]" />}
      {label}
    </span>
  );
}

/**
 * 查询输入框：主页为大号（hero），有结果后收起为紧凑样式（compact）。
 * 输入值保存在组件内部，打字时只重渲染输入框本身，不会牵动结果列表和详情面板（perf-todo P2-4）。
 */
export function Composer(p: ComposerProps) {
  const [value, setValue] = useState(p.seed.value);
  // 只在写入序号变化时覆盖输入：effect 可能因 Suspense 显隐被重新执行，不能因此清掉用户的输入
  const appliedNonce = useRef(p.seed.nonce);
  useEffect(() => {
    if (appliedNonce.current === p.seed.nonce) return;
    appliedNonce.current = p.seed.nonce;
    setValue(p.seed.value);
  }, [p.seed.nonce, p.seed.value]);
  const change = (v: string) => {
    setValue(v);
    if (p.error) p.onErrorDismiss?.();
  };
  const submit = () => p.onSubmit(value);
  const Icon = p.mode === "port" ? Hash : Cpu;
  const placeholder = p.mode === "port" ? "输入端口号，例如 5173" : "输入 PID，例如 18244";
  const maxLength = p.mode === "port" ? 5 : 10;
  const input = (
    <input
      autoFocus
      value={value}
      maxLength={maxLength}
      inputMode="numeric"
      placeholder={placeholder}
      spellCheck={false}
      onChange={(e) => change(e.target.value.replace(/\D/g, ""))}
      onKeyDown={(e) => e.key === "Enter" && submit()}
      className={cn(
        "selectable min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:font-sans placeholder:text-fg-3",
        p.variant === "hero" ? "text-base" : "font-mono text-[15px] font-medium",
        p.variant === "hero" && value && "font-mono",
      )}
    />
  );
  const clear = value && (
    <button aria-label="清空输入" onClick={() => change("")} className="text-fg-3 hover:text-fg">
      <CircleX className="size-4" />
    </button>
  );
  const queryButton = (
    <Button onClick={submit} disabled={p.loading}>
      <Search />
      查询
    </Button>
  );
  const errorText = p.error && (
    <div className="flex items-center gap-1.5 px-1 text-xs text-danger">
      <CircleAlert className="size-[13px]" />
      {p.error}
    </div>
  );

  if (p.variant === "compact") {
    return (
      <div className="flex flex-col gap-1.5">
        <div
          className={cn(
            "flex items-center gap-2.5 rounded-xl border bg-surface py-1.5 pr-1.5 pl-3.5",
            p.error ? "border-danger" : "border-line-strong",
          )}
        >
          <Icon className="size-4 text-fg-3" />
          {input}
          {clear}
          <InfoChip icon={Layers} label="TCP + UDP" compact />
          <InfoChip icon={Globe} label="IPv4 + IPv6" compact />
          <StateChip includeAll={p.includeAll} onClick={p.onToggleIncludeAll} compact />
          {queryButton}
        </div>
        {errorText}
      </div>
    );
  }

  return (
    <div className="flex w-[620px] flex-col gap-2">
      <div
        className={cn(
          "flex flex-col gap-3.5 rounded-2xl border bg-surface pt-4 pr-3 pb-2.5 pl-[18px] shadow-[0_6px_24px_rgba(0,0,0,0.06)]",
          p.error ? "border-danger" : "border-line-strong",
        )}
      >
        <div className="flex items-center gap-2.5">
          <Icon className="size-[18px] text-fg-3" />
          {input}
          {clear}
        </div>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <InfoChip icon={Layers} label="TCP + UDP" />
            <InfoChip icon={Globe} label="IPv4 + IPv6" />
            <StateChip includeAll={p.includeAll} onClick={p.onToggleIncludeAll} />
          </div>
          <div className="flex items-center gap-2.5">
            <span className="text-xs text-fg-3">Enter 查询</span>
            {queryButton}
          </div>
        </div>
      </div>
      {errorText}
    </div>
  );
}
