export interface ProcessIdentity {
  pid: number;
  startedAt: number;
}
export interface DisplayLaunchContext {
  schemaVersion?: number;
  runId?: string | null;
  environmentStrategy?: string | null;
  recoveryConfidence?:
    "exact" | "observed" | "recovered" | "inferred" | "unavailable";
  id: string;
  source:
    "shell_observed" | "parent_process" | "process_inspection" | "user_defined";
  confidence: "HIGH" | "MEDIUM" | "LOW" | "UNKNOWN";
  kind: "direct_process" | "shell_command";
  command: string;
  executable: string;
  args: string[];
  workingDirectory: string;
  environment: { name: string; value: string | null }[];
  shell: string | null;
  projectId: string | null;
  launchRoot: ProcessIdentity;
  capturedAt: number;
  recoverable: boolean;
  reason: string;
  relaunchedAt: number | null;
  fingerprint?: string;
}
export interface LaunchProfile {
  projectId: string;
  name: string;
  executable: string;
  args: string[];
  workingDirectory: string;
}
export interface RecoveryStatus {
  state: string;
  message: string;
  oldPid: number;
  newPid: number | null;
  ports: number[];
  output: string;
}
export type CommandRunState =
  "RUNNING" | "COMPLETED" | "FAILED" | "STOPPED" | "UNVERIFIED";
export interface RunPort {
  port: number;
  protocol: string;
  address: string;
}
export interface CommandRun {
  id: string;
  launchContextId: string;
  fingerprint: string;
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  terminationReason: string | null;
  state: CommandRunState;
  projectId: string | null;
  projectName: string | null;
  processName: string | null;
  observedPorts: RunPort[];
  processIdentity: ProcessIdentity | null;
}
export interface HistoricalCommand {
  launchContext: DisplayLaunchContext;
  latestRun: CommandRun;
  runCount: number;
  typicalPorts: number[];
  pinnedPorts: number[];
  active: boolean;
}
export interface RunHistoryPage {
  commands: HistoricalCommand[];
  runs: CommandRun[];
  storageError: string | null;
}
export interface HistoryQuery {
  search?: string;
  state?: string;
  since?: number;
  port?: number;
  limit?: number;
}
export const launchSources = {
  shell_observed: "Observed from terminal",
  parent_process: "Recovered from parent process",
  process_inspection: "Recovered from process metadata",
  user_defined: "Project recovery command",
};
