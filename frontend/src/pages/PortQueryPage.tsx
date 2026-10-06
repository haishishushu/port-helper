import { CircleCheck, Plug, Unplug } from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import type { AppError } from "@/bindings/AppError";
import type { PortQueryResult } from "@/bindings/PortQueryResult";
import type { ProcessDetail } from "@/bindings/ProcessDetail";
import type { ProcessGroup } from "@/bindings/ProcessGroup";
import { TitleBar } from "@/components/layout/TitleBar";
import { Composer, type ComposerSeed } from "@/components/query/Composer";
import { DetailPanel, processNote } from "@/components/query/DetailPanel";
import { ProcessCard } from "@/components/query/ProcessCard";
import { QuickPorts } from "@/components/query/QuickPorts";
import { RefreshButton } from "@/components/query/Refresh";
import { EmptyState, HiddenNote, LoadingState, QueryErrorState } from "@/components/query/States";
import { Button } from "@/components/ui/button";
import { useProcessActions, type ActionTarget, type Settled } from "@/hooks/use-process-actions";
import { useRefreshShortcut } from "@/hooks/use-refresh-shortcut";
import { useSettings } from "@/hooks/use-settings";
import { api, toAppError } from "@/lib/api";
import { localEndpoint, stateLabel } from "@/lib/format";
import { notify } from "@/lib/notify";
import { pushRecentPort } from "@/lib/recent-ports";
import { parsePort } from "@/lib/validate";
import { cn } from "@/lib/utils";

const ClearRecentDialog = lazy(() =>
  import("@/components/dialogs/ClearRecentDialog").then((m) => ({ default: m.ClearRecentDialog })),
);

export interface PortRequest {
  port: number;
  nonce: number;
}

const groupTarget = (g: ProcessGroup, port: number, note: string | null = null): ActionTarget => ({
  process: g.process,
  endpoints: g.bindings.map((b) => `${b.protocol.toUpperCase()} ${localEndpoint(b)}`),
  port,
  note,
});

/** PID 0（TIME_WAIT 等内核记录）不算真正的占用者 */
const occupants = (r: PortQueryResult) => r.groups.filter((g) => g.process.pid !== 0);

/** 结果指纹：用于判断刷新前后占用情况是否有变化 */
const signature = (r: PortQueryResult) =>
  r.groups
    .map((g) => `${g.process.pid}@${g.process.startTime}:` + g.bindings.map((b) => `${localEndpoint(b)}/${b.state ? stateLabel(b.state) : ""}`).join(","))
    .join("|");

export function PortQueryPage({ active, request, onShowAllPorts }: {
  active: boolean;
  request: PortRequest | null;
  onShowAllPorts: (pid: number) => void;
}) {
  const { settings, loaded, update } = useSettings();
  const [seed, setSeed] = useState<ComposerSeed>({ value: "", nonce: 0 });
  /** 写入输入框（快捷端口、跳转、返回主页、提交成功后同步紧凑输入框） */
  const fill = (value: string) => setSeed((s) => ({ value, nonce: s.nonce + 1 }));
  const [inputError, setInputError] = useState<string | null>(null);
  const [includeAll, setIncludeAll] = useState(false);
  const [port, setPort] = useState<number | null>(null);
  const [result, setResult] = useState<PortQueryResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  const [selectedPid, setSelectedPid] = useState<number | null>(null);
  const [released, setReleased] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const seq = useRef(0);
  const lastResult = useRef<PortQueryResult | null>(null);

  useEffect(() => {
    if (loaded) setIncludeAll(!settings.listenOnly);
  }, [loaded, settings.listenOnly]);

  const run = useCallback(
    async (target: number, all: boolean, opts: { after?: Settled; refresh?: boolean } = {}) => {
      const my = ++seq.current;
      setPort(target);
      setLoading(true);
      setError(null);
      try {
        const r = await api.queryPort(target, all);
        if (my !== seq.current) return;
        if (opts.refresh) reportAfterRefresh(lastResult.current, r);
        lastResult.current = r;
        setResult(r);
        setSelectedPid((prev) => {
          if (prev != null && r.groups.some((g) => g.process.pid === prev)) return prev;
          return (r.groups.find((g) => g.process.actionable) ?? r.groups[0])?.process.pid ?? null;
        });
        if (opts.after) reportAfterAction(opts.after, r);
      } catch (err) {
        if (my === seq.current) {
          lastResult.current = null;
          setResult(null);
          setError(toAppError(err));
        }
      } finally {
        if (my === seq.current) setLoading(false);
      }
    },
    [],
  );

  /** 操作完成后重新查询，并根据端口是否真正释放给出提示。 */
  const reportAfterAction = (after: Settled, r: PortQueryResult) => {
    const still = occupants(r);
    const name =
      after.action === "released" ? after.closed.map((t) => t.process.appName).join("、") : after.target.process.appName;
    if (still.length === 0) {
      const verb = after.action === "killed" ? "已强制结束" : "已关闭";
      setReleased(`${name} 已退出，重新查询未发现占用记录`);
      notify.success(`${verb} ${name}，端口 ${r.port} 已释放`);
    } else {
      setReleased(null);
      const other = still[0].process;
      notify.warning(`进程已退出，但端口 ${r.port} 又被 ${other.name}（PID ${other.pid}）占用`, {
        label: "查看",
        onClick: () => setSelectedPid(other.pid),
      });
    }
  };

  /** 刷新只在结果有变化时提示；没有变化时由卡片柔光和图标旋转提供反馈，不打扰 */
  const reportAfterRefresh = (before: PortQueryResult | null, after: PortQueryResult) => {
    if (!before || signature(before) === signature(after)) return;
    const still = occupants(after);
    if (still.length === 0) notify.info(`已刷新，端口 ${after.port} 已空闲`);
    else notify.info(`已刷新，端口 ${after.port} 的占用有变化，现有 ${still.length} 个进程`);
  };

  /** 结果页返回主页：丢弃进行中的查询，清空结果与输入，常用端口保留 */
  const goHome = () => {
    seq.current++;
    lastResult.current = null;
    setPort(null);
    setResult(null);
    setError(null);
    setLoading(false);
    setSelectedPid(null);
    setReleased(null);
    fill("");
    setInputError(null);
  };

  const requery = () => port != null && void run(port, includeAll);
  /** 用户主动刷新：保留当前结果，起跑柔光；查询进行中不重复触发 */
  const refresh = () => {
    if (port == null || loading) return;
    setRefreshKey((k) => k + 1);
    void run(port, includeAll, { refresh: true });
  };
  useRefreshShortcut(active && port != null, refresh);
  const refreshState = { refreshKey, refreshing: loading };

  const actions = useProcessActions({
    onSettled: (s) => port != null && void run(port, includeAll, { after: s }),
    onRequery: requery,
  });

  const submit = (raw: string) => {
    const p = parsePort(raw);
    if (p == null) {
      setInputError("端口号需为 1–65535 之间的整数");
      return;
    }
    setInputError(null);
    fill(String(p));
    setReleased(null);
    update({ recentPorts: pushRecentPort(settings.recentPorts, p) });
    void run(p, includeAll);
  };

  useEffect(() => {
    if (!request) return;
    submit(String(request.port));
  }, [request?.nonce]); // 只响应外部跳转请求

  const toggleIncludeAll = () => {
    const next = !includeAll;
    setIncludeAll(next);
    if (port != null) void run(port, next);
  };

  const selected = result?.groups.find((g) => g.process.pid === selectedPid) ?? null;
  const occupied = result ? occupants(result) : [];
  const actionableTargets = occupied.filter((g) => g.process.actionable).map((g) => groupTarget(g, result!.port));
  const detailTarget = (d: ProcessDetail) => {
    const g = result!.groups.find((x) => x.process.pid === d.pid)!;
    return groupTarget({ ...g, process: d }, result!.port, processNote(d)?.text ?? null);
  };

  const clearDialog = confirmClear && (
    <Suspense fallback={null}>
      <ClearRecentDialog open onClose={() => setConfirmClear(false)} />
    </Suspense>
  );

  // 主页：还没有发起过查询
  if (port == null) {
    return (
      <div className="flex min-w-0 flex-1 flex-col">
        <TitleBar title="端口查询" />
        <div className="flex flex-1 flex-col items-center justify-center gap-7 pb-[90px]">
          <div className="flex flex-col items-center gap-2">
            <h1 className="text-[26px] font-semibold">哪个端口被占用了？</h1>
            <p className="text-sm text-fg-2">输入端口号，定位占用它的进程，并关闭或结束它</p>
          </div>
          <Composer
            variant="hero"
            mode="port"
            seed={seed}
            onSubmit={submit}
            onErrorDismiss={() => setInputError(null)}
            error={inputError}
            includeAll={includeAll}
            onToggleIncludeAll={toggleIncludeAll}
          />
          <QuickPorts
            ports={settings.recentPorts}
            onPick={(p) => submit(String(p))}
            onClear={() => setConfirmClear(true)}
          />
        </div>
        {clearDialog}
      </div>
    );
  }

  const busyDot = occupied.length > 0;
  const recordCount = result?.groups.reduce((n, g) => n + g.bindings.length, 0) ?? 0;

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <TitleBar title="端口查询" crumb={`端口 ${port}`} onBack={goHome} backLabel="返回端口查询主页" />
      <div className="px-5 pt-1 pb-3.5">
        <Composer
          variant="compact"
          mode="port"
          seed={seed}
          onSubmit={submit}
          onErrorDismiss={() => setInputError(null)}
          error={inputError}
          includeAll={includeAll}
          onToggleIncludeAll={toggleIncludeAll}
          loading={loading}
        />
      </div>
      <div className="flex min-h-0 flex-1 border-t border-line">
        <section className="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto px-5 py-4">
          {result && (
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className={cn("size-2 shrink-0 rounded-full", busyDot ? "bg-danger" : "bg-success")} />
                <span className="truncate text-[13px] font-medium">
                  {busyDot
                    ? `端口 ${result.port} 已被占用 · ${occupied.length} 个进程 · ${recordCount} 条${includeAll ? "" : "监听"}记录`
                    : `端口 ${result.port} 当前空闲`}
                </span>
                <span className="shrink-0 font-mono text-[11px] whitespace-nowrap text-fg-3">{result.elapsedMs} ms</span>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <RefreshButton {...refreshState} onRefresh={refresh} />
                {actionableTargets.length > 0 && (
                  <Button size="default" onClick={() => actions.requestRelease(result.port, actionableTargets)}>
                    <Unplug />
                    立即释放端口
                  </Button>
                )}
              </div>
            </div>
          )}
          {loading && !result && !error && <LoadingState label={`正在查询端口 ${port}…`} />}
          {error && <QueryErrorState error={error} onRetry={requery} />}
          {result && result.groups.length === 0 && !loading && (
            released ? (
              <EmptyState icon={CircleCheck} tone="success" title={`端口 ${result.port} 已释放`} description={released} />
            ) : (
              <EmptyState
                icon={Plug}
                title={`端口 ${result.port} 未被占用`}
                description="TCP / UDP、IPv4 / IPv6 均未发现记录，可以直接使用"
              />
            )
          )}
          {result?.groups.map((g) => (
            <ProcessCard
              key={g.process.pid}
              process={g.process}
              bindings={g.bindings}
              selected={g.process.pid === selectedPid}
              onSelect={setSelectedPid}
              refreshKey={refreshKey}
              refreshing={loading}
            />
          ))}
          {result && <HiddenNote hidden={result.hidden} onShowAll={toggleIncludeAll} />}
        </section>
        {selected && result && (
          <DetailPanel
            key={selected.process.pid}
            detail={selected.process}
            port={result.port}
            showAllPortsLink
            onShowAllPorts={onShowAllPorts}
            onClose={(d) => actions.requestClose(detailTarget(d))}
            onKill={(d) => actions.requestKill(detailTarget(d))}
          />
        )}
      </div>
      {actions.element}
      {clearDialog}
    </div>
  );
}
