import { Trash2 } from "lucide-react";
import { ConfirmDialog } from "@/components/dialogs/ConfirmDialog";
import { Button } from "@/components/ui/button";
import { useSettings } from "@/hooks/use-settings";
import { notify } from "@/lib/notify";

export function ClearRecentDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { settings, update } = useSettings();
  const ports = settings.recentPorts;
  return (
    <ConfirmDialog
      open={open}
      onClose={onClose}
      tone="neutral"
      icon={Trash2}
      title="清空常用端口？"
      description={`将删除全部 ${ports.length} 个最近查询的端口，清空后无法恢复。之后的查询会重新记录。`}
      actions={
        <>
          <Button variant="outline" onClick={onClose} autoFocus>
            取消
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              update({ recentPorts: [] });
              onClose();
              notify.success("已清空常用端口");
            }}
          >
            <Trash2 />
            清空
          </Button>
        </>
      }
    >
      <div className="flex flex-wrap gap-1.5">
        {ports.map((p) => (
          <span key={p} className="rounded-md bg-surface-muted px-2 py-1 font-mono text-[11.5px] text-fg-2">
            {p}
          </span>
        ))}
      </div>
    </ConfirmDialog>
  );
}
