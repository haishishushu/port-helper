import { Store } from "@tauri-apps/plugin-store";

export type ThemeMode = "light" | "dark" | "system";

export interface Settings {
  theme: ThemeMode;
  /** 默认只显示 LISTEN 状态 */
  listenOnly: boolean;
  /** 正常关闭的等待时长（秒） */
  closeTimeoutSec: number;
  /** 最近查询的端口，最近的在前 */
  recentPorts: number[];
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  listenOnly: true,
  closeTimeoutSec: 5,
  recentPorts: [],
};

export const CLOSE_TIMEOUT_RANGE = { min: 1, max: 60 } as const;

const STORE_FILE = "settings.json";
let storePromise: Promise<Store | null> | null = null;

/** 非 Tauri 环境（如在普通浏览器里调试界面）时返回 null，设置只保存在内存中。 */
function getStore() {
  storePromise ??= Store.load(STORE_FILE, { defaults: { ...DEFAULT_SETTINGS }, autoSave: 200 }).catch(() => null);
  return storePromise;
}

export async function loadSettings(): Promise<Settings> {
  const store = await getStore();
  if (!store) return { ...DEFAULT_SETTINGS };
  const entries = await Promise.all(
    (Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]).map(async (k) => [k, await store.get(k)] as const),
  );
  const loaded = { ...DEFAULT_SETTINGS };
  for (const [k, v] of entries) {
    if (v !== undefined && v !== null) (loaded as Record<string, unknown>)[k] = v;
  }
  return sanitize(loaded);
}

export async function saveSettings(patch: Partial<Settings>) {
  const store = await getStore();
  if (!store) return;
  await Promise.all(Object.entries(patch).map(([k, v]) => store.set(k, v)));
}

function sanitize(s: Settings): Settings {
  const theme: ThemeMode = ["light", "dark", "system"].includes(s.theme) ? s.theme : "system";
  const timeout = Math.round(Number(s.closeTimeoutSec));
  return {
    theme,
    listenOnly: Boolean(s.listenOnly),
    closeTimeoutSec: Number.isFinite(timeout)
      ? Math.min(CLOSE_TIMEOUT_RANGE.max, Math.max(CLOSE_TIMEOUT_RANGE.min, timeout))
      : DEFAULT_SETTINGS.closeTimeoutSec,
    recentPorts: Array.isArray(s.recentPorts)
      ? s.recentPorts.filter((p) => Number.isInteger(p) && p >= 1 && p <= 65535)
      : [],
  };
}
