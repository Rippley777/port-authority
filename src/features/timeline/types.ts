import type { PortEntry } from "../../lib/types";
export interface ProcessIdentity {
  pid: number;
  startedAt: number;
}
export interface Ancestor {
  identity: ProcessIdentity;
  name: string;
  command: string[];
  cwd: string | null;
  parentPid: number | null;
}
export interface Observation {
  process: PortEntry;
  ancestors: Ancestor[];
}
export type EventType =
  | "PORT_CLAIMED"
  | "PORT_RELEASED"
  | "OWNER_CHANGED"
  | "PROCESS_RESTARTED"
  | "PORT_RECLAIMED"
  | "OBSERVED";
export interface TimelineEvent {
  id: string;
  sequence: number;
  timestamp: number;
  sessionId: string;
  port: number;
  protocol: string;
  address: string;
  eventType: EventType;
  process: Observation | null;
  previousProcess: Observation | null;
  uncertain: boolean;
  endpointAvailable?: boolean;
  correlationId: string | null;
}
export interface MonitoringSession {
  id: string;
  startedAt: number;
  lastObserved: number;
  endedAt: number | null;
  reason: string;
}
export interface TimelineConfig {
  enabled: boolean;
  retentionDays: number;
  reclaimWindowSeconds: number;
  reclaimThreshold: number;
}
export interface Recurrence {
  sessionId: string;
  port: number;
  protocol: string;
  address: string;
  count: number;
  firstAt: number;
  latestAt: number;
  persistentAncestor: Ancestor | null;
  process: Observation;
  previousPids: number[];
}
export interface TimelinePage {
  events: TimelineEvent[];
  sessions: MonitoringSession[];
  recurring: Recurrence[];
  nextCursor: number | null;
  config: TimelineConfig;
  databaseBytes: number;
  error: string | null;
  monitoring: boolean;
}
export interface TimelineQuery {
  port?: number;
  search?: string;
  eventType?: string;
  since?: number;
  before?: number;
  limit?: number;
}
export interface TreeMember {
  identity: ProcessIdentity;
  name: string;
  parentPid: number | null;
  command: string[];
}
export const eventLabels: Record<EventType, string> = {
  PORT_CLAIMED: "Port claimed",
  PORT_RELEASED: "Port released",
  OWNER_CHANGED: "Owner changed",
  PROCESS_RESTARTED: "Process restarted",
  PORT_RECLAIMED: "Port reclaimed",
  OBSERVED: "Owner observed",
};
