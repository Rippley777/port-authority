import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@tauri-apps/api/core";
import { demoPorts } from "../../lib/demo";
import {
  eventLabels,
  type TimelineConfig,
  type TimelineEvent,
  type TimelinePage,
  type TimelineQuery,
  type Ancestor,
} from "./types";
const now = Math.floor(Date.now() / 1000);
export const defaultTimelineConfig: TimelineConfig = {
  enabled: true,
  retentionDays: 30,
  reclaimWindowSeconds: 600,
  reclaimThreshold: 3,
};
let config = { ...defaultTimelineConfig };
try {
  const saved = JSON.parse(
    localStorage.getItem("pa-preview-timeline-config") ?? "null",
  );
  if (saved && [0, 1, 7, 30, 90].includes(saved.retentionDays))
    config = { ...config, ...saved };
} catch {
  /* use defaults */
}
const vite = demoPorts.find((p) => p.port === 5173)!;
const ancestor: Ancestor = {
  identity: { pid: 47001, startedAt: now - 3600 },
  name: "npm",
  command: ["npm", "run", "dev"],
  cwd: vite.cwd,
  parentPid: 46000,
};
const owner = (pid: number) => ({
  process: {
    ...vite,
    pid,
    parentPid: ancestor.identity.pid,
    startedAt: pid === vite.pid ? vite.startedAt : now - 700 + (pid % 100),
  },
  ancestors: [ancestor],
});
let previewEvents: TimelineEvent[] = [];
function seed() {
  let previous = owner(48291);
  for (let i = 0; i < 4; i++) {
    const current = owner(i === 3 ? vite.pid! : 48300 + i);
    if (i > 0)
      previewEvents.push({
        id: `preview-release-${i}`,
        sequence: i * 2,
        timestamp: now - 180 + i * 40 - 3,
        sessionId: "preview-session",
        port: 5173,
        protocol: "TCP",
        address: vite.address,
        eventType: "PORT_RELEASED",
        endpointAvailable: true,
        process: null,
        previousProcess: previous,
        uncertain: false,
        correlationId: null,
      });
    previewEvents.push({
      id: `preview-claim-${i}`,
      sequence: i * 2 + 1,
      timestamp: now - 180 + i * 40,
      sessionId: "preview-session",
      port: 5173,
      protocol: "TCP",
      address: vite.address,
      eventType: i ? "PROCESS_RESTARTED" : "PORT_CLAIMED",
      process: current,
      previousProcess: i ? previous : null,
      uncertain: false,
      correlationId: i ? `preview-release-${i}` : null,
    });
    previous = current;
  }
}
seed();
export async function queryTimeline(q: TimelineQuery): Promise<TimelinePage> {
  if (isTauri()) return invoke("timeline_query", { query: q });
  const since = Math.max(
    q.since ?? 0,
    config.retentionDays
      ? Math.floor(Date.now() / 1000) - config.retentionDays * 86400
      : 0,
  );
  const events = previewEvents
    .filter(
      (e) =>
        (!q.port || e.port === q.port) &&
        (!q.eventType || e.eventType === q.eventType) &&
        e.timestamp >= since &&
        (!q.before || e.sequence < q.before) &&
        (!q.search ||
          `${eventLabels[e.eventType]} ${JSON.stringify(e)} PID ${e.process?.process.pid} PID ${e.previousProcess?.process.pid}`
            .toLowerCase()
            .includes(q.search.toLowerCase())),
    )
    .sort((a, b) => b.sequence - a.sequence);
  const limit = q.limit ?? 100;
  const restarts = previewEvents.filter(
    (e) =>
      e.eventType === "PROCESS_RESTARTED" &&
      e.timestamp >= Date.now() / 1000 - config.reclaimWindowSeconds,
  );
  return {
    events: events.slice(0, limit),
    nextCursor: events.length > limit ? events[limit - 1].sequence : null,
    sessions: [
      {
        id: "preview-session",
        startedAt: now - 240,
        lastObserved: Math.floor(Date.now() / 1000),
        endedAt: null,
        reason: "Monitoring resumed",
      },
    ],
    recurring:
      previewActive &&
      restarts.length >= config.reclaimThreshold &&
      (!q.port || q.port === 5173)
        ? [
            {
              sessionId: "preview-session",
              port: 5173,
              protocol: "TCP",
              address: vite.address,
              count: restarts.length,
              firstAt: restarts[0].timestamp,
              latestAt: restarts.at(-1)!.timestamp,
              persistentAncestor: ancestor,
              process: owner(vite.pid!),
              previousPids: [48300, 48301, 48302, vite.pid!],
            },
          ]
        : [],
    config,
    databaseBytes: 0,
    error: null,
    monitoring: config.enabled,
  };
}
export async function configureTimeline(next: TimelineConfig) {
  if (isTauri()) await invoke("timeline_configure", { config: next });
  else {
    config = next;
    localStorage.setItem("pa-preview-timeline-config", JSON.stringify(next));
  }
}
export async function clearTimeline() {
  if (isTauri()) await invoke("timeline_clear");
  else previewEvents = [];
}

let previewActive = true;
export function recordPreviewRelease(
  entry: import("../../lib/types").PortEntry,
) {
  if (entry.port === 5173) previewActive = false;
  if (!config.enabled) return;
  previewEvents.push({
    id: crypto.randomUUID(),
    sequence: (previewEvents.at(-1)?.sequence ?? 0) + 1,
    timestamp: Math.floor(Date.now() / 1000),
    sessionId: "preview-session",
    port: entry.port,
    protocol: entry.protocol,
    address: entry.address,
    eventType: "PORT_RELEASED",
    process: null,
    previousProcess: {
      process: entry,
      ancestors: entry.port === 5173 ? [ancestor] : [],
    },
    uncertain: false,
    endpointAvailable: true,
    correlationId: null,
  });
}
