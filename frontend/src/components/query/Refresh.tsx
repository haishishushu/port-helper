import { RotateCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SHIMMER_CYCLE_MS, SHIMMER_MIN_CYCLES, SPIN_CYCLE_MS, SPIN_MIN_CYCLES, useCycleExit } from "@/lib/motion";
import { cn } from "@/lib/utils";

export interface RefreshState {
  /** 刷新序号：每次用户主动刷新 +1，变化即起跑动画 */
  refreshKey: number;
  /** 查询进行中 */
  refreshing: boolean;
}

/**
 * 整卡柔光：放在 `relative` 且带圆角的卡片里。
 * 不带 key，整个生命周期只挂载一次，由 useCycleExit 在轮次边界摘除，避免重挂载导致的“卡一下”。
 */
export function RefreshVeil({ refreshKey, refreshing }: RefreshState) {
  const shimmering = useCycleExit(refreshKey, refreshing, SHIMMER_CYCLE_MS, SHIMMER_MIN_CYCLES);
  if (!shimmering) return null;
  return (
    <span aria-hidden className="refresh-veil">
      <span className="refresh-shimmer" />
    </span>
  );
}

/** 刷新按钮：图标按整圈旋转，查询结束后转满当前这一圈再停；查询中禁止重复触发。 */
export function RefreshButton({ refreshKey, refreshing, onRefresh }: RefreshState & { onRefresh: () => void }) {
  const spinning = useCycleExit(refreshKey, refreshing, SPIN_CYCLE_MS, SPIN_MIN_CYCLES);
  return (
    <Button
      variant="ghost"
      onClick={onRefresh}
      disabled={refreshing}
      title="刷新（F5）"
      aria-busy={spinning}
      className="disabled:opacity-100"
    >
      {/* 文字保持不变，避免宽度变化挤压左侧摘要 */}
      <RotateCw className={cn(spinning && "refresh-spin")} />
      刷新
    </Button>
  );
}
