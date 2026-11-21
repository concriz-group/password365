import { test } from "@playwright/test";

import { DEMO_PASSWORD } from "./helpers";

// Makes the website's desktop screenshots (made-up data: see the `?shots` mode of the demo backend).
//   SITE_SHOTS=1 npx playwright test site-screenshots
// Not part of the normal run.
const LANGUAGES = { en: "en-US", pt: "pt-BR", es: "es-ES" } as const;

for (const [tag, locale] of Object.entries(LANGUAGES)) {
  test(`desktop screenshot (${tag})`, async ({ browser }) => {
    test.skip(!process.env.SITE_SHOTS, "only when SITE_SHOTS=1");
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, locale });
    const page = await context.newPage();
    await page.goto("/?shots");
    await page.locator('input[type="password"]').first().fill(DEMO_PASSWORD);
    await page.locator('button[type="submit"]').click();
    await page.getByText("Lumen Cloud").first().waitFor();
    // The notices a first run shows (updates, backup) are not what the page is about.
    for (const name of [/^(Not now|Agora não|Ahora no)$/]) {
      const dismiss = page.getByRole("button", { name });
      if (await dismiss.count()) await dismiss.first().click();
    }
    await page.getByText("Lumen Cloud").first().click();
    await page.waitForTimeout(600);
    await page.screenshot({ path: `../../site/public/screens/${tag}/desktop-01-vault.jpg`, type: "jpeg", quality: 88 });
    await context.close();
  });
}
