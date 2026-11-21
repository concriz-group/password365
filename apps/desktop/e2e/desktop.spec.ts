import { expect, test } from "@playwright/test";

import { DEMO_PASSWORD, unlock } from "./helpers";

test.describe("first run", () => {
  test("creates the account, shows the Secret Key and opens the vault", async ({ page }) => {
    await page.goto("/?fresh=1");
    await page.getByLabel("Master password", { exact: true }).fill("a long master password");
    await page.getByLabel("Repeat master password", { exact: true }).fill("a long master password");
    await page.getByRole("button", { name: "Create account" }).click();

    await expect(page.getByRole("heading", { name: "Save your Secret Key" })).toBeVisible();
    await expect(page.getByLabel("Secret Key", { exact: true })).toContainText("P365-");
    const open = page.getByRole("button", { name: "Open my vault" });
    await expect(open).toBeDisabled();
    await page.getByLabel("I have saved my Secret Key").check();
    await open.click();
    await expect(page.getByRole("button", { name: "+ New item" })).toBeVisible();
  });

  test("does not create the account when the passwords differ", async ({ page }) => {
    await page.goto("/?fresh=1");
    await page.getByLabel("Master password", { exact: true }).fill("one password here");
    await page.getByLabel("Repeat master password", { exact: true }).fill("another one");
    await expect(page.getByText("Passwords do not match.")).toBeVisible();
    await expect(page.getByRole("button", { name: "Create account" })).toBeDisabled();
  });
});

test.describe("unlock and lock", () => {
  test("a wrong password is refused, the right one opens the vault, Lock closes it again", async ({ page }) => {
    await page.goto("/");
    await page.getByLabel("Master password", { exact: true }).fill("wrong");
    await page.getByRole("button", { name: "Unlock" }).click();
    await expect(page.getByRole("alert")).toContainText("wrong master password");

    await page.getByLabel("Master password", { exact: true }).fill(DEMO_PASSWORD);
    await page.getByRole("button", { name: "Unlock" }).click();
    await expect(page.getByText("GitHub").first()).toBeVisible();

    await page.getByRole("button", { name: "Lock", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Your vault is locked" })).toBeVisible();
  });
});

test.describe("items", () => {
  test("search narrows the list", async ({ page }) => {
    await unlock(page);
    await expect(page.getByText("Bank").first()).toBeVisible();
    await page.getByRole("searchbox").or(page.getByPlaceholder("Search")).first().fill("jira");
    await expect(page.getByText("Jira").first()).toBeVisible();
    await expect(page.getByText("Bank")).toHaveCount(0);
  });

  test("creates a login, reveals its password, deletes it and restores it", async ({ page }) => {
    await unlock(page);
    await page.getByRole("button", { name: "+ New item" }).click();
    await page.getByRole("button", { name: "Login", exact: true }).click();
    await page.getByLabel("Title", { exact: true }).fill("Example Shop");
    await page.getByLabel("Username", { exact: true }).fill("me@example.com");
    await page.getByLabel("Password", { exact: true }).fill("S3cret-e2e-value");
    await page.getByRole("button", { name: "Save" }).click();

    await expect(page.getByRole("heading", { name: "Example Shop" })).toBeVisible();
    await expect(page.getByText("S3cret-e2e-value")).toHaveCount(0);
    await page.getByRole("button", { name: "Reveal" }).first().click();
    await expect(page.getByText("S3cret-e2e-value")).toBeVisible();

    await page.getByRole("button", { name: "Delete", exact: true }).click();
    await page.getByRole("button", { name: "Move to Recently Deleted" }).click();
    await expect(page.getByRole("button", { name: /Example Shop/ })).toHaveCount(0);

    await page.getByRole("button", { name: "Recently Deleted" }).click();
    await expect(page.getByText("Example Shop")).toBeVisible();
    await page.getByRole("button", { name: "Restore" }).first().click();
    await expect(page.getByText("No recently deleted items.")).toBeVisible();
  });
});

test.describe("security and settings", () => {
  test("Watchtower lists the demo vault's findings", async ({ page }) => {
    await unlock(page);
    await page.getByRole("button", { name: "Watchtower" }).click();
    await expect(page.getByRole("heading", { name: "Watchtower" })).toBeVisible();
    await expect(page.getByText("Overall Password Strength")).toBeVisible();
  });

  test("the Emergency Kit can be kept in the vault from Settings", async ({ page }) => {
    await unlock(page);
    await page.getByRole("button", { name: "Settings" }).click();
    await page.getByRole("button", { name: "Security" }).click();
    await page.getByLabel("Master password", { exact: true }).fill(DEMO_PASSWORD);
    await page.getByRole("button", { name: "Save in the vault" }).click();
    await expect(page.getByText("Saved in your vault as a document.")).toBeVisible();
  });
});

test.describe("erase the vault", () => {
  test("a locked vault with a forgotten password can be erased and started again", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: /Forgot your master password/ }).click();
    const confirm = page.getByRole("button", { name: "Erase this vault" });
    await expect(confirm).toBeDisabled();
    await page.getByLabel("To go on, type ERASE:").fill("ERASE");
    await confirm.click();
    await expect(page.getByRole("heading", { name: "Create your vault" })).toBeVisible();
  });
});
