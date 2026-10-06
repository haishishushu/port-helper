/** 端口号：1–65535 的整数，返回 null 表示非法。 */
export function parsePort(input: string): number | null {
  const text = input.trim();
  if (!/^\d{1,5}$/.test(text)) return null;
  const port = Number(text);
  return port >= 1 && port <= 65535 ? port : null;
}

/** PID：非负整数（Windows PID 为 32 位无符号整数）。 */
export function parsePid(input: string): number | null {
  const text = input.trim();
  if (!/^\d{1,10}$/.test(text)) return null;
  const pid = Number(text);
  return pid <= 0xffffffff ? pid : null;
}
