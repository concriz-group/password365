import { expect, type Page } from "@playwright/test";

/** The demo backend's master password (see src/dev/mockBackend.ts). */
export const DEMO_PASSWORD = "demo";

/** Opens the app on the unlock screen and unlocks it with the demo password. */
export async function unlock(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Your vault is locked" })).toBeVisible();
  await page.getByLabel("Master password", { exact: true }).fill(DEMO_PASSWORD);
  await page.getByRole("button", { name: "Unlock" }).click();
  await expect(page.getByRole("button", { name: "+ New item" })).toBeVisible();
}
