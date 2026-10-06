import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { Check, Minus, Plus, Shield, Trash2 } from "lucide-react";
import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import { TitleBar } from "@/components/layout/TitleBar";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { usePrivilege } from "@/hooks/use-privilege";
import { useRelaunchAdmin } from "@/hooks/use-relaunch";
import { useSettings } from "@/hooks/use-settings";
import { notify } from "@/lib/notify";
import { CLOSE_TIMEOUT_RANGE, type ThemeMode } from "@/lib/settings";
import { cn } from "@/lib/utils";

const ClearRecentDialog = lazy(() =>
  import("@/components/dialogs/ClearRecentDialog").then((m) => ({ default: m.ClearRecentDialog })),
);

const PALETTE = {
  light: { tray: "#ECECEA", bg: "#FFFFFF", side: "#F7F7F6", line: "#E4E4E1", act: "#E9E9E7", comp: "#FFFFFF", compLine: "#D5D5D2", ink: "#111111", stroke: "#E2E2DF" },
  dark: { tray: "#2A2A2A", bg: "#181818", side: "#111111", line: "#2B2B2B", act: "#2A2A2A", comp: "#1F1F1F", compLine: "#3A3A3A", ink: "#EDEDED", stroke: "#2E2E2E" },
};

/** 主题缩略图：托盘 + 从左上角伸出的迷你窗口（与设计稿 07 一致）。 */
function MiniWindow({ mode }: { mode: "light" | "dark" }) {
  const c = PALETTE[mode];
  return (
    <div
      className="absolute top-3.5 left-4 flex h-20 w-[calc(100%-16px)] overflow-hidden rounded-tl-[7px] border shadow-[0_2px_8px_rgba(0,0,0,0.1)]"
      style={{ background: c.bg, borderColor: c.stroke }}
    >
      <div className="flex w-11 flex-col gap-[5px] px-[7px] py-2" style={{ background: c.side }}>
        <span className="size-2 rounded-[2px]" style={{ background: c.ink }} />
        <span className="h-[7px] rounded-[3px]" style={{ background: c.act }} />
        <span className="h-[3px] w-[22px] rounded-sm" style={{ background: c.line }} />
      </div>
      <div className="flex flex-1 flex-col gap-1.5 px-3.5 py-4">
        <span className="h-1 w-[46px] rounded-sm" style={{ background: c.line }} />
        <span className="flex h-4 items-center justify-end rounded-[5px] border px-[3px]" style={{ background: c.comp, borderColor: c.compLine }}>
          <span className="h-2.5 w-3.5 rounded-[3px]" style={{ background: c.ink }} />
        </span>
        <span className="flex gap-[3px]">
          {[0, 1, 2, 3].map((i) => (
            <span key={i} className="h-[5px] w-3 rounded-sm" style={{ background: c.line }} />
          ))}
        </span>
      </div>
    </div>
  );
}

function ThemePreview({ mode }: { mode: ThemeMode }) {
  if (mode !== "system") {
    return (
      <div className="relative h-[84px] overflow-hidden rounded-lg" style={{ background: PALETTE[mode].tray }}>
        <MiniWindow mode={mode} />
      </div>
    );
  }
  return (
    <div className="relative h-[84px] overflow-hidden rounded-lg" style={{ background: `linear-gradient(90deg, ${PALETTE.light.tray} 50%, ${PALETTE.dark.tray} 50%)` }}>
      <MiniWindow mode="light" />
      <div className="absolute inset-y-0 right-0 left-1/2 overflow-hidden">
        <div className="absolute inset-y-0 -left-full right-0">
          <MiniWindow mode="dark" />
        </div>
      </div>
      <span className="absolute inset-y-0 left-1/2 w-px bg-white/25" />
    </div>
  );
}

const THEMES: { mode: ThemeMode; label: string; desc: string }[] = [
  { mode: "light", label: "浅色", desc: "明亮背景" },
  { mode: "dark", label: "深色", desc: "护眼暗色" },
  { mode: "system", label: "跟随系统", desc: "随 Windows 切换" },
];

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <h2 className="text-[13px] font-semibold">{title}</h2>
      {children}
    </section>
  );
}

function Row({ label, desc, children, first }: { label: string; desc?: string; children: ReactNode; first?: boolean }) {
  return (
    <div className={cn("flex items-center gap-4 px-4 py-[13px]", !first && "border-t border-line")}>
      <div className="flex min-w-0 flex-1 flex-col gap-[3px]">
        <span className="text-[13.5px]">{label}</span>
        {desc && <span className="text-xs leading-normal text-fg-3">{desc}</span>}
      </div>
      {children}
    </div>
  );
}

export function SettingsPage() {
  const { settings, update } = useSettings();
  const { elevated } = usePrivilege();
  const requestRelaunch = useRelaunchAdmin();
  const [version, setVersion] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);

  useEffect(() => {
    if (isTauri()) getVersion().then(setVersion).catch(() => {});
  }, []);

  const setTheme = (mode: ThemeMode) => {
    if (mode === settings.theme) return;
    update({ theme: mode });
    notify.success(`已切换为${THEMES.find((t) => t.mode === mode)!.label}主题`);
  };

  const setTimeoutSec = (sec: number) => {
    const next = Math.min(CLOSE_TIMEOUT_RANGE.max, Math.max(CLOSE_TIMEOUT_RANGE.min, sec));
    if (next === settings.closeTimeoutSec) return;
    update({ closeTimeoutSec: next });
    notify.success(`正常关闭等待时长已改为 ${next} 秒`);
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <TitleBar title="设置" />
      <div className="flex flex-1 justify-center overflow-y-auto pt-5 pb-10">
        <div className="flex w-[640px] flex-col gap-5">
          <Section title="外观">
            <div className="grid grid-cols-3 gap-3">
              {THEMES.map((t) => {
                const selected = settings.theme === t.mode;
                return (
                  <button
                    key={t.mode}
                    onClick={() => setTheme(t.mode)}
                    className={cn(
                      "flex flex-col gap-2.5 rounded-xl bg-surface p-1.5 text-left",
                      selected ? "border-[1.5px] border-ink" : "border border-line hover:border-line-strong",
                    )}
                  >
                    <ThemePreview mode={t.mode} />
                    <div className="flex items-center justify-between px-1.5 pb-1">
                      <div className="flex flex-col gap-0.5">
                        <span className={cn("text-[13px]", selected && "font-semibold")}>{t.label}</span>
                        <span className="text-[11.5px] text-fg-3">{t.desc}</span>
                      </div>
                      {selected ? (
                        <span className="flex size-[18px] items-center justify-center rounded-full bg-ink">
                          <Check className="size-[11px] text-on-ink" />
                        </span>
                      ) : (
                        <span className="size-[18px] rounded-full border-[1.5px] border-line-strong" />
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
            <p className="text-xs text-fg-3">选择“跟随系统”时，会随 Windows 的浅色/深色设置自动切换。</p>
          </Section>

          <Section title="查询">
            <div className="rounded-xl border border-line bg-surface">
              <Row first label="默认只显示 LISTEN 状态" desc="关闭后会同时显示 ESTABLISHED、TIME_WAIT 等连接记录">
                <Switch checked={settings.listenOnly} onCheckedChange={(v) => update({ listenOnly: v })} />
              </Row>
              <Row label="常用端口" desc="每次查询后自动加入，最近查询的排在最前；重复端口只保留一个，最多保留 10 个">
                <button
                  disabled={settings.recentPorts.length === 0}
                  onClick={() => setConfirmClear(true)}
                  className="flex items-center gap-1.5 text-[12.5px] text-fg-2 hover:text-fg disabled:opacity-40"
                >
                  <Trash2 className="size-[13px]" />
                  清空（{settings.recentPorts.length}）
                </button>
              </Row>
            </div>
          </Section>

          <Section title="进程操作">
            <div className="rounded-xl border border-line bg-surface">
              <Row first label="正常关闭等待时长" desc="超过该时长进程仍未退出时，提示是否强制结束">
                <div className="flex h-[30px] items-center rounded-lg border border-line-strong">
                  <button aria-label="减少" className="px-[9px] text-fg-2 hover:text-fg" onClick={() => setTimeoutSec(settings.closeTimeoutSec - 1)}>
                    <Minus className="size-[13px]" />
                  </button>
                  <span className="flex h-full min-w-14 items-center justify-center border-x border-line-strong px-3 text-[13px]">
                    {settings.closeTimeoutSec} 秒
                  </span>
                  <button aria-label="增加" className="px-[9px] text-fg-2 hover:text-fg" onClick={() => setTimeoutSec(settings.closeTimeoutSec + 1)}>
                    <Plus className="size-[13px]" />
                  </button>
                </div>
              </Row>
            </div>
          </Section>

          <Section title="关于">
            <div className="rounded-xl border border-line bg-surface">
              <Row
                first
                label="运行权限"
                desc={elevated ? "当前以管理员权限运行" : "当前为普通权限。结束服务或 SYSTEM 进程需要管理员权限"}
              >
                {!elevated && (
                  <Button variant="outline" onClick={requestRelaunch}>
                    <Shield />
                    以管理员身份重新启动
                  </Button>
                )}
              </Row>
              <Row label="版本" desc={`port-helper ${version || "—"}`}>
                <span className="font-mono text-xs text-fg-3">Windows x64</span>
              </Row>
            </div>
          </Section>
        </div>
      </div>

      {confirmClear && (
        <Suspense fallback={null}>
          <ClearRecentDialog open onClose={() => setConfirmClear(false)} />
        </Suspense>
      )}
    </div>
  );
}
