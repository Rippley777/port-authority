export type Protocol = "TCP" | "UDP";
export interface PortEntry {
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
  id: string;
  time: number;
  port: number;
  process: string;
  type: "started" | "stopped";
}
export interface Settings {
  refreshInterval: number;
  showSystem: boolean;
  confirmKill: boolean;
  protocol: "http" | "https";
  keepHistory: boolean;
  retention: number;
  theme: "Dark" | "System";
}
export const defaultSettings: Settings = {
  refreshInterval: 2,
  showSystem: true,
  confirmKill: true,
  protocol: "http",
  keepHistory: true,
  retention: 500,
  theme: "Dark",
};
export type ProcessAction = "kill" | "force" | "restart";
