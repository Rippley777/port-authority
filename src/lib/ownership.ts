import type {
  MonitoringSession,
  Observation,
  TimelineEvent,
} from "../features/timeline/types";
export interface OwnershipSession {
  id: string;
  port: number;
  endpoint: string;
  startedAt: number;
  endedAt: number;
  owner: Observation | null;
  uncertainStart: boolean;
}
// Derive only intervals bounded by observations in the same monitoring session.
export function ownershipSessions(
  events: TimelineEvent[],
  monitoring: MonitoringSession[],
): OwnershipSession[] {
  const result: OwnershipSession[] = [];
  const active = new Map<string, OwnershipSession>();
  for (const e of [...events].sort((a, b) => a.sequence - b.sequence)) {
    const endpoint = `${e.protocol} · ${e.address}:${e.port}`;
    const key = `${e.sessionId}:${endpoint}`;
    const previous = e.previousProcess?.process;
    if (previous) {
      const identity = `${key}:${previous.pid}:${previous.startedAt}`;
      const interval = active.get(identity);
      if (interval) {
        interval.endedAt = Math.max(interval.startedAt, e.timestamp);
        active.delete(identity);
      }
    }
    if (e.process) {
      const free = active.get(`${key}:available`);
      if (free) {
        free.endedAt = Math.max(free.startedAt, e.timestamp);
        active.delete(`${key}:available`);
      }
      const p = e.process.process;
      const session = monitoring.find((s) => s.id === e.sessionId);
      const interval: OwnershipSession = {
        id: e.id,
        port: e.port,
        endpoint,
        startedAt: e.timestamp,
        endedAt: Math.max(e.timestamp, session?.lastObserved ?? e.timestamp),
        owner: e.process,
        uncertainStart: e.uncertain,
      };
      active.set(`${key}:${p.pid}:${p.startedAt}`, interval);
      result.push(interval);
    } else if (
      e.eventType === "PORT_RELEASED" &&
      !e.uncertain &&
      e.endpointAvailable &&
      ![...active.keys()].some((k) => k.startsWith(`${key}:`))
    ) {
      const session = monitoring.find((s) => s.id === e.sessionId);
      const free: OwnershipSession = {
        id: e.id,
        port: e.port,
        endpoint,
        startedAt: e.timestamp,
        endedAt: Math.max(e.timestamp, session?.lastObserved ?? e.timestamp),
        owner: null,
        uncertainStart: false,
      };
      active.set(`${key}:available`, free);
      result.push(free);
    }
  }
  return result.reverse();
}
