import { invoke } from "@tauri-apps/api/core";
import { demoPorts } from "./demo";
import { desktop } from "./api";
import type { PortEntry } from "./types";
export type ConflictAction =
  | "kill_retry"
  | "force_retry"
  | "alternate"
  | "restart_owner"
  | "retry"
  | "ignore";
export type ConflictStatus =
  | "detected"
  | "resolving"
  | "force_required"
  | "starting"
  | "resolved"
  | "failed"
  | "unverified"
  | "ignored"
  | "expired";
export interface SafetyAssessment {
  category:
    | "DEV_SERVER"
    | "USER_PROCESS"
    | "INFRASTRUCTURE"
    | "SYSTEM_PROCESS"
    | "UNKNOWN";
  risk: "LOW" | "MEDIUM" | "HIGH" | "BLOCKED";
  heading: string;
  reasons: string[];
  project: string | null;
  projectPath: string | null;
  differentProject: boolean;
  previouslyObserved: boolean;
  possiblyStale: boolean;
}
export interface Conflict {
  id: string;
  port: number;
  command: string[];
  cwd: string;
  project: string;
  owner: PortEntry | null;
  safety: SafetyAssessment;
  status: ConflictStatus;
  message: string;
  evidence: string;
  steps: string[];
  output: string;
  alternative: number | null;
  alternateReason: string;
  restartOwnerAvailable: boolean;
  restartOwnerReason: string;
  launchedPid: number | null;
  recoveryPort: number | null;
  recoveryAction: ConflictAction | null;
  retryCommand: string[] | null;
  listener: PortEntry | null;
  createdAt: number;
  expiresAt: number;
}
export interface AutopilotSnapshot {
  enabled: boolean;
  supported: boolean;
  connectionError: string | null;
  setupCommand: string;
  conflicts: Conflict[];
}
export const isPending = (c: Conflict) =>
  ["detected", "failed", "force_required"].includes(c.status);
export const needsConfirmation = (c: Conflict, action: ConflictAction) =>
  action === "force_retry" ||
  action === "restart_owner" ||
  (action === "kill_retry" && c.safety.risk !== "LOW");
export function commandText(argv: string[]) {
  return argv
    .map((s) =>
      /^[a-zA-Z0-9_./:=@-]+$/.test(s) ? s : `'${s.replaceAll("'", "'\"'\"'")}'`,
    )
    .join(" ");
}
let preview: AutopilotSnapshot = {
  enabled: false,
  supported: true,
  connectionError: null,
  setupCommand:
    "# Open the desktop app to copy setup for your bash or zsh session.",
  conflicts: [],
};
let unresponsive = false;
export async function getAutopilot(): Promise<AutopilotSnapshot> {
  return desktop ? invoke("autopilot_snapshot") : structuredClone(preview);
}
export async function enableAutopilot(enabled: boolean): Promise<void> {
  if (desktop) await invoke("autopilot_enable", { enabled });
  else {
    preview.enabled = enabled;
    if (!enabled) preview.conflicts = [];
  }
}
export async function actOnConflict(
  id: string,
  action: ConflictAction,
  approved = false,
  targetPort: number | null = null,
): Promise<void> {
  if (desktop)
    return invoke("autopilot_action", { id, action, approved, targetPort });
  const c = preview.conflicts.find((c) => c.id === id);
  if (!c) throw new Error("This preview conflict no longer exists.");
  if (action === "ignore") {
    preview.conflicts = preview.conflicts.filter((c) => c.id !== id);
    return;
  }
  if (needsConfirmation(c, action) && !approved)
    throw new Error("Confirmation required.");
  if (action === "restart_owner") throw new Error(c.restartOwnerReason);
  if (action !== "alternate" && c.safety.risk === "BLOCKED")
    throw new Error("This process is protected.");
  c.recoveryAction = action;
  c.status = "resolving";
  c.steps = ["Verified the expected owner and captured command"];
  await new Promise((resolve) => setTimeout(resolve, 350));
  if (action === "kill_retry" && unresponsive) {
    c.status = "force_required";
    c.message =
      "The sample owner did not release the port after graceful termination. Explicitly confirm Force Kill & Retry to continue.";
    c.steps.push("Sent graceful termination; the owner is still listening");
    return;
  }
  const port = action === "alternate" ? targetPort : c.port;
  c.steps.push(
    action === "alternate"
      ? `Verified alternate port ${port}; existing owner left running`
      : `Confirmed port ${port} is free`,
  );
  c.status = "starting";
  c.message = "Preview: retrying the captured command…";
  await new Promise((resolve) => setTimeout(resolve, 400));
  c.steps.push(
    "Retried the captured command with its original context",
    `Verified the new process owns port ${port}`,
  );
  c.status = "resolved";
  c.message = `Preview: your retried command is listening on port ${port}. No real process was affected.`;
  c.launchedPid = 64001;
  c.recoveryPort = port;
  c.retryCommand =
    action === "alternate"
      ? [...c.command, "--", "--port", String(port)]
      : c.command;
  c.listener = c.owner
    ? {
        ...c.owner,
        id: "preview-recovered",
        pid: 64001,
        port: port ?? c.port,
        startedAt: Math.floor(Date.now() / 1000),
        cwd: c.cwd,
      }
    : null;
  c.output = `> npm run dev\nVITE ready\nLocal: http://localhost:${port}/`;
}
export type PreviewScenario = "dev" | "database" | "unresponsive";
export async function simulateConflict(
  scenario: PreviewScenario,
): Promise<void> {
  if (desktop)
    throw new Error(
      "Sample conflicts are only available in the browser preview.",
    );
  const database = scenario === "database";
  unresponsive = scenario === "unresponsive";
  const source = demoPorts.find((p) => p.port === (database ? 5432 : 5173))!;
  const owner = {
    ...source,
    id: `autopilot-${source.id}`,
    cwd: "/Users/developer/Code/old-dashboard",
    startedAt: Math.floor(Date.now() / 1000) - 11640,
  };
  preview.enabled = true;
  preview.conflicts = [
    {
      id: "preview-conflict",
      port: source.port,
      command: ["npm", "run", "dev"],
      cwd: "/Users/developer/Code/new-dashboard",
      project: "new-dashboard",
      owner,
      safety: {
        category: database ? "INFRASTRUCTURE" : "DEV_SERVER",
        risk: database ? "HIGH" : "MEDIUM",
        heading: database
          ? "Caution: infrastructure service"
          : "Looks like a development server",
        reasons: database
          ? [
              "This process appears to be a PostgreSQL database. Stopping it may interrupt other applications. Explicit confirmation is required.",
            ]
          : [
              "Recognized from the command arguments. This is a heuristic, not a guarantee.",
              "This owner belongs to another project. Confirm it is safe to interrupt.",
              "Possibly left over: running for over three hours in another project. Age alone is not proof that it is stale.",
            ],
        project: "old-dashboard",
        projectPath: owner.cwd,
        differentProject: true,
        previouslyObserved: true,
        possiblyStale: !database,
      },
      status: "detected",
      message: `new-dashboard could not start because port ${source.port} is already in use.`,
      evidence: `Error: listen EADDRINUSE: address already in use :::${source.port}`,
      steps: [],
      output: "",
      alternative: database ? null : 5174,
      alternateReason: database
        ? "No supported alternate-port adapter for this example."
        : "The Vite script supports an explicit --port option.",
      restartOwnerAvailable: false,
      restartOwnerReason:
        "The owner’s original environment was not captured. Restart cannot be reproduced safely.",
      launchedPid: null,
      recoveryPort: null,
      recoveryAction: null,
      retryCommand: null,
      listener: null,
      createdAt: Math.floor(Date.now() / 1000),
      expiresAt: Math.floor(Date.now() / 1000) + 900,
    },
  ];
}
