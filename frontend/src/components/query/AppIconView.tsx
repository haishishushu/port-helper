import { AppWindow, Coffee, Container, Cpu, Database, Server, Settings2, SquareTerminal, Zap, type LucideIcon } from "lucide-react";
import type { AppIcon } from "@/bindings/AppIcon";
import { cn } from "@/lib/utils";

const ICONS: Record<AppIcon, LucideIcon> = {
  generic: AppWindow,
  web: Zap,
  java: Coffee,
  python: SquareTerminal,
  database: Database,
  server: Server,
  container: Container,
  terminal: SquareTerminal,
  system: Cpu,
  service: Settings2,
};

export function AppIconView({ icon, className }: { icon: AppIcon; className?: string }) {
  const Icon = ICONS[icon];
  return (
    <span className={cn("flex size-[34px] shrink-0 items-center justify-center rounded-[9px] bg-surface-muted", className)}>
      <Icon className="size-[17px] text-fg" />
    </span>
  );
}
