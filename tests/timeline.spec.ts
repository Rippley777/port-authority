import { test, expect, type Page } from "@playwright/test";
async function openHistory(page: Page) {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "History", exact: true })
    .click();
}
test("row timeline reveals recurring parent, historic snapshots and scoped filters", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await page
    .getByRole("button", { name: "More actions for port 5173" })
    .click();
  await page.getByRole("menuitem", { name: "View Timeline" }).click();
  await expect(page.getByLabel("Timeline port", { exact: true })).toHaveValue(
    "5173",
  );
  await expect(
    page.getByRole("heading", { name: "Port 5173 keeps returning" }),
  ).toBeVisible();
  await expect(page.locator(".recurrence-panel")).toContainText("npm run dev");
  await expect(page.locator(".recurrence-panel")).toContainText("PID 47001");
  await page
    .getByLabel("Timeline event type")
    .selectOption("PROCESS_RESTARTED");
  await expect(page.locator(".timeline-event")).toHaveCount(3);
  await page.getByLabel("Search history").fill("PID 48302");
  await expect(page.locator(".timeline-event")).toHaveCount(2);
  await page.locator(".timeline-event summary").first().click();
  await expect(
    page
      .locator(".timeline-event")
      .first()
      .locator(".timeline-snapshot")
      .last(),
  ).toContainText("~/Code/shipwreck".replace("~", "/Users/developer"));
  await page.getByLabel("Search history").fill("no-such-project");
  await expect(
    page.getByRole("heading", { name: "No matching activity yet." }),
  ).toBeVisible();
  await page.getByLabel("Search history").fill("");
  await page.getByRole("tab", { name: "Ownership sessions" }).click();
  await expect(page.locator(".ownership-row")).toHaveCount(7);
  await expect(
    page.locator(".ownership-row").filter({ hasText: "Available" }),
  ).toHaveCount(3);
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    ),
  ).toBe(false);
  expect(errors).toEqual([]);
});
test("process tree review requires explicit acknowledgement and preview never kills", async ({
  page,
}) => {
  await openHistory(page);
  await page
    .getByRole("button", { name: "Stop Process Tree…", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Inspect process tree" });
  await expect(dialog).toContainText("npm");
  await expect(dialog).toContainText("PID 47001");
  const stop = dialog.getByRole("button", {
    name: "Confirm Stop Process Tree",
  });
  await expect(stop).toBeDisabled();
  await dialog.getByRole("checkbox").check();
  await stop.click();
  await expect(dialog.getByRole("alert")).toContainText(
    "No real process was affected",
  );
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
test("retention settings persist and clearing is a deliberate action", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Tools" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await page.getByLabel("History retention", { exact: true }).selectOption("7");
  await page.getByLabel("Reclaim detection window").selectOption("900");
  await page.reload();
  await page
    .getByRole("navigation", { name: "Tools" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await expect(
    page.getByLabel("History retention", { exact: true }),
  ).toHaveValue("7");
  await expect(page.getByLabel("Reclaim detection window")).toHaveValue("900");
  await page.getByRole("switch", { name: "Record port history" }).click();
  await expect(
    page.getByRole("switch", { name: "Record port history" }),
  ).toHaveAttribute("aria-checked", "false");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "History", exact: true })
    .click();
  await expect(page.locator(".timeline-event")).toHaveCount(4);
  await page
    .getByRole("button", { name: "Clear History", exact: true })
    .click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator(".timeline-event")).toHaveCount(4);
  await page
    .getByRole("button", { name: "Clear History", exact: true })
    .click();
  await page.getByRole("button", { name: "Delete all history" }).click();
  await expect(
    page.getByRole("heading", { name: "No matching activity yet." }),
  ).toBeVisible();
});
test("autopilot and process drawer link directly to port history", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("row", { name: "Inspect vite on port 5173" }).click();
  await page
    .getByRole("button", { name: "View Timeline", exact: true })
    .click();
  await expect(page.getByLabel("Timeline port", { exact: true })).toHaveValue(
    "5173",
  );
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Autopilot", exact: true })
    .click();
  await page.getByRole("button", { name: "Simulate a conflict" }).click();
  await expect(page.locator(".conflict-history")).toContainText(
    "The same npm PID 47001",
  );
  await page.getByRole("button", { name: "View Port History" }).click();
  await expect(page.getByLabel("Timeline port", { exact: true })).toHaveValue(
    "5173",
  );
});

test("native history paginates, shows monitoring gaps and queues new events without resetting scroll", async ({
  page,
}) => {
  const { demoPorts } = await import("../src/lib/demo");
  const process = demoPorts.find((p) => p.port === 5173)!;
  await page.addInitScript(
    ({ process }) => {
      const now = Math.floor(Date.now() / 1000);
      const make = (sequence: number) => ({
        id: `event-${sequence}`,
        sequence,
        timestamp: now - 1000 + sequence,
        sessionId: sequence > 90 ? "new" : "old",
        port: 5173,
        protocol: "TCP",
        address: "127.0.0.1",
        eventType: sequence === 91 ? "OWNER_CHANGED" : "PORT_CLAIMED",
        process: {
          process: {
            ...process,
            pid: sequence + 1000,
            startedAt: now - 1000 + sequence,
          },
          ancestors: [],
        },
        previousProcess:
          sequence === 91
            ? {
                process: {
                  ...process,
                  project: { ...process.project, name: "Previous project" },
                },
                ancestors: [],
              }
            : null,
        uncertain: sequence === 91,
        correlationId: null,
      });
      const events = Array.from({ length: 105 }, (_, i) => make(i + 1));
      Object.defineProperty(window, "addTimelineEvent", {
        value: () => events.push(make(events.length + 1)),
      });
      Object.defineProperty(window, "isTauri", { value: true });
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          transformCallback: () => 1,
          invoke: async (
            command: string,
            args?: { query?: { before?: number; limit?: number } },
          ) => {
            if (command === "scan_ports") return [process];
            if (command === "projects_snapshot")
              return { projects: [], resolving: false, storageError: null };
            if (command === "project_applications")
              return { editors: [], terminals: [] };
            if (command === "autopilot_snapshot")
              return {
                enabled: false,
                supported: true,
                connectionError: null,
                setupCommand: "",
                conflicts: [],
              };
            if (command === "timeline_query") {
              const filtered = events
                .filter((e) => e.sequence < (args?.query?.before ?? Infinity))
                .sort((a, b) => b.sequence - a.sequence);
              const limit = args?.query?.limit ?? 100;
              return {
                events: filtered.slice(0, limit),
                nextCursor:
                  filtered.length > limit ? filtered[limit - 1].sequence : null,
                sessions: [
                  {
                    id: "old",
                    startedAt: now - 1100,
                    lastObserved: now - 950,
                    endedAt: now - 950,
                    reason: "Monitoring stopped",
                  },
                  {
                    id: "new",
                    startedAt: now - 910,
                    lastObserved: now,
                    endedAt: null,
                    reason: "Monitoring resumed",
                  },
                ],
                recurring: [],
                config: {
                  enabled: true,
                  retentionDays: 30,
                  reclaimWindowSeconds: 600,
                  reclaimThreshold: 3,
                },
                databaseBytes: 32768,
                error: null,
                monitoring: true,
              };
            }
            return null;
          },
        },
      });
      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        value: { unregisterListener: () => {} },
      });
    },
    { process },
  );
  await openHistory(page);
  await expect(page.locator(".timeline-event")).toHaveCount(100);
  await expect(page.locator(".timeline-gap")).toContainText(
    "Monitoring unavailable",
  );
  await expect(
    page
      .locator(".timeline-event")
      .filter({ hasText: "Owner changed while monitoring was offline" }),
  ).toHaveCount(1);
  await page.getByRole("button", { name: "Load older events" }).click();
  await expect(page.locator(".timeline-event")).toHaveCount(105);
  await page.locator(".main-content").evaluate((el) => {
    el.scrollTop = 800;
  });
  await page.evaluate(() =>
    (window as unknown as { addTimelineEvent: () => void }).addTimelineEvent(),
  );
  await expect(
    page.getByRole("button", { name: "Show new activity" }),
  ).toBeAttached();
  expect(
    await page.locator(".main-content").evaluate((el) => el.scrollTop),
  ).toBeGreaterThan(700);
  await expect(page.locator(".timeline-event")).toHaveCount(105);
  await page.getByRole("button", { name: "Show new activity" }).click();
  await expect(page.locator(".timeline-event")).toHaveCount(106);
  const ids = await page
    .locator(".timeline-event .timeline-event-meta .mono")
    .allTextContents();
  expect(new Set(ids).size).toBe(106);
});

test("database failure is visible without disabling port inspection", async ({
  page,
}) => {
  const { demoPorts } = await import("../src/lib/demo");
  await page.addInitScript(
    ({ ports }) => {
      Object.defineProperty(window, "isTauri", { value: true });
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          transformCallback: () => 1,
          invoke: async (command: string) => {
            if (command === "timeline_query")
              throw new Error("History database is read-only");
            if (command === "scan_ports") return ports;
            if (command === "projects_snapshot")
              return { projects: [], resolving: false, storageError: null };
            if (command === "project_applications")
              return { editors: [], terminals: [] };
            if (command === "autopilot_snapshot")
              return {
                enabled: false,
                supported: true,
                connectionError: null,
                setupCommand: "",
                conflicts: [],
              };
            return null;
          },
        },
      });
      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        value: { unregisterListener: () => {} },
      });
    },
    { ports: demoPorts },
  );
  await openHistory(page);
  await expect(page.getByRole("alert")).toContainText("read-only");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: /Ports/ })
    .click();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(12);
});
