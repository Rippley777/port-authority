import { demoPorts } from "./demo";
import assert from "node:assert/strict";
import { test } from "node:test";
import { activeLaunch, usefulRecentCommands } from "./runHistory";
import type { HistoricalCommand } from "../features/recovery/types";

const entry = (
  id: string,
  state: HistoricalCommand["latestRun"]["state"],
  projectId: string | null,
  recoverable = true,
): HistoricalCommand => ({
  launchContext: {
    id,
    source: "shell_observed",
    confidence: "HIGH",
    kind: "direct_process",
    command: "npm run dev",
    executable: "/usr/local/bin/npm",
    args: ["run", "dev"],
    workingDirectory: projectId ?? "/tmp/work",
    environment: [],
    shell: null,
    projectId,
    launchRoot: { pid: 1, startedAt: 1 },
    capturedAt: 1,
    recoverable,
    reason: "",
    relaunchedAt: null,
  },
  latestRun: {
    id,
    launchContextId: id,
    fingerprint: id,
    startedAt: 10,
    endedAt: null,
    exitCode: null,
    terminationReason: null,
    state,
    projectId,
    projectName: null,
    processName: null,
    observedPorts: [],
    processIdentity: null,
  },
  runCount: 1,
  typicalPorts: [],
  pinnedPorts: [],
  active: state === "RUNNING",
});

test("recent commands prioritize recoverable projects and exclude runtime installations and unavailable launches", () => {
  const commands = [
    entry("live", "RUNNING", "/Code/live"),
    entry("runtime", "STOPPED", "/Users/dev/.nvm"),
    entry("legacy", "STOPPED", "/Code/legacy", false),
    entry("failed", "FAILED", "/Code/failed"),
    entry("stopped", "STOPPED", "/Code/stopped"),
  ];
  assert.deepEqual(
    usefulRecentCommands(commands).map((command) => command.launchContext.id),
    ["stopped", "failed", "live"],
  );
  assert.equal(
    commands[0].launchContext.id,
    "live",
    "ranking must not mutate the main history",
  );
});

test("restart resolves a saved launch to its current process without selecting another service in the same project", () => {
  const context = {
    ...entry("old", "STOPPED", "/Code/app").launchContext,
    fingerprint: "same-command",
  };
  const other = {
    ...demoPorts[0],
    launch: { ...context, id: "other", fingerprint: "other-command" },
  };
  const live = { ...other, pid: 777, launch: { ...context, id: "new" } };
  assert.equal(activeLaunch(context, [other]), undefined);
  assert.equal(activeLaunch(context, [other, live]), live);
  assert.equal(
    activeLaunch({ ...context, fingerprint: undefined }, [live]),
    undefined,
  );
  assert.equal(activeLaunch(live.launch, [live]), live);
  assert.equal(activeLaunch(undefined, [live]), undefined);
});
