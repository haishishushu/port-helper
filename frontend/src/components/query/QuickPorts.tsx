import { cn } from "@/lib/utils";

/** 常用端口：自动记录最近查询的端口，最近的在前。 */
export function QuickPorts({ ports, onPick, onClear }: { ports: number[]; onPick: (p: number) => void; onClear: () => void }) {
  return (
    <div className="flex w-[620px] flex-col gap-2.5">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs text-fg-3">
          <span>常用端口</span>
          <span>· 按最近查询排序，最近的在前</span>
        </div>
        {ports.length > 0 && (
          <button onClick={onClear} className="text-xs text-fg-2 hover:text-fg">
            清空
          </button>
        )}
      </div>
      {ports.length === 0 ? (
        <div className="rounded-lg border border-dashed border-line px-3 py-2.5 text-xs text-fg-3">
          还没有查询记录，查询过的端口会自动出现在这里
        </div>
      ) : (
        <div className="grid grid-cols-10 gap-1.5">
          {ports.map((p, i) => (
            <button
              key={p}
              onClick={() => onPick(p)}
              className={cn(
                "rounded-lg py-1.5 font-mono text-xs text-fg-2 transition-colors hover:bg-surface-hover hover:text-fg",
                i === 0 ? "border border-line-strong bg-surface" : "bg-surface-muted",
              )}
            >
              {p}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
