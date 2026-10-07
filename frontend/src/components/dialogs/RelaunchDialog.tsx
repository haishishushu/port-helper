import { Shield } from "lucide-react";
import { useState } from "react";
import { ConfirmDialog } from "@/components/dialogs/ConfirmDialog";
import { Button } from "@/components/ui/button";
import { api, toAppError } from "@/lib/api";
import { notifyError } from "@/lib/notify";
import { platform } from "@/lib/platform";

/** “以管理员（root）身份重新启动”确认框（按需加载，打开时才挂载） */
export function RelaunchDialog({ onClose }: { onClose: () => void }) {
  const [busy, setBusy] = useState(false);

  const relaunch = async () => {
    setBusy(true);
    try {
      await api.relaunchAsAdmin();
    } catch (err) {
      onClose();
      notifyError(toAppError(err), "重新启动失败");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ConfirmDialog
      open
      onClose={onClose}
      tone="info"
      icon={Shield}
      title={`以${platform.admin}身份重新启动？`}
      description={`当前窗口会关闭，并以${platform.admin}权限重新打开 port-helper。${platform.elevatePrompt}`}
      actions={
        <>
          <Button variant="outline" onClick={onClose} autoFocus>
            取消
          </Button>
          <Button onClick={relaunch} disabled={busy}>
            <Shield />
            重新启动
          </Button>
        </>
      }
    />
  );
}
