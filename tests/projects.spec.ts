import { test, expect } from "@playwright/test";

test("project identity, repository and tilde-path search share the ports view", async ({
  page,
}) => {
  await page.goto("/");
  for (const query of [
    "shipwreck",
    "Rippley777/shipwreck",
    "~/Code/shipwreck",
  ]) {
    await page
      .getByRole("textbox", { name: "Search ports", exact: true })
      .fill(query);
    await expect(page.locator(".port-table tbody tr")).toHaveCount(2);
  }
  await page
    .getByRole("row", { name: "Inspect vite on port 5173" })
    .getByRole("button", { name: /Shipwreck/ })
    .click();
  const details = page.getByRole("dialog", {
    name: "Project details for Shipwreck",
  });
  await expect(details).toContainText("Vite / React");
  await expect(details).toContainText("Rippley777/shipwreck");
  await expect(details).toContainText(":8080");
  await expect(
    details.getByRole("button", { name: "Open Repository", exact: true }),
  ).toBeVisible();
  await details.getByRole("button", { name: "Show project ports" }).click();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(2);
});
test("project grouping excludes global infrastructure and pinning survives reload", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Projects", exact: true })
    .click();
  await expect(page.locator(".project-card")).toHaveCount(3);
  const ship = page.locator(".project-card").filter({ hasText: "Shipwreck" });
  await expect(ship).toContainText("Vite");
  await expect(ship).toContainText("Rust API");
  await expect(page.locator(".project-list")).not.toContainText("PostgreSQL");
  await ship
    .getByRole("button", { name: "Pin Shipwreck", exact: true })
    .click();
  await page.reload();
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Projects", exact: true })
    .click();
  await page.getByRole("button", { name: "Pinned", exact: true }).click();
  await expect(page.locator(".project-card")).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Unpin Shipwreck" }),
  ).toBeVisible();
});
test("project context actions, clipboard and configurable editor stay consistent", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Tools" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await page
    .getByLabel("Preferred Editor", { exact: true })
    .selectOption("cursor");
  await page
    .getByLabel("Preferred Terminal", { exact: true })
    .selectOption("ghostty");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: /Ports/ })
    .click();
  await page
    .getByRole("button", { name: "More actions for port 5173" })
    .click();
  await expect(
    page.getByRole("menuitem", { name: "Open in Cursor" }),
  ).toBeVisible();
  await page.getByRole("menuitem", { name: "Copy Project Path" }).click();
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe("/Users/developer/Code/shipwreck");
  await page.keyboard.press("Control+k");
  await page
    .getByRole("textbox", { name: "Search commands and ports" })
    .fill("ship");
  await expect(
    page.getByRole("button", { name: "Open Shipwreck in Cursor" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Show Shipwreck ports" }).click();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(2);
});
test("previously observed projects remain visible after listeners stop", async ({
  page,
}) => {
  await page.goto("/");
  for (const [port, process] of [
    [5173, "vite"],
    [8080, "rust-api"],
  ] as const) {
    await page
      .getByRole("button", { name: `Terminate ${process}`, exact: true })
      .click();
    await page
      .getByRole("button", { name: "Stop process", exact: true })
      .click();
    await expect(
      page.getByRole("row", { name: `Inspect ${process} on port ${port}` }),
    ).toHaveCount(0);
  }
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Projects", exact: true })
    .click();
  const ship = page.locator(".project-card").filter({ hasText: "Shipwreck" });
  await expect(ship).toContainText("Not running");
  await expect(ship).toContainText("Known ports :5173 :8080");
  await ship.getByRole("button", { name: /Shipwreck.*Code/ }).click();
  const details = page.getByRole("dialog", {
    name: "Project details for Shipwreck",
  });
  await details.getByRole("button", { name: "Forget recent project" }).click();
  await expect(page.locator(".project-card")).toHaveCount(2);
});
test("project screen fits a narrow viewport and has no browser errors", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Projects", exact: true })
    .click();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    ),
  ).toBe(false);
  expect(errors).toEqual([]);
});

test("late native enrichment works while paused and rejects reused PID metadata", async ({
  page,
}) => {
  const { demoPorts } = await import("../src/lib/demo");
  const resolved = demoPorts.find((p) => p.port === 5173)!;
  const raw = { ...resolved, project: null, serviceName: null };
  await page.addInitScript(
    ({ raw }) => {
      const callbacks = new Map<number, (event: unknown) => void>();
      let next = 0;
      let handler = 0;
      Object.defineProperty(window, "isTauri", { value: true });
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          transformCallback: (callback: (event: unknown) => void) => {
            callbacks.set(++next, callback);
            return next;
          },
          invoke: async (command: string, args?: { handler: number }) => {
            if (command === "scan_ports") return [raw];
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
            if (command === "plugin:event|listen") {
              handler = args!.handler;
              return handler;
            }
            return null;
          },
        },
      });
      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        value: { unregisterListener: () => {} },
      });
      Object.defineProperty(window, "emitProjectTest", {
        value: (payload: unknown) =>
          callbacks.get(handler)?.({
            event: "projects-resolved",
            id: handler,
            payload,
          }),
      });
      localStorage.setItem(
        "pa-settings",
        JSON.stringify({ refreshInterval: 10 }),
      );
    },
    { raw },
  );
  await page.goto("/");
  await expect(page.locator(".port-table tbody tr")).toHaveCount(1);
  await page.getByRole("button", { name: "Pause live updates" }).click();
  await page.evaluate(
    (payload) =>
      (
        window as unknown as { emitProjectTest: (p: unknown) => void }
      ).emitProjectTest(payload),
    [
      {
        ...resolved,
        startedAt: resolved.startedAt! - 100,
        project: { ...resolved.project!, name: "Wrong project" },
      },
    ],
  );
  await expect(page.locator(".port-table")).not.toContainText("Wrong project");
  await page.evaluate(
    (payload) =>
      (
        window as unknown as { emitProjectTest: (p: unknown) => void }
      ).emitProjectTest(payload),
    [resolved],
  );
  await expect(page.locator(".port-table")).toContainText("Shipwreck");
  await expect(page.locator(".port-table")).not.toContainText("Wrong project");
});
