import type { ProjectIdentity } from "../features/projects/types";
export type Protocol = "TCP" | "UDP";
export type MetadataAccessState =
  | "available"
  | "permissionDenied"
  | "protectedResource"
  | "unsupported"
  | "unknown";
export interface PortEntry {
  metadataAccess?: {
    cwd: MetadataAccessState;
    executable: MetadataAccessState;
    command: MetadataAccessState;
  } | null;
  launch?: import("../features/recovery/types").LaunchContext | null;
  project?: ProjectIdentity | null;
  serviceName?: string | null;
  id: string;
  port: number;
  protocol: Protocol;
  address: string;
  pid: number | null;
  process: string;
  command: string[];
  executable: string | null;
  cwd: string | null;
  parentPid: number | null;
  user: string | null;
  startedAt: number | null;
  memory: number | null;
  cpu: number | null;
  system: boolean;
  protected: boolean;
  restartable: boolean;
  restartReason: string;
  permissionLimited: boolean;
}
export type Page =
  | "Conflict Autopilot"
  | "Projects"
  | "Overview"
  | "Ports"
  | "Processes"
  | "Favorites"
  | "History"
  | "Check Port"
  | "Settings";
export type Filter =
  | "All"
  | "TCP"
  | "UDP"
  | "Listening"
  | "Localhost"
  | "External"
  | "System"
  | "User";
export type SortKey = "port" | "protocol" | "process" | "pid" | "startedAt";
export interface HistoryEvent {
  processPid?: number | null;
  processStartedAt?: number | null;
  projectName?: string;
  projectPath?: string;
  id: string;
  time: number;
  port: number;
  process: string;
  type: "started" | "stopped";
}
export interface Settings {
  preferredEditor: string;
  customEditor: string;
  preferredTerminal: string;
  refreshInterval: number;
  showSystem: boolean;
  confirmKill: boolean;
  protocol: "http" | "https";
  keepHistory: boolean;
  retention: number;
  theme: "Dark" | "System";
}
export const defaultSettings: Settings = {
  preferredEditor: "auto",
  customEditor: "",
  preferredTerminal: "auto",
  refreshInterval: 2,
  showSystem: true,
  confirmKill: true,
  protocol: "http",
  keepHistory: true,
  retention: 500,
  theme: "Dark",
};
export type ProcessAction = "kill" | "force" | "restart" | "relaunch";
