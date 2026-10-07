import { ArrowUpRight, PlugZap, ScanSearch, Search, Settings, Shield, ShieldCheck, type LucideIcon } from "lucide-react";
import { usePrivilege } from "@/hooks/use-privilege";
import { useRelaunchAdmin } from "@/hooks/use-relaunch";
import { cn } from "@/lib/utils";
import { platform } from "@/lib/platform";

export type View = "port" | "pid" | "settings";

function NavItem({ icon: Icon, label, active, onClick }: { icon: LucideIcon; label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-[7px] text-left text-[13.5px] text-fg transition-colors",
        active ? "bg-surface-hover" : "hover:bg-surface-hover/60",
      )}
    >
      <Icon className="size-4 text-fg-2" />
      {label}
    </button>
  );
}

/** 运行权限：单行低调展示。普通权限时右侧“提升”打开“以管理员身份重新启动”确认框。 */
function PermissionRow() {
  const { elevated } = usePrivilege();
  const requestRelaunch = useRelaunchAdmin();
  return (
    <div className="flex items-center gap-2 px-2.5 py-[7px]">
      {elevated ? <ShieldCheck className="size-3.5 shrink-0 text-success" /> : <Shield className="size-3.5 shrink-0 text-fg-3" />}
      <span className="flex-1 truncate text-xs text-fg-2">{elevated ? `${platform.admin}权限运行` : "普通权限运行"}</span>
      {!elevated && (
        <button
          onClick={requestRelaunch}
          title={`以${platform.admin}身份重新启动`}
          className="flex shrink-0 items-center gap-0.5 text-xs text-fg-3 transition-colors hover:text-fg"
        >
          <ArrowUpRight className="size-[11px]" />
          提升
        </button>
      )}
    </div>
  );
}

export function Sidebar({ view, onNavigate }: { view: View; onNavigate: (v: View) => void }) {
  return (
    <aside className="flex w-[232px] shrink-0 flex-col gap-0.5 border-r border-line bg-sidebar px-2.5 pt-3.5 pb-3">
      <div data-tauri-drag-region className="flex items-center gap-[9px] px-2 pt-1 pb-4">
        <span className="pointer-events-none flex size-6 items-center justify-center rounded-[7px] bg-ink">
          <PlugZap className="size-3.5 text-on-ink" />
        </span>
        <span className="pointer-events-none text-sm font-semibold">port-helper</span>
      </div>
      <NavItem icon={Search} label="端口查询" active={view === "port"} onClick={() => onNavigate("port")} />
      <NavItem icon={ScanSearch} label="进程反查" active={view === "pid"} onClick={() => onNavigate("pid")} />
      <div data-tauri-drag-region className="flex-1" />
      <PermissionRow />
      <NavItem icon={Settings} label="设置" active={view === "settings"} onClick={() => onNavigate("settings")} />
    </aside>
  );
}
