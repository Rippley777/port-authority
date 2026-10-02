import { expect, test } from "@playwright/test";
test("ports, search, protocol filters, and process details work", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Ports", exact: true }),
  ).toBeVisible();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(12);
  await page.getByRole("button", { name: "UDP 1", exact: true }).click();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(1);
  await page.getByRole("button", { name: "All ports 12" }).click();
  await page.getByRole("searchbox").count();
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill(":5173");
  await expect(page.locator(".port-table tbody tr")).toHaveCount(1);
  await page.getByRole("row", { name: "Inspect vite on port 5173" }).click();
  const drawer = page.getByRole("dialog", { name: "Process details for vite" });
  await expect(drawer).toBeVisible();
  await expect(
    drawer.getByRole("button", { name: "Restart", exact: true }),
  ).toBeDisabled();
  await expect(drawer.getByText("48291", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(drawer).toBeHidden();
  expect(errors).toEqual([]);
});
test("unused port can be watched and persists after reload", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill(":45678");
  await expect(
    page.getByRole("heading", { name: "Port 45678 is available" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Watch port", exact: true }).click();
  await page.reload();
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: /Favorites/ })
    .click();
  await expect(page.locator(".favorite-available")).toContainText("45678");
  await expect(page.locator(".favorite-available")).toContainText("AVAILABLE");
});
test("termination requires confirmation and records history in preview", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill(":5173");
  await page
    .getByRole("button", { name: "Terminate vite", exact: true })
    .click();
  const modal = page.getByRole("dialog", {
    name: "Confirm process termination",
  });
  await expect(
    modal.getByText("Preview only. No real process will be affected."),
  ).toBeVisible();
  await modal.getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(1);
  await page
    .getByRole("button", { name: "Terminate vite", exact: true })
    .click();
  await modal
    .getByRole("button", { name: "Stop process", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Port 5173 is available" }),
  ).toBeVisible();
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "History", exact: true })
    .click();
  await page.getByRole("tab", { name: "Port Timeline", exact: true }).click();
  await expect(page.locator(".timeline-event").first()).toContainText(
    "Port released",
  );
  await expect(page.locator(".timeline-event").first()).toContainText(
    "Shipwreck",
  );
  await page
    .getByRole("button", { name: "Clear History", exact: true })
    .click();
  await page.getByRole("button", { name: "Delete all history" }).click();
  await expect(
    page.getByRole("heading", { name: "No matching activity yet." }),
  ).toBeVisible();
});
test("force kill remains confirmed and system processes are protected", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill("mDNSResponder");
  await expect(
    page.getByRole("button", { name: "Protected system process" }),
  ).toBeDisabled();
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill(":3000");
  await page
    .getByRole("button", { name: "More actions for port 3000" })
    .click();
  await page
    .getByRole("menuitem", { name: "Force kill…", exact: true })
    .click();
  await expect(
    page
      .getByRole("dialog")
      .getByText(
        "This immediately stops the process without cleanup. Unsaved work may be lost.",
      ),
  ).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
});
test("command palette, settings, pause, grouping, and check tool", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+k");
  await page
    .getByRole("textbox", { name: "Search commands and ports" })
    .fill("settings");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Settings", exact: true }),
  ).toBeVisible();
  await page.getByLabel("Refresh interval", { exact: true }).selectOption("5");
  await page.reload();
  await expect(page.getByText("Updates every 5s")).toBeVisible();
  await page.getByRole("button", { name: "Pause live updates" }).click();
  await expect(page.getByText("Auto-refresh paused")).toBeVisible();
  await page.getByRole("button", { name: "Resume live updates" }).click();
  await page.getByRole("tab", { name: "Processes view" }).click();
  await expect(page.locator(".group-header")).toHaveCount(11);
  await page
    .getByRole("navigation", { name: "Tools" })
    .getByRole("button", { name: /Check a port/ })
    .click();
  await page.getByRole("textbox", { name: "Port number" }).fill("70000");
  await expect(
    page.getByText("Enter a valid port number between 1 and 65535."),
  ).toBeVisible();
  await page.getByRole("textbox", { name: "Port number" }).fill("5432");
  await expect(page.locator(".check-result")).toContainText("postgres");
});
test("sort order, system visibility, and narrow layout", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "PORT", exact: true }).click();
  await expect(page.locator(".port-table tbody tr").first()).toContainText(
    "27017",
  );
  await page.getByRole("switch", { name: "Hide system processes" }).click();
  await expect(page.locator(".port-table tbody tr")).toHaveCount(9);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(
    page.getByRole("heading", { name: "Ports", exact: true }),
  ).toBeVisible();
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
  expect(overflow).toBe(false);
});

test("clipboard and browser actions use the selected connection", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await context.route("http://localhost:5173/**", (route) =>
    route.fulfill({ contentType: "text/plain", body: "Test service" }),
  );
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill(":5173");
  await page
    .getByRole("button", { name: "More actions for port 5173" })
    .click();
  await page
    .getByRole("menuitem", { name: "Copy address", exact: true })
    .click();
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe("127.0.0.1:5173");
  const popupPromise = page.waitForEvent("popup");
  await page
    .getByRole("button", { name: "Open localhost:5173", exact: true })
    .click();
  const popup = await popupPromise;
  await expect(popup).toHaveURL("http://localhost:5173/");
  await popup.close();
});

test("failed native scan reports an error without claiming a port is available", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, "isTauri", { value: true });
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke: () =>
          Promise.reject("Permission denied while inspecting local sockets."),
      },
    });
  });
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("Permission denied");
  await page
    .getByRole("textbox", { name: "Search ports", exact: true })
    .fill(":5173");
  await expect(
    page.getByRole("heading", { name: "Port 5173 is available" }),
  ).toHaveCount(0);
  await page
    .getByRole("navigation", { name: "Tools" })
    .getByRole("button", { name: /Check a port/ })
    .click();
  await expect(
    page.getByRole("heading", { name: "Waiting for a successful scan" }),
  ).toBeVisible();
});
