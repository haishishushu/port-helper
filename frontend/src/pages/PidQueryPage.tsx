import { ArrowUpRight, CircleCheck, CircleX, Cpu, Minus, MousePointerClick, Plug, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { AppError } from "@/bindings/AppError";
import type { PidQueryResult } from "@/bindings/PidQueryResult";
import type { PortBindings } from "@/bindings/PortBindings";
import type { ProcessDetail } from "@/bindings/ProcessDetail";
import { TitleBar } from "@/components/layout/TitleBar";
import { Composer, type ComposerSeed } from "@/components/query/Composer";
import { DetailPanel, processNote } from "@/components/query/DetailPanel";
import { BindingRow } from "@/components/query/ProcessCard";
import { RefreshButton, RefreshVeil, type RefreshState } from "@/components/query/Refresh";
import { EmptyState, HiddenNote, LoadingState, QueryErrorState } from "@/components/query/States";
import { Button } from "@/components/ui/button";
import { useProcessActions, type ActionTarget, type Settled } from "@/hooks/use-process-actions";
import { useRefreshShortcut } from "@/hooks/use-refresh-shortcut";
import { useSettings } from "@/hooks/use-settings";
import { api, toAppError } from "@/lib/api";
import { localEndpoint, stateLabel } from "@/lib/format";
import { notify } from "@/lib/notify";
import { parsePid } from "@/lib/validate";

export interface PidRequest {
  pid: number;
  nonce: number;
}

/** 结果指纹：用于判断刷新前后端口是否有变化 */
const signature = (r: PidQueryResult) =>
  [...r.tcp, ...r.udp]
    .flatMap((g) => g.bindings)
    .map((b) => `${b.protocol}:${localEndpoint(b)}/${b.state ? stateLabel(b.state) : ""}`)
    .join("|");

function PortCard({ group, onGoto, refresh }: { group: PortBindings; onGoto: (port: number) => void; refresh: RefreshState }) {
  return (
    <div className="relative flex flex-col rounded-xl border border-line bg-surface">
      <div className="flex items-center gap-2.5 px-4 py-3">
        <span className="font-mono text-base font-semibold">{group.port}</span>
        <span className="text-xs text-fg-3">{group.bindings.length} 条记录</span>
        <span className="flex-1" />
        <button onClick={() => onGoto(group.port)} className="flex items-center gap-1 text-xs text-fg-2 hover:text-fg">
          在端口查询中查看
          <ArrowUpRight className="size-[13px]" />
        </button>
      </div>
      {group.bindings.map((b, i) => (
        <BindingRow key={i} b={b} />
      ))}
      <RefreshVeil {...refresh} />
    </div>
  );
}

function Section({ title, groups, onGoto, refresh }: {
  title: string;
  groups: PortBindings[];
  onGoto: (port: number) => void;
  refresh: RefreshState;
}) {
  return (
    <>
      <div className="flex items-center gap-2 px-0.5 pt-1.5">
        <span className="text-xs font-semibold text-fg-2">{title}</span>
        <span className="font-mono text-[11px] text-fg-3">{groups.length} 个端口</span>
      </div>
      {groups.length === 0 ? (
        <div className="relative flex items-center gap-2 rounded-xl border border-line px-4 py-3 text-[12.5px] text-fg-3">
          <Minus className="size-[13px]" />
          该进程没有占用 {title} 端口
          <RefreshVeil {...refresh} />
        </div>
      ) : (
        groups.map((g) => <PortCard key={g.port} group={g} onGoto={onGoto} refresh={refresh} />)
      )}
    </>
  );
}

export function PidQueryPage({ active, request, onGotoPort }: {
  active: boolean;
  request: PidRequest | null;
  onGotoPort: (port?: number) => void;
}) {
  const { settings, loaded } = useSettings();
  const [seed, setSeed] = useState<ComposerSeed>({ value: "", nonce: 0 });
  /** 写入输入框（快捷端口、跳转、返回主页、提交成功后同步紧凑输入框） */
  const fill = (value: string) => setSeed((s) => ({ value, nonce: s.nonce + 1 }));
  const [inputError, setInputError] = useState<string | null>(null);
  const [includeAll, setIncludeAll] = useState(false);
  const [pid, setPid] = useState<number | null>(null);
  const [result, setResult] = useState<PidQueryResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  /** 进程被本页关闭/结束后，查询结果显示“已退出”而不是“PID 不存在” */
  const [ended, setEnded] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const seq = useRef(0);
  const lastResult = useRef<PidQueryResult | null>(null);

  useEffect(() => {
    if (loaded) setIncludeAll(!settings.listenOnly);
  }, [loaded, settings.listenOnly]);

  const run = async (target: number, all: boolean, refresh = false) => {
    const my = ++seq.current;
    setPid(target);
    setLoading(true);
    setError(null);
    try {
      const r = await api.queryPid(target, all);
      if (my !== seq.current) return;
      const before = lastResult.current;
      if (refresh && before && signature(before) !== signature(r)) {
        notify.info(`已刷新，PID ${r.process.pid} 的端口有变化，现占用 ${r.tcp.length + r.udp.length} 个端口`);
      }
      lastResult.current = r;
      setResult(r);
    } catch (err) {
      if (my === seq.current) {
        const e = toAppError(err);
        if (refresh && e.code === "PROCESS_NOT_FOUND") setEnded("进程在刷新前已经退出，它占用的端口已全部释放。");
        lastResult.current = null;
        setResult(null);
        setError(e);
      }
    } finally {
      if (my === seq.current) setLoading(false);
    }
  };

  /** 结果页返回主页：丢弃进行中的查询，清空结果与输入 */
  const goHome = () => {
    seq.current++;
    lastResult.current = null;
    setPid(null);
    setResult(null);
    setError(null);
    setLoading(false);
    setEnded(null);
    fill("");
    setInputError(null);
  };

  const requery = () => pid != null && void run(pid, includeAll);
  /** 用户主动刷新：保留当前结果，起跑柔光；查询进行中不重复触发 */
  const refresh = () => {
    if (pid == null || loading) return;
    setRefreshKey((k) => k + 1);
    void run(pid, includeAll, true);
  };
  useRefreshShortcut(active && pid != null, refresh);
  const refreshState = { refreshKey, refreshing: loading };
  const onSettled = (s: Settled) => {
    if (s.action === "released") return;
    const verb = s.action === "killed" ? "已强制结束" : "已关闭";
    notify.success(`${verb} ${s.target.process.appName}（PID ${s.target.process.pid}）`);
    setEnded(`${s.target.process.appName}（${s.target.process.name}）${verb}，它占用的端口已全部释放。`);
    requery();
  };
  const actions = useProcessActions({ onSettled, onRequery: requery });

  const submit = (raw: string) => {
    const p = parsePid(raw);
    if (p == null) {
      setInputError("PID 需为非负整数");
      return;
    }
    setInputError(null);
    fill(String(p));
    setEnded(null);
    void run(p, includeAll);
  };

  useEffect(() => {
    if (!request) return;
    submit(String(request.pid));
  }, [request?.nonce]); // 只响应外部跳转请求

  const toggleIncludeAll = () => {
    const next = !includeAll;
    setIncludeAll(next);
    if (pid != null) void run(pid, next);
  };

  const target = (d: ProcessDetail): ActionTarget => ({
    process: d,
    endpoints: [...(result?.tcp ?? []), ...(result?.udp ?? [])]
      .flatMap((g) => g.bindings)
      .map((b) => `${b.protocol.toUpperCase()} ${localEndpoint(b)}`),
    port: result?.tcp[0]?.port ?? result?.udp[0]?.port,
    note: processNote(d)?.text ?? null,
  });

  const composer = (variant: "hero" | "compact") => (
    <Composer
      variant={variant}
      mode="pid"
      seed={seed}
      onSubmit={submit}
      onErrorDismiss={() => setInputError(null)}
      error={inputError}
      includeAll={includeAll}
      onToggleIncludeAll={toggleIncludeAll}
      loading={loading}
    />
  );

  if (pid == null) {
    return (
      <div className="flex min-w-0 flex-1 flex-col">
        <TitleBar title="进程反查" />
        <div className="flex flex-1 flex-col items-center justify-center gap-7 pb-[90px]">
          <div className="flex flex-col items-center gap-2">
            <h1 className="text-[26px] font-semibold">这个进程占用了哪些端口？</h1>
            <p className="text-sm text-fg-2">输入 PID，列出它占用的全部 TCP / UDP 端口</p>
          </div>
          {composer("hero")}
          <div className="flex w-[620px] flex-col gap-2 text-xs text-fg-3">
            <span className="flex items-center gap-2">
              <MousePointerClick className="size-[13px]" />
              在端口查询结果中点击“查看该进程的全部端口”，会自动带入 PID 跳转到这里
            </span>
            <span className="flex items-center gap-2">
              <Cpu className="size-[13px]" />
              PID 只在进程运行期间有效，进程重启后 PID 会变化
            </span>
          </div>
        </div>
      </div>
    );
  }

  const portCount = result ? result.tcp.length + result.udp.length : 0;
  const recordCount = result ? [...result.tcp, ...result.udp].reduce((n, g) => n + g.bindings.length, 0) : 0;
  const notFound = error?.code === "PROCESS_NOT_FOUND";

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <TitleBar title="进程反查" crumb={`PID ${pid}`} onBack={goHome} backLabel="返回进程反查主页" />
      <div className="px-5 pt-1 pb-3.5">{composer("compact")}</div>
      <div className="flex min-h-0 flex-1 border-t border-line">
        <section className="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto px-5 py-4">
          {result && (
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span className="size-2 rounded-full bg-info" />
                <span className="text-[13px] font-medium">
                  PID {result.process.pid} · 占用 {portCount} 个端口 · {recordCount} 条{includeAll ? "" : "监听"}记录
                </span>
                <span className="shrink-0 font-mono text-[11px] whitespace-nowrap text-fg-3">{result.elapsedMs} ms</span>
              </div>
              <RefreshButton {...refreshState} onRefresh={refresh} />
            </div>
          )}
          {loading && !result && !error && <LoadingState label={`正在查询 PID ${pid}…`} />}
          {notFound &&
            (ended ? (
              <EmptyState icon={CircleCheck} tone="success" title="进程已退出" description={ended} />
            ) : (
              <EmptyState
                icon={CircleX}
                tone="danger"
                title={`PID ${pid} 不存在`}
                description="该进程可能已经退出，或 PID 输入有误。进程重启后 PID 会变化。"
              >
                <Button variant="outline" onClick={() => onGotoPort()}>
                  <Search />
                  改用端口查询
                </Button>
              </EmptyState>
            ))}
          {error && !notFound && <QueryErrorState error={error} onRetry={requery} />}
          {result && portCount === 0 && (
            <EmptyState
              icon={Plug}
              title={`PID ${result.process.pid} 未占用任何端口`}
              description={`${result.process.name} 正在运行，但当前没有${includeAll ? "" : "监听或使用"}任何 TCP / UDP 端口。`}
            />
          )}
          {result && portCount > 0 && (
            <>
              <Section title="TCP" groups={result.tcp} onGoto={onGotoPort} refresh={refreshState} />
              <Section title="UDP" groups={result.udp} onGoto={onGotoPort} refresh={refreshState} />
              <HiddenNote hidden={result.hidden} onShowAll={toggleIncludeAll} />
            </>
          )}
        </section>
        {result && (
          <DetailPanel
            key={result.process.pid}
            detail={result.process}
            onClose={(d) => actions.requestClose(target(d))}
            onKill={(d) => actions.requestKill(target(d))}
          />
        )}
      </div>
      {actions.element}
    </div>
  );
}
