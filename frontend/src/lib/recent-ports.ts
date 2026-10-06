export const MAX_RECENT_PORTS = 10;

/** 把刚查询的端口移到最前，去重并截断（最近查询的在前）。 */
export function pushRecentPort(list: number[], port: number, max = MAX_RECENT_PORTS): number[] {
  return [port, ...list.filter((p) => p !== port)].slice(0, max);
}
