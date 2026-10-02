import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  CommandRun,
  HistoricalCommand,
  HistoryQuery,
  LaunchContext,
  RecoveryStatus,
  RunHistoryPage,
} from "./types";

const now = Math.floor(Date.now() / 1000);
const desktop = isTauri();
const previewContexts: LaunchContext[] = [
  {
    id: "preview-shipwreck-dev",
    source: "shell_observed",
    confidence: "HIGH",
    kind: "direct_process",
    command: "npm run dev",
    executable: "/usr/local/bin/npm",
    args: ["run", "dev"],
    workingDirectory: "/Users/developer/Code/shipwreck",
    environment: [{ name: "NODE_ENV", value: "development" }],
    shell: "zsh",
    projectId: "/Users/developer/Code/shipwreck",
    launchRoot: { pid: 48270, startedAt: now - 7200 },
    capturedAt: now - 7200,
    recoverable: true,
    reason: "",
    relaunchedAt: null,
    fingerprint: "preview-shipwreck-dev-fingerprint",
  },
  {
    id: "preview-plant-dev",
    source: "shell_observed",
    confidence: "HIGH",
    kind: "direct_process",
    command: "pnpm dev",
    executable: "/usr/local/bin/pnpm",
    args: ["dev"],
    workingDirectory: "/Users/developer/Code/plant-journal",
    environment: [],
    shell: "zsh",
    projectId: "/Users/developer/Code/plant-journal",
    launchRoot: { pid: 47002, startedAt: now - 94_000 },
    capturedAt: now - 94_000,
    recoverable: true,
    reason: "",
    relaunchedAt: null,
    fingerprint: "preview-plant-dev-fingerprint",
  },
];

let previewRuns: CommandRun[] = [
  {
    id: "preview-run-live",
    launchContextId: previewContexts[0].id,
    fingerprint: previewContexts[0].fingerprint!,
    startedAt: now - 5200,
    endedAt: null,
    exitCode: null,
    terminationReason: null,
    state: "RUNNING",
    projectId: previewContexts[0].projectId,
    projectName: "Shipwreck",
    processName: "Vite",
    observedPorts: [{ port: 5173, protocol: "TCP", address: "127.0.0.1" }],
    processIdentity: { pid: 48291, startedAt: now - 5200 },
  },
  {
    id: "preview-run-shipwreck-old",
    launchContextId: previewContexts[0].id,
    fingerprint: previewContexts[0].fingerprint!,
    startedAt: now - 90_000,
    endedAt: now - 85_080,
    exitCode: null,
    terminationReason: "Stopped by user",
    state: "STOPPED",
    projectId: previewContexts[0].projectId,
    projectName: "Shipwreck",
    processName: "Vite",
    observedPorts: [{ port: 5173, protocol: "TCP", address: "127.0.0.1" }],
    processIdentity: { pid: 45191, startedAt: now - 90_000 },
  },
  {
    id: "preview-run-plant",
    launchContextId: previewContexts[1].id,
    fingerprint: previewContexts[1].fingerprint!,
    startedAt: now - 96_000,
    endedAt: now - 93_780,
    exitCode: null,
    terminationReason: "Process exited",
    state: "STOPPED",
    projectId: previewContexts[1].projectId,
    projectName: "Plant Journal",
    processName: "Vite",
    observedPorts: [{ port: 5173, protocol: "TCP", address: "127.0.0.1" }],
    processIdentity: { pid: 44012, startedAt: now - 96_000 },
  },
];
let previewPinned: Record<number, string> = {};

export function recordPreviewRunStopped(port: number) {
  const stoppedAt = Math.floor(Date.now() / 1000);
  previewRuns = previewRuns.map((run) =>
    run.state === "RUNNING" &&
    run.observedPorts.some((binding) => binding.port === port)
      ? {
          ...run,
          state: "STOPPED" as const,
          endedAt: stoppedAt,
          terminationReason: "Stopped by user",
        }
      : run,
  );
}

export async function queryRunHistory(
  query: HistoryQuery = {},
): Promise<RunHistoryPage> {
  if (desktop) {
    const page = await invoke<RunHistoryPage | null>("recovery_history", {
      query,
    });
    if (!page)
      throw new Error("Run history database did not return a response.");
    return page;
  }
  const contexts = new Map(
    previewContexts.map((context) => [context.id, context]),
  );
  const search = query.search?.toLowerCase() ?? "";
  let runs = previewRuns
    .filter((run) => !query.since || run.startedAt >= query.since)
    .filter((run) => !query.state || run.state === query.state)
    .filter(
      (run) =>
        !query.port ||
        run.observedPorts.some((port) => port.port === query.port),
    )
    .filter((run) => {
      const context = contexts.get(run.launchContextId)!;
      const haystack =
        `${run.projectName} ${run.processName} ${context.command} ${context.workingDirectory} ${run.observedPorts.map((port) => port.port).join(" ")}`.toLowerCase();
      return search
        .split(/\s+/)
        .filter(Boolean)
        .every((term) => haystack.includes(term));
    })
    .sort((a, b) => b.startedAt - a.startedAt)
    .slice(0, query.limit ?? 250);
  const grouped = new Map<string, CommandRun[]>();
  for (const run of runs)
    grouped.set(run.fingerprint, [
      ...(grouped.get(run.fingerprint) ?? []),
      run,
    ]);
  const commands: HistoricalCommand[] = [...grouped.values()].map((group) => {
    const context = contexts.get(group[0].launchContextId)!;
    return {
      launchContext: context,
      latestRun: group[0],
      runCount: group.length,
      typicalPorts: [
        ...new Set(
          group.flatMap((run) => run.observedPorts.map((p) => p.port)),
        ),
      ],
      pinnedPorts: Object.entries(previewPinned)
        .filter(([, id]) => id === context.id)
        .map(([port]) => Number(port)),
      active: group.some((run) => run.state === "RUNNING"),
    };
  });
  return { commands, runs, storageError: null };
}

export async function runHistoricalCommand(
  id: string,
  confirmed = false,
): Promise<RecoveryStatus> {
  if (desktop) return invoke("recovery_run_again", { id, confirmed });
  const context = previewContexts.find((candidate) => candidate.id === id);
  if (!context) throw new Error("Launch context unavailable");
  if (historicalCommandNeedsConfirmation(context) && !confirmed)
    throw new Error(
      "Confirmation required: review this recovered command before running it.",
    );
  const active = previewRuns.find(
    (run) => run.launchContextId === id && run.state === "RUNNING",
  );
  if (active)
    throw new Error(
      `This command appears to already be running. Open or Restart it instead: ${context.command}`,
    );
  const ports = previewRuns
    .filter((run) => run.launchContextId === id)
    .flatMap((run) => run.observedPorts);
  const occupied = previewRuns.find(
    (run) =>
      run.state === "RUNNING" &&
      run.launchContextId !== id &&
      run.observedPorts.some((binding) =>
        ports.some((expected) => expected.port === binding.port),
      ),
  );
  if (occupied)
    throw new Error(
      `Cannot run ${context.command} on :${occupied.observedPorts[0].port}. ${occupied.projectName ?? occupied.processName} currently owns the port. Inspect the conflict in Conflict Autopilot.`,
    );
  const run: CommandRun = {
    id: crypto.randomUUID(),
    launchContextId: id,
    fingerprint: context.fingerprint ?? id,
    startedAt: Math.floor(Date.now() / 1000),
    endedAt: null,
    exitCode: null,
    terminationReason: null,
    state: "RUNNING",
    projectId: context.projectId,
    projectName:
      previewRuns.find((candidate) => candidate.launchContextId === id)
        ?.projectName ?? null,
    processName:
      previewRuns.find((candidate) => candidate.launchContextId === id)
        ?.processName ?? null,
    observedPorts: ports.slice(0, 1),
    processIdentity: { pid: 59001, startedAt: Math.floor(Date.now() / 1000) },
  };
  previewRuns = [run, ...previewRuns];
  return {
    state: "RUNNING",
    message: `${context.command} started. Verified ports ${ports.map((port) => `:${port.port}`).join(", ")}.`,
    oldPid: 0,
    newPid: 59001,
    ports: ports.map((port) => port.port),
    output: "",
  };
}

export function historicalCommandNeedsConfirmation(context: LaunchContext) {
  if (context.source === "user_defined") return false;
  const executable = context.command
    .trim()
    .split(/\s+/, 1)[0]
    .split("/")
    .at(-1);
  if (["npm", "pnpm", "yarn", "bun"].includes(executable ?? ""))
    return context.args.length === 0;
  if (executable === "cargo") return context.args[0] !== "run";
  if (executable?.startsWith("python")) return context.args.length === 0;
  if (["node", "nodejs"].includes(executable ?? ""))
    return !context.args[0]?.match(
      /(?:npm-cli\.js|pnpm\.cjs|yarn\.(?:js|cjs))$/,
    );
  return true;
}

export async function pinHistoricalCommand(
  port: number,
  id: string,
  pinned: boolean,
) {
  if (desktop) return invoke("recovery_pin_command", { port, id, pinned });
  if (pinned) previewPinned[port] = id;
  else if (previewPinned[port] === id) delete previewPinned[port];
}

export async function removeHistoricalRun(id: string) {
  if (desktop) return invoke("recovery_remove_run", { id });
  previewRuns = previewRuns.filter((run) => run.id !== id);
}
