import { Toaster as Sonner } from "sonner";
import type { CSSProperties } from "react";

/** 提示统一显示在窗口顶部（标题栏区域）、水平居中，新提示替换旧提示。 */
export function Toaster({ theme }: { theme: "light" | "dark" }) {
  return (
    <Sonner
      theme={theme}
      position="top-center"
      offset={2}
      visibleToasts={1}
      gap={0}
      style={{ "--width": "720px" } as CSSProperties}
      toastOptions={{ unstyled: true, classNames: { toast: "flex w-full justify-center" } }}
    />
  );
}
