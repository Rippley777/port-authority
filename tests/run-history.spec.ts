import { expect, test } from "@playwright/test";

test("run history groups equivalent commands and searches across project and command", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "History", exact: true })
    .click();
  await expect(page.getByRole("tab", { name: "Run History" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const cards = page.locator(".command-history-list .command-history-card");
  await expect(cards).toHaveCount(2);
  await expect(cards.filter({ hasText: "Shipwreck" })).toContainText("2 runs");
  await expect(cards.filter({ hasText: "Shipwreck" })).toContainText(
    "npm run dev",
  );
  await page.getByLabel("Search run history").fill("plant pnpm");
  await expect(cards).toHaveCount(1);
  await expect(cards.first()).toContainText("Plant Journal");
  await expect(cards.first()).toContainText("pnpm dev");
  await cards.first().getByRole("button", { name: "Details" }).click();
  await expect(
    page.getByRole("dialog", { name: "Run details for pnpm dev" }),
  ).toContainText("Process exited");
  await page.keyboard.press("Escape");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    ),
  ).toBe(false);
});

test("an available favorite relaunches its remembered command and records a new run", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Search ports").fill(":5173");
  await page
    .getByRole("button", { name: "Terminate vite", exact: true })
    .click();
  await page
    .getByRole("dialog", { name: "Confirm process termination" })
    .getByRole("button", { name: "Stop process", exact: true })
    .click();
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: /Favorites/ })
    .click();
  const favorite = page
    .locator(".favorite-memory-card")
    .filter({ hasText: ":5173" });
  await expect(favorite).toContainText("AVAILABLE");
  await expect(favorite).toContainText("npm run dev");
  await favorite.getByRole("button", { name: "Run Again" }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Verified ports :5173" }),
  ).toBeVisible();
  await favorite.getByRole("button", { name: "History" }).click();
  await expect(favorite).toContainText("RECENT COMMANDS");
  await expect(favorite.locator(".favorite-command-row")).toHaveCount(2);
});

test("command palette fuzzy-searches historical launch points", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+k");
  await page.getByLabel("Search commands and ports").fill("plant dev");
  await expect(
    page.getByRole("button", { name: /Run Plant Journal — pnpm dev/ }),
  ).toBeVisible();
});
