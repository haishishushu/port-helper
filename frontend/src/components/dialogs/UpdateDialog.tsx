import { ArrowRight, CalendarDays, CircleAlert, CircleArrowUp, Download, LoaderCircle, PackageCheck, RotateCcw } from "lucide-react";
import { ConfirmDialog } from "@/components/dialogs/ConfirmDialog";
import { Button } from "@/components/ui/button";
import { progressPercent, useUpdate } from "@/hooks/use-update";
import { formatBytes } from "@/lib/format";

/**
 * 更新进度框（按需加载，打开时才挂载）。
 * 下载期间可以关掉它继续使用，下载在后台进行，左上角按钮会显示进度；下载完成后点一次“安装并重启”。
 */
export function UpdateDialog({ onClose }: { onClose: () => void }) {
  const { phase, info, progress, error, openUpdate, install } = useUpdate();
  if (!info) return null;

  const version = `v${info.availableVersion}`;
  const percent = progressPercent(progress);
  const installing = phase === "installing";

  const view = {
    downloading: { icon: LoaderCircle, spinning: true, tone: "success", title: `正在下载 ${version}`, desc: "下载完成后会自动校验签名。关闭此窗口不会中断下载。" },
    downloaded: { icon: PackageCheck, spinning: false, tone: "success", title: `${version} 已准备就绪`, desc: "安装包已下载并通过签名校验。安装时 port-helper 会先关闭，装好后自动重新打开。" },
    downloadError: { icon: CircleAlert, spinning: false, tone: "danger", title: "下载更新失败", desc: error ?? "请检查网络后重试。" },
    installing: { icon: LoaderCircle, spinning: true, tone: "success", title: `正在安装 ${version}`, desc: "安装器正在接管，完成后 port-helper 会自动重新打开。" },
  } as const;
  const v = phase in view ? view[phase as keyof typeof view] : view.downloading;

  return (
    <ConfirmDialog
      open
      onClose={onClose}
      dismissible={!installing}
      tone={v.tone}
      icon={v.icon}
      spinning={v.spinning}
      title={v.title}
      description={phase === "downloaded" && error ? error : v.desc}
      rows={[
        {
          icon: CircleArrowUp,
          label: "版本",
          value: (
            <span className="flex items-center gap-1.5">
              <span className="text-fg-3">v{info.currentVersion}</span>
              <ArrowRight className="size-3 text-fg-3" />
              <span className="font-semibold text-success">{version}</span>
            </span>
          ),
        },
        ...(info.pubDate ? [{ icon: CalendarDays, label: "发布时间", value: info.pubDate.slice(0, 10) }] : []),
      ]}
      actions={
        phase === "downloaded" ? (
          <>
            <Button variant="outline" onClick={onClose}>
              稍后
            </Button>
            <Button variant="success" onClick={install} autoFocus>
              <PackageCheck />
              安装并重启
            </Button>
          </>
        ) : phase === "downloadError" ? (
          <>
            <Button variant="outline" onClick={onClose}>
              关闭
            </Button>
            <Button variant="success" onClick={openUpdate} autoFocus>
              <RotateCcw />
              重新下载
            </Button>
          </>
        ) : installing ? (
          <Button variant="success" disabled>
            <LoaderCircle className="animate-spin" />
            正在安装…
          </Button>
        ) : (
          <Button variant="outline" onClick={onClose} autoFocus>
            <Download />
            后台下载
          </Button>
        )
      }
    >
      {phase === "downloading" && (
        <div className="flex flex-col gap-1.5">
          <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent ?? undefined}
            className="h-1.5 overflow-hidden rounded-full bg-surface-muted"
          >
            <div
              className={percent === null ? "h-full w-2/5 animate-pulse rounded-full bg-success" : "h-full rounded-full bg-success transition-[width] duration-200"}
              style={percent === null ? undefined : { width: `${percent}%` }}
            />
          </div>
          <div className="flex justify-between font-mono text-[11.5px] text-fg-3">
            <span>{progress.total ? `${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}` : formatBytes(progress.downloaded)}</span>
            <span className="font-semibold text-success">{percent === null ? "…" : `${percent}%`}</span>
          </div>
        </div>
      )}
      {info.notes && (
        <div className="flex flex-col gap-1.5">
          <span className="text-xs font-semibold text-fg-2">更新内容</span>
          <p className="max-h-36 overflow-y-auto rounded-[10px] bg-surface-muted px-3.5 py-3 text-[12.5px] leading-relaxed whitespace-pre-wrap text-fg-2">
            {info.notes}
          </p>
        </div>
      )}
    </ConfirmDialog>
  );
}
