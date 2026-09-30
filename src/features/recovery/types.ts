export interface ProcessIdentity {
  pid: number;
  startedAt: number;
}
export interface LaunchContext {
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
export const launchSources = {
  shell_observed: "Observed from terminal",
  parent_process: "Recovered from parent process",
  process_inspection: "Recovered from process metadata",
  user_defined: "Project recovery command",
};
