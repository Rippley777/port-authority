import { defaultSettings } from "./types";
import type { HistoryEvent, Settings } from "./types";
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
export function parseSettings(value: unknown): Settings {
  if (!record(value)) return defaultSettings;
  return {
    refreshInterval:
      typeof value.refreshInterval === "number" &&
      [1, 2, 5, 10].includes(value.refreshInterval)
        ? value.refreshInterval
        : 2,
    showSystem: typeof value.showSystem === "boolean" ? value.showSystem : true,
    confirmKill:
      typeof value.confirmKill === "boolean" ? value.confirmKill : true,
    protocol: value.protocol === "https" ? "https" : "http",
    keepHistory:
      typeof value.keepHistory === "boolean" ? value.keepHistory : true,
    retention:
      typeof value.retention === "number" &&
      [100, 500, 1000].includes(value.retention)
        ? value.retention
        : 500,
    theme: value.theme === "System" ? "System" : "Dark",
  };
}
export function parseFavorites(value: unknown): number[] {
  if (!Array.isArray(value)) return [3000, 5173, 5432];
  return [
    ...new Set(
      value.filter(
        (p): p is number =>
          typeof p === "number" && Number.isInteger(p) && p > 0 && p <= 65535,
      ),
    ),
  ]
    .slice(0, 1000)
    .sort((a, b) => a - b);
}
export function parseHistory(value: unknown): HistoryEvent[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter(
      (e): e is HistoryEvent =>
        record(e) &&
        typeof e.id === "string" &&
        typeof e.time === "number" &&
        Number.isFinite(e.time) &&
        typeof e.port === "number" &&
        Number.isInteger(e.port) &&
        e.port > 0 &&
        e.port <= 65535 &&
        typeof e.process === "string" &&
        (e.type === "started" || e.type === "stopped"),
    )
    .slice(0, 1000);
}
