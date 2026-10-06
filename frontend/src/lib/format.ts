import type { HiddenState } from "@/bindings/HiddenState";
import type { SocketBinding } from "@/bindings/SocketBinding";
import type { SocketState } from "@/bindings/SocketState";

const STATE_LABEL: Record<SocketState, string> = {
  closed: "CLOSED",
  listen: "LISTEN",
  synSent: "SYN_SENT",
  synReceived: "SYN_RECEIVED",
  established: "ESTABLISHED",
  finWait1: "FIN_WAIT_1",
  finWait2: "FIN_WAIT_2",
  closeWait: "CLOSE_WAIT",
  closing: "CLOSING",
  lastAck: "LAST_ACK",
  timeWait: "TIME_WAIT",
  deleteTcb: "DELETE_TCB",
  unknown: "UNKNOWN",
};

export const stateLabel = (s: SocketState) => STATE_LABEL[s];

export function endpoint(address: string, port: number, ipv6: boolean) {
  return ipv6 ? `[${address}]:${port}` : `${address}:${port}`;
}

export function localEndpoint(b: SocketBinding) {
  return endpoint(b.localAddress, b.localPort, b.family === "ipv6");
}

export function remoteEndpoint(b: SocketBinding) {
  if (b.remoteAddress == null || b.remotePort == null) return null;
  return endpoint(b.remoteAddress, b.remotePort, b.family === "ipv6");
}

export function hiddenSummary(hidden: HiddenState[]) {
  const total = hidden.reduce((n, h) => n + h.count, 0);
  const detail = hidden.map((h) => `${stateLabel(h.state)} ×${h.count}`).join(" · ");
  return { total, detail };
}

const pad = (n: number) => String(n).padStart(2, "0");

export function formatDateTime(ms: number) {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function formatUptime(startMs: number, now = Date.now()) {
  const minutes = Math.max(0, Math.floor((now - startMs) / 60000));
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const mins = minutes % 60;
  if (days > 0) return `已运行 ${days} 天 ${hours} 小时`;
  if (hours > 0) return `已运行 ${hours} 小时 ${mins} 分`;
  return `已运行 ${mins} 分`;
}
