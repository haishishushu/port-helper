import { createContext, lazy, Suspense, use, useState, type ReactNode } from "react";

const RelaunchDialog = lazy(() => import("@/components/dialogs/RelaunchDialog").then((m) => ({ default: m.RelaunchDialog })));

const RelaunchContext = createContext<() => void>(() => {});

/** 全局“以管理员身份重新启动”确认框，任何位置（Toast、侧边栏、详情面板、设置页）都可触发。 */
export function RelaunchProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <RelaunchContext value={() => setOpen(true)}>
      {children}
      {open && (
        <Suspense fallback={null}>
          <RelaunchDialog onClose={() => setOpen(false)} />
        </Suspense>
      )}
    </RelaunchContext>
  );
}

export const useRelaunchAdmin = () => use(RelaunchContext);
