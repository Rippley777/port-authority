import {
  Box,
  Database,
  Hexagon,
  Layers,
  Leaf,
  Server,
  Settings2,
  Terminal,
  Zap,
} from "lucide-react";
import { service } from "../lib/ports";
import type { PortEntry } from "../lib/types";
export function ServiceIcon({
  entry,
  large = false,
}: {
  entry: PortEntry;
  large?: boolean;
}) {
  const { icon, color } = service(entry);
  const Icon = (
    {
      vite: Zap,
      database: Database,
      node: Hexagon,
      redis: Layers,
      leaf: Leaf,
      docker: Box,
      system: Settings2,
      rust: Server,
      terminal: Terminal,
    } as Record<string, typeof Zap>
  )[icon];
  return (
    <span className={`service-icon ${color} ${large ? "large" : ""}`}>
      {Icon ? (
        <Icon size={large ? 25 : 17} strokeWidth={1.8} />
      ) : (
        <span className={icon === "next" ? "next-logo" : "python-logo"}>
          {icon === "next" ? "N" : "Py"}
        </span>
      )}
    </span>
  );
}
