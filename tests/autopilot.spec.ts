import { test, expect } from "@playwright/test";
async function openAutopilot(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Autopilot", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Conflict Autopilot", exact: true }),
  ).toBeVisible();
}
test("another project requires confirmation and inspection uses the existing drawer", async ({
  page,
}) => {
  await openAutopilot(page);
  await page.getByRole("button", { name: "Simulate a conflict" }).click();
  await expect(
    page.getByRole("heading", { name: "Port 5173 is already occupied" }),
  ).toBeVisible();
  await expect(page.locator(".conflict-owner")).toContainText("Shipwreck");
  await page
    .getByRole("button", { name: "Open Shipwreck", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Project details for Shipwreck" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close project details" }).click();
  await page.getByRole("button", { name: "Inspect", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Process details for vite" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Kill & Retry", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Confirm conflict recovery" }),
  ).toContainText("/Users/developer/Code/shipwreck");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator(".recovery-steps")).toHaveCount(0);
  await page.getByRole("button", { name: "Kill & Retry", exact: true }).click();
  await page
    .getByRole("button", { name: "Confirm Kill & Retry", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Back to building on port 5173" }),
  ).toBeVisible();
  await expect(page.locator(".recovery-steps")).toContainText(
    "Verified the new process owns port 5173",
  );
  await expect(page.locator(".conflict-message")).toContainText(
    "No real process was affected",
  );
});
test("alternate port does not request termination and restart is honestly unavailable", async ({
  page,
}) => {
  await openAutopilot(page);
  await page.getByRole("button", { name: "Simulate a conflict" }).click();
  await expect(
    page.getByRole("button", { name: "Restart Owner", exact: true }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Use 5174 Instead", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Back to building on port 5174" }),
  ).toBeVisible();
  await expect(page.locator(".recovery-steps")).toContainText(
    "existing owner left running",
  );
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
test("infrastructure confirmation requires explicit acknowledgement", async ({
  page,
}) => {
  await openAutopilot(page);
  await page.getByLabel("Conflict preview scenario").selectOption("database");
  await page.getByRole("button", { name: "Simulate a conflict" }).click();
  await expect(
    page.getByText("Caution: infrastructure service", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Kill & Retry", exact: true }).click();
  const confirm = page.getByRole("button", {
    name: "Confirm Kill & Retry",
    exact: true,
  });
  await expect(confirm).toBeDisabled();
  await page.getByRole("checkbox").check();
  await expect(confirm).toBeEnabled();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page
    .getByRole("button", { name: "Ignore conflict on port 5432" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Nothing in your way." }),
  ).toBeVisible();
});
test("force escalation is a separate, confirmed action", async ({ page }) => {
  await openAutopilot(page);
  await page
    .getByLabel("Conflict preview scenario")
    .selectOption("unresponsive");
  await page.getByRole("button", { name: "Simulate a conflict" }).click();
  await expect(
    page.getByRole("button", { name: "Force Kill & Retry…", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Kill & Retry", exact: true }).click();
  await page
    .getByRole("button", { name: "Confirm Kill & Retry", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Force Kill & Retry…", exact: true })
    .click();
  const confirm = page.getByRole("button", {
    name: "Confirm Force Kill & Retry",
    exact: true,
  });
  await expect(confirm).toBeDisabled();
  await page.getByRole("checkbox").check();
  await confirm.click();
  await expect(
    page.getByRole("heading", { name: "Back to building on port 5173" }),
  ).toBeVisible();
});
test("a pending conflict is visible across the workspace and Ignore is nondestructive", async ({
  page,
}) => {
  await openAutopilot(page);
  await page.getByRole("button", { name: "Simulate a conflict" }).click();
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Ports 12", exact: true })
    .click();
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: "Port 5173 is blocking new-dashboard" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Review conflict", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Ignore conflict on port 5173" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Nothing in your way." }),
  ).toBeVisible();
});
