import { projectSearch } from "../features/projects/types";
import type { Filter, PortEntry, SortKey } from "./types";
export const isLocal = (address: string) =>
  address === "::1" || address === "localhost" || address.startsWith("127.");
export const displayAddress = (address: string) =>
  isLocal(address)
    ? "localhost"
    : address === "0.0.0.0" || address === "::"
      ? "All interfaces"
      : address;
export function service(entry: PortEntry): {
  label: string;
  icon: string;
  color: string;
} {
  const name = `${entry.process} ${entry.command.join(" ")}`.toLowerCase();
  if (name.includes("vite"))
    return { label: "Vite dev server", icon: "vite", color: "purple" };
  if (name.includes("next"))
    return { label: "Next.js application", icon: "next", color: "white" };
  if (name.includes("postgres"))
    return { label: "PostgreSQL database", icon: "database", color: "blue" };
  if (name.includes("redis"))
    return { label: "Redis cache", icon: "redis", color: "red" };
  if (name.includes("mongo"))
    return { label: "MongoDB database", icon: "leaf", color: "green" };
  if (name.includes("python"))
    return { label: "Python service", icon: "python", color: "yellow" };
  if (name.includes("docker"))
    return { label: "Docker service", icon: "docker", color: "blue" };
  if (name.includes("node"))
    return { label: "Node.js application", icon: "node", color: "green" };
  if (name.includes("rust") || name.includes("cargo"))
    return { label: "Rust application", icon: "rust", color: "orange" };
  if (entry.system)
    return { label: "System service", icon: "system", color: "gray" };
  return { label: "Local service", icon: "terminal", color: "gray" };
}
export function isHttp(entry: PortEntry) {
  return (
    entry.protocol === "TCP" &&
    [3000, 3001, 4000, 4200, 5000, 5173, 5174, 8000, 8080, 8888].includes(
      entry.port,
    )
  );
}
export function portFromQuery(query: string): number | null {
  const value = query.trim().replace(/^:/, "");
  if (!/^\d{1,5}$/.test(value)) return null;
  const port = Number(value);
  return port > 0 && port <= 65535 ? port : null;
}
export function filterPorts(
  entries: PortEntry[],
  query: string,
  filter: Filter,
  showSystem: boolean,
): PortEntry[] {
  const search = query.trim().toLowerCase();
  const exact = search.startsWith(":") ? portFromQuery(search) : null;
  return entries.filter((p) => {
    if (!showSystem && p.system) return false;
    if (filter === "TCP" || filter === "UDP") {
      if (p.protocol !== filter) return false;
    }
    if (filter === "Listening" && p.protocol !== "TCP") return false;
    if (filter === "Localhost" && !isLocal(p.address)) return false;
    if (filter === "External" && isLocal(p.address)) return false;
    if (filter === "System" && !p.system) return false;
    if (filter === "User" && p.system) return false;
    if (exact !== null) return p.port === exact;
    return (
      !search ||
      `${p.port} ${p.pid ?? ""} ${p.process} ${p.address} ${displayAddress(p.address)} ${p.command.join(" ")} ${p.project ? projectSearch(p.project) : ""} ${p.serviceName ?? ""}`
        .toLowerCase()
        .includes(search)
    );
  });
}
export function sortPorts(
  entries: PortEntry[],
  key: SortKey,
  ascending: boolean,
) {
  return [...entries].sort((a, b) => {
    const x =
        (key === "process"
          ? (a.project?.name ?? a.serviceName ?? a.process)
          : a[key]) ?? 0,
      y =
        (key === "process"
          ? (b.project?.name ?? b.serviceName ?? b.process)
          : b[key]) ?? 0;
    const result =
      typeof x === "string" && typeof y === "string"
        ? x.localeCompare(y)
        : Number(x) - Number(y);
    return (ascending ? result : -result) || a.id.localeCompare(b.id);
  });
}
export function uptime(startedAt: number | null) {
  if (!startedAt) return "—";
  const seconds = Math.max(0, Date.now() / 1000 - startedAt);
  if (seconds < 60) return `${Math.floor(seconds)}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400)
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`;
}
export function reconcile(previous: PortEntry[], next: PortEntry[]) {
  const old = new Map(previous.map((p) => [p.id, p]));
  return next.map((p) => {
    const existing = old.get(p.id);
    return existing && JSON.stringify(existing) === JSON.stringify(p)
      ? existing
      : p;
  });
}
export function connectionAddress(entry: PortEntry) {
  return `${entry.address.includes(":") ? `[${entry.address}]` : entry.address}:${entry.port}`;
}
