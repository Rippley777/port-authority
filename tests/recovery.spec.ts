import { expect, test } from "@playwright/test";
import { demoPorts } from "../src/lib/demo";
import type { PortEntry } from "../src/lib/types";
const base = demoPorts.find((p) => p.port === 5173)!;
const entry: PortEntry = {
  ...base,
  restartable: true,
  restartReason: "",
  launch: {
    id: "launch-1",
    source: "shell_observed",
    confidence: "HIGH",
    kind: "direct_process",
    command: "npm run dev",
    executable: "/usr/local/bin/npm",
    args: ["run", "dev"],
    workingDirectory: "/Users/developer/Code/shipwreck/apps/web",
    environment: [
      { name: "NODE_ENV", value: "development" },
      { name: "API_TOKEN", value: null },
    ],
    shell: null,
    projectId: base.project!.rootPath,
    launchRoot: { pid: 48270, startedAt: base.startedAt! },
    capturedAt: base.startedAt!,
    recoverable: true,
    reason: "",
    relaunchedAt: null,
  },
};
async function mockDesktop(
  page: import("@playwright/test").Page,
  fail = false,
) {
  await page.addInitScript(
    ({ entry, fail }) => {
      let status: unknown = null;
      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: {
          transformCallback: () => 1,
          invoke: async (cmd: string, args: Record<string, unknown>) => {
            if (cmd === "scan_ports") return [entry];
            if (cmd === "projects_snapshot")
              return { projects: [], resolving: false, storageError: null };
            if (cmd === "project_applications")
              return { editors: [], terminals: [] };
            if (cmd === "autopilot_snapshot")
              return {
                enabled: false,
                supported: true,
                conflicts: [],
                setupCommand: "",
                connectionError: null,
              };
            if (cmd === "recovery_history")
              return {
                commands: [
                  {
                    launchContext: entry.launch,
                    active: true,
                    runCount: 1,
                    typicalPorts: [5173],
                    pinnedPorts: [],
                    latestRun: {
                      id: "run-1",
                      launchContextId: "launch-1",
                      fingerprint: "launch-1",
                      startedAt: entry.startedAt,
                      endedAt: null,
                      state: "RUNNING",
                      projectName: "Shipwreck",
                      processName: entry.process,
                      projectId: entry.project?.rootPath,
                      observedPorts: [],
                      exitCode: null,
                      terminationReason: null,
                      processIdentity: null,
                    },
                  },
                ],
                runs: [],
                storageError: null,
              };
            if (cmd === "timeline_query")
              return {
                events: [
                  {
                    id: "event-1",
                    sequence: 1,
                    timestamp: entry.startedAt,
                    sessionId: "session-1",
                    port: 5173,
                    protocol: "TCP",
                    address: entry.address,
                    eventType: "PORT_CLAIMED",
                    process: { process: entry, ancestors: [] },
                    previousProcess: null,
                    uncertain: false,
                    correlationId: null,
                  },
                ],
                sessions: [],
                recurring: [],
                nextCursor: null,
                databaseBytes: 0,
                error: null,
                monitoring: true,
                config: {
                  enabled: true,
                  retentionDays: 30,
                  reclaimWindowSeconds: 600,
                  reclaimThreshold: 3,
                },
              };
            if (cmd === "recovery_terminal") {
              if (args.directory !== entry.launch!.workingDirectory)
                throw new Error("Wrong launch directory");
              return null;
            }
            if (cmd === "recovery_profile") return null;
            if (cmd === "recovery_status") return status;
            if (cmd === "recovery_restart") {
              if (
                args.id !== "launch-1" ||
                (args.identity as { pid: number }).pid !== entry.pid ||
                args.port !== 5173
              )
                throw new Error("Incorrect restart identity");
              status = {
                state: "VERIFYING_PORT",
                message:
                  "Waiting for the new process tree to reopen every expected port…",
                oldPid: entry.pid,
                newPid: 49102,
                ports: [5173],
                output: "Vite starting",
              };
              await new Promise((r) => setTimeout(r, 1200));
              if (fail) {
                status = {
                  state: "EXPECTED_PORT_NOT_OPENED",
                  message: "Process relaunched, but port 5173 did not reopen.",
                  oldPid: entry.pid,
                  newPid: 49102,
                  ports: [5173],
                  output: "Fixture startup error",
                };
                throw new Error(
                  "Process relaunched, but port 5173 did not reopen.",
                );
              }
              status = {
                state: "RUNNING",
                message:
                  "Restarted successfully. PID 48291 → 49102. Verified port 5173.",
                oldPid: entry.pid,
                newPid: 49102,
                ports: [5173],
                output: "Vite ready",
              };
              return status;
            }
            return null;
          },
        },
      });
      localStorage.setItem("pa-timeline-migrated", "1");
    },
    { entry, fail },
  );
}
test("unknown launches keep Restart disabled and omit it from the context menu", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("row", { name: "Inspect vite on port 5173" }).click();
  const drawer = page.getByRole("dialog", { name: "Process details for vite" });
  await expect(drawer.getByLabel("Launch context")).toContainText(
    "Original launch command could not be determined",
  );
  await expect(
    drawer.getByRole("button", { name: "Restart", exact: true }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await page
    .getByRole("button", { name: "More actions for port 5173" })
    .click();
  await expect(
    page.getByRole("menuitem", { name: "Restart", exact: true }),
  ).toHaveCount(0);
});
test("observed launch shows its root, redacted environment and verified restart result", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto("/");
  await page
    .getByRole("button", { name: "More actions for port 5173" })
    .click();
  await expect(
    page.getByRole("menuitem", { name: "Open Terminal Here", exact: true }),
  ).toHaveCount(1);
  await page
    .getByRole("menuitem", { name: "Open Terminal Here", exact: true })
    .click();
  await expect(page.getByRole("menu")).toBeHidden();
  await page.getByRole("row", { name: "Inspect vite on port 5173" }).click();
  const drawer = page.getByRole("dialog", { name: "Process details for vite" });
  await expect(drawer.getByLabel("Launch context")).toContainText(
    "npm run dev",
  );
  await expect(drawer.getByLabel("Launch context")).toContainText(
    "Observed from terminal",
  );
  await drawer.locator("summary").click();
  await expect(drawer.getByLabel("Launch context")).toContainText("••••••••");
  await drawer.getByRole("button", { name: "Restart", exact: true }).click();
  const confirm = page.getByRole("dialog", {
    name: "Confirm command recovery",
  });
  await expect(confirm).toContainText("Launch root: PID 48270");
  await confirm.getByRole("button", { name: "Restart", exact: true }).click();
  await expect(confirm).toContainText("Waiting for the new process tree");
  await expect(
    page.getByText(
      "Restarted successfully. PID 48291 → 49102. Verified port 5173.",
    ),
  ).toBeVisible();
});
test("failed relaunch remains inspectable and exposes output without claiming success", async ({
  page,
}) => {
  await mockDesktop(page, true);
  await page.goto("/");
  await page.getByRole("row", { name: "Inspect vite on port 5173" }).click();
  await page.getByRole("button", { name: "Restart", exact: true }).click();
  const confirm = page.getByRole("dialog", {
    name: "Confirm command recovery",
  });
  await confirm.getByRole("button", { name: "Restart", exact: true }).click();
  await expect(confirm).toContainText("did not reopen");
  await confirm.getByRole("button", { name: "View Output" }).click();
  await expect(confirm).toContainText("Fixture startup error");
  await expect(
    page.getByText("Restarted successfully", { exact: false }),
  ).toHaveCount(0);
});

for (const surface of [
  "Ports",
  "Processes",
  "Favorites",
  "Run History",
  "Run details",
  "Port Timeline",
] as const) {
  test(`${surface} restarts the live launch on its original port`, async ({
    page,
  }) => {
    await mockDesktop(page);
    await page.goto("/");
    const navigation = page.getByRole("navigation", {
      name: "Main navigation",
    });
    if (surface === "Processes" || surface === "Favorites")
      await navigation
        .getByRole("button", { name: new RegExp(surface) })
        .click();
    if (["Run History", "Run details", "Port Timeline"].includes(surface)) {
      await navigation
        .getByRole("button", { name: "History", exact: true })
        .click();
      if (surface === "Port Timeline") {
        await page.getByRole("tab", { name: "Port Timeline" }).click();
        await page
          .getByRole("button", { name: "Restart running command" })
          .click();
      } else {
        const card = page
          .locator(".command-history-list .command-history-card")
          .first();
        if (surface === "Run details") {
          await card.getByRole("button", { name: "Details" }).click();
          await page
            .getByRole("dialog")
            .getByRole("button", { name: "Restart", exact: true })
            .click();
        } else
          await card
            .getByRole("button", { name: "Restart", exact: true })
            .click();
      }
    } else if (surface === "Favorites") {
      await page
        .locator(".favorite-memory-card")
        .filter({ hasText: ":5173" })
        .getByRole("button", { name: "Restart", exact: true })
        .click();
    } else
      await page
        .getByRole("button", {
          name: "Restart service on port 5173",
          exact: true,
        })
        .click();
    const confirm = page.getByRole("dialog", {
      name: "Confirm command recovery",
    });
    await expect(confirm).toContainText("5173");
    await confirm.getByRole("button", { name: "Restart", exact: true }).click();
    await expect(
      page.getByText(
        "Restarted successfully. PID 48291 → 49102. Verified port 5173.",
      ),
    ).toBeVisible();
  });
}
