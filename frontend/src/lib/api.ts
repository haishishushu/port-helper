import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "@/bindings/AppError";
import type { CloseResult } from "@/bindings/CloseResult";
import type { KillResult } from "@/bindings/KillResult";
import type { PidQueryResult } from "@/bindings/PidQueryResult";
import type { PortQueryResult } from "@/bindings/PortQueryResult";
import type { PrivilegeInfo } from "@/bindings/PrivilegeInfo";

/** 把 invoke 抛出的任意错误统一为 AppError。 */
export function toAppError(err: unknown): AppError {
  if (err && typeof err === "object" && "code" in err && "message" in err) {
    return err as AppError;
  }
  return { code: "INTERNAL", message: typeof err === "string" ? err : String(err) };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw toAppError(err);
  }
}

export const api = {
  queryPort: (port: number, includeAllStates: boolean) =>
    call<PortQueryResult>("query_port", { port, includeAllStates }),
  queryPid: (pid: number, includeAllStates: boolean) =>
    call<PidQueryResult>("query_pid", { pid, includeAllStates }),
  closeProcess: (pid: number, startTime: number, timeoutMs: number, confirmCritical: boolean) =>
    call<CloseResult>("close_process", { pid, startTime, timeoutMs, confirmCritical }),
  killProcess: (pid: number, startTime: number, confirmCritical: boolean) =>
    call<KillResult>("kill_process", { pid, startTime, confirmCritical }),
  getPrivilege: () => call<PrivilegeInfo>("get_privilege"),
  relaunchAsAdmin: () => call<void>("relaunch_as_admin"),
  trayMenuReady: () => call<void>("tray_menu_ready"),
  trayOpenMain: () => call<void>("tray_open_main"),
  trayHideMenu: () => call<void>("tray_hide_menu"),
  quitApp: () => call<void>("quit_app"),
};
