import { createContext, use, useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { DEFAULT_SETTINGS, loadSettings, saveSettings, type Settings } from "@/lib/settings";

interface SettingsContextValue {
  settings: Settings;
  loaded: boolean;
  /** 实际生效的主题（system 已解析为 light / dark） */
  resolvedTheme: "light" | "dark";
  update: (patch: Partial<Settings>) => void;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

const darkQuery = () => window.matchMedia("(prefers-color-scheme: dark)");

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(false);
  const [systemDark, setSystemDark] = useState(() => darkQuery().matches);

  useEffect(() => {
    loadSettings().then((s) => {
      setSettings(s);
      setLoaded(true);
    });
  }, []);

  useEffect(() => {
    const mq = darkQuery();
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const resolvedTheme = settings.theme === "system" ? (systemDark ? "dark" : "light") : settings.theme;

  useEffect(() => {
    document.documentElement.classList.toggle("dark", resolvedTheme === "dark");
    document.documentElement.style.colorScheme = resolvedTheme;
  }, [resolvedTheme]);

  const update = useCallback((patch: Partial<Settings>) => {
    setSettings((prev) => ({ ...prev, ...patch }));
    void saveSettings(patch);
  }, []);

  const value = useMemo(() => ({ settings, loaded, resolvedTheme, update }), [settings, loaded, resolvedTheme, update]);
  return <SettingsContext value={value}>{children}</SettingsContext>;
}

export function useSettings() {
  const ctx = use(SettingsContext);
  if (!ctx) throw new Error("useSettings 必须在 SettingsProvider 内使用");
  return ctx;
}
