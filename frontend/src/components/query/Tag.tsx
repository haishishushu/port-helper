import { cn } from "@/lib/utils";

export type TagTone = "neutral" | "success" | "info" | "warning" | "danger";

const TONE: Record<TagTone, string> = {
  neutral: "bg-surface-muted text-fg-2",
  success: "bg-success-bg text-success",
  info: "bg-info-bg text-info",
  warning: "bg-warning-bg text-warning",
  danger: "bg-danger-bg text-danger",
};

export function Tag({ tone = "neutral", mono = true, className, children }: {
  tone?: TagTone;
  mono?: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center rounded-[5px] px-[7px] py-0.5 text-[11px] leading-4 font-medium",
        mono && "font-mono",
        TONE[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}
