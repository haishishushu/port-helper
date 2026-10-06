import { lazy, Suspense, useState } from "react";
import { Sidebar, type View } from "@/components/layout/Sidebar";
import { Toaster } from "@/components/ui/sonner";
import { PrivilegeProvider } from "@/hooks/use-privilege";
import { RelaunchProvider } from "@/hooks/use-relaunch";
import { SettingsProvider, useSettings } from "@/hooks/use-settings";
import { cn } from "@/lib/utils";
import type { PidRequest } from "@/pages/PidQueryPage";
import { PortQueryPage, type PortRequest } from "@/pages/PortQueryPage";

// 进程反查页、设置页首次访问时才加载并挂载（perf-todo P2-3）
const PidQueryPage = lazy(() => import("@/pages/PidQueryPage").then((m) => ({ default: m.PidQueryPage })));
const SettingsPage = lazy(() => import("@/pages/SettingsPage").then((m) => ({ default: m.SettingsPage })));

function Shell() {
  const { resolvedTheme } = useSettings();
  const [view, setView] = useState<View>("port");
  /** 访问过的页面保持挂载，切换时保留各自的查询状态 */
  const [visited, setVisited] = useState<Set<View>>(() => new Set(["port"]));
  const navigate = (v: View) => {
    setVisited((s) => (s.has(v) ? s : new Set(s).add(v)));
    setView(v);
  };
  const [portRequest, setPortRequest] = useState<PortRequest | null>(null);
  const [pidRequest, setPidRequest] = useState<PidRequest | null>(null);

  const page = (v: View) => cn("flex min-w-0 flex-1", view !== v && "hidden");

  return (
    <div className="flex h-full">
      <Sidebar view={view} onNavigate={navigate} />
      <main className={page("port")}>
        <PortQueryPage
          active={view === "port"}
          request={portRequest}
          onShowAllPorts={(pid) => {
            setPidRequest({ pid, nonce: Date.now() });
            navigate("pid");
          }}
        />
      </main>
      {/* 每个页面各自的加载边界：一个页面加载时不会让另一个已显示的页面被隐藏、重跑 effect */}
      {visited.has("pid") && (
        <Suspense fallback={null}>
          <main className={page("pid")}>
            <PidQueryPage
              active={view === "pid"}
              request={pidRequest}
              onGotoPort={(port) => {
                if (port != null) setPortRequest({ port, nonce: Date.now() });
                navigate("port");
              }}
            />
          </main>
        </Suspense>
      )}
      {visited.has("settings") && (
        <Suspense fallback={null}>
          <main className={page("settings")}>
            <SettingsPage />
          </main>
        </Suspense>
      )}
      <Toaster theme={resolvedTheme} />
    </div>
  );
}

export default function App() {
  return (
    <SettingsProvider>
      <PrivilegeProvider>
        <RelaunchProvider>
          <Shell />
        </RelaunchProvider>
      </PrivilegeProvider>
    </SettingsProvider>
  );
}
