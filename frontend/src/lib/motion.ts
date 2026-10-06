import { useEffect, useRef, useState } from "react";

/**
 * 刷新动效节奏（思路参考 haishishushu/cc-usage 的整卡柔光）：
 * - 柔光掠过整卡一轮 1100ms，与 index.css 的 refresh-shimmer 保持一致；
 * - 刷新图标转一圈 700ms，与 index.css 的 refresh-spin 保持一致。
 */
export const SHIMMER_CYCLE_MS = 1100;
/** 端口表查询通常几十毫秒就返回，只掠一轮容易没看清；两轮分量够又不拖沓 */
export const SHIMMER_MIN_CYCLES = 2;
export const SPIN_CYCLE_MS = 700;
export const SPIN_MIN_CYCLES = 1;

/**
 * 还要多久才能收尾：对齐到轮次边界，且总时长不少于 `minCycles` 轮。
 * 轮次边界处柔光正好在卡外且透明、图标正好转满整圈，此时移除不会有跳变。
 */
export function cycleExitDelay(elapsedMs: number, cycleMs: number, minCycles = 1): number {
  const toBoundary = cycleMs - (elapsedMs % cycleMs);
  const minTotal = cycleMs * minCycles;
  return elapsedMs + toBoundary >= minTotal ? toBoundary : minTotal - elapsedMs;
}

/**
 * 循环反馈动画的开关：`startKey` 变化时起跑，`active` 落下后补齐到轮次边界再停。
 *
 * - 起跑看序号变化而不是 `active`：查询秒回时 loading 的 true→false 可能被 React
 *   合并到同一批更新里，只看 `active` 会让整段动画丢失。
 * - 动画进行中再次刷新不重置起点：CSS 动画没有重启，重置会算错轮次边界。
 * - 用计时器而不是 animationiteration 事件：减少动态效果时动画被关闭，事件永远不会触发。
 */
export function useCycleExit(startKey: number, active: boolean, cycleMs: number, minCycles = 1): boolean {
  const [running, setRunning] = useState(false);
  const startedAt = useRef(0);
  const previousKey = useRef(startKey);

  useEffect(() => {
    if (previousKey.current === startKey) return;
    previousKey.current = startKey;
    if (startedAt.current === 0) startedAt.current = performance.now();
    setRunning(true);
  }, [startKey]);

  useEffect(() => {
    if (active || !running) return;
    const timer = window.setTimeout(
      () => {
        startedAt.current = 0;
        setRunning(false);
      },
      cycleExitDelay(performance.now() - startedAt.current, cycleMs, minCycles),
    );
    return () => window.clearTimeout(timer);
  }, [active, running, cycleMs, minCycles]);

  return running;
}
