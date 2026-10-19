import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createContext, lazy, Suspense, use, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { UpdateCheck } from "@/bindings/UpdateCheck";
import type { UpdateProgress } from "@/bindings/UpdateProgress";
import { useSettings } from "@/hooks/use-settings";
import { api, toAppError } from "@/lib/api";

const UpdateDialog = lazy(() => import("@/components/dialogs/UpdateDialog").then((m) => ({ default: m.UpdateDialog })));

/** 与后端 updater.rs 的 PROGRESS_EVENT 一致 */
const PROGRESS_EVENT = "update-download-progress";
/** 启动后延迟检查，避开首屏渲染与首次查询 */
const STARTUP_CHECK_DELAY_MS = 1000;

export type UpdatePhase = "idle" | "checking" | "downloading" | "downloadError" | "downloaded" | "installing";

interface UpdateContextValue {
  phase: UpdatePhase;
  /** 最近一次检查到的新版本；没有新版本时为 null */
  info: UpdateCheck | null;
  progress: UpdateProgress;
  error: string | null;
  /** 侧边栏左上角是否显示更新按钮：有新版本且没被跳过，或已开始更新 */
  badgeVisible: boolean;
  /** 手动检查，返回是否有新版本；失败时抛出 AppError，由调用方提示 */
  checkNow: () => Promise<boolean>;
  /** 点击任一更新入口：未下载时开始下载，已在更新流程中则重新打开进度框 */
  openUpdate: () => void;
  install: () => void;
  /** 跳过此版本：同版本不再在左上角提示 */
  skip: () => void;
}

const UpdateContext = createContext<UpdateContextValue | null>(null);

/** 更新流程中的阶段（进度框可打开） */
export const inFlow = (phase: UpdatePhase) => phase !== "idle" && phase !== "checking";

/** 下载进度 0–100；总量未知时返回 null，界面显示不定长进度 */
export function progressPercent({ downloaded, total }: UpdateProgress): number | null {
  if (!total) return null;
  return Math.min(100, Math.max(0, Math.round((downloaded / total) * 100)));
}

/** 两个更新入口共用的状态文案 */
export function phaseLabel(phase: UpdatePhase, progress: UpdateProgress): string {
  switch (phase) {
    case "downloading": {
      const pct = progressPercent(progress);
      return pct === null ? "下载中" : `下载中 ${pct}%`;
    }
    case "downloadError":
      return "下载失败";
    case "downloaded":
      return "安装更新";
    case "installing":
      return "正在安装";
    default:
      return "";
  }
}

/**
 * 应用更新状态。侧边栏左上角的更新按钮与设置页的“检查更新”共用这一份状态：
 * 启动 1 秒后静默检查一次，发现新版本后点任一入口即下载并校验，下载完成后在进度框里点一次“安装并重启”。
 */
export function UpdateProvider({ children }: { children: ReactNode }) {
  const { settings, update } = useSettings();
  const [phase, setPhase] = useState<UpdatePhase>("idle");
  const [info, setInfo] = useState<UpdateCheck | null>(null);
  const [progress, setProgress] = useState<UpdateProgress>({ downloaded: 0, total: null });
  const [error, setError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  /** 检查、下载、安装互斥，避免重复点击发出并发请求 */
  const busy = useRef(false);

  const checkNow = useCallback(async () => {
    if (busy.current) return info !== null;
    busy.current = true;
    setPhase("checking");
    try {
      const result = await api.checkUpdate();
      setInfo(result.available ? result : null);
      return result.available;
    } finally {
      busy.current = false;
      setPhase("idle");
    }
  }, [info]);

  const download = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setPhase("downloading");
    setProgress({ downloaded: 0, total: null });
    setError(null);
    let unlisten: UnlistenFn | undefined;
    try {
      unlisten = await listen<UpdateProgress>(PROGRESS_EVENT, (e) => setProgress(e.payload));
      setInfo(await api.downloadUpdate());
      setPhase("downloaded");
    } catch (err) {
      setError(toAppError(err).message);
      setPhase("downloadError");
    } finally {
      unlisten?.();
      busy.current = false;
    }
  }, []);

  const openUpdate = useCallback(() => {
    if (phase === "checking") return;
    setDialogOpen(true);
    if (phase === "idle" || phase === "downloadError") void download();
  }, [phase, download]);

  const install = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setError(null);
    setPhase("installing");
    try {
      // 成功时应用会退出并由安装器重新打开，不会返回
      await api.installUpdate();
    } catch (err) {
      setError(toAppError(err).message);
      setPhase("downloaded");
    } finally {
      busy.current = false;
    }
  }, []);

  const skip = useCallback(() => {
    if (info) update({ skippedVersion: info.availableVersion });
  }, [info, update]);

  useEffect(() => {
    if (!isTauri()) return;
    // 启动检查失败（离线、GitHub 不通）不打扰，下次启动或手动检查时再试
    const timer = window.setTimeout(() => void checkNow().catch(() => {}), STARTUP_CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
    // 只在启动时检查一次
  }, []);

  const badgeVisible = inFlow(phase) || (info !== null && info.availableVersion !== settings.skippedVersion);

  const value = useMemo<UpdateContextValue>(
    () => ({ phase, info, progress, error, badgeVisible, checkNow, openUpdate, install: () => void install(), skip }),
    [phase, info, progress, error, badgeVisible, checkNow, openUpdate, install, skip],
  );

  return (
    <UpdateContext value={value}>
      {children}
      {dialogOpen && info && (
        <Suspense fallback={null}>
          <UpdateDialog onClose={() => setDialogOpen(false)} />
        </Suspense>
      )}
    </UpdateContext>
  );
}

export function useUpdate() {
  const ctx = use(UpdateContext);
  if (!ctx) throw new Error("useUpdate 必须在 UpdateProvider 内使用");
  return ctx;
}
