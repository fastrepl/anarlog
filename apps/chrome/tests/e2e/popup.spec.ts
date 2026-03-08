import { chromium, expect, test } from "@playwright/test";
import os from "node:os";
import path from "node:path";

test("popup renders basic React UI", async () => {
  const extensionPath = path.resolve(
    import.meta.dirname,
    "../../.output/chrome-mv3",
  );

  const context = await chromium.launchPersistentContext(
    path.join(os.tmpdir(), "char-wxt-e2e"),
    {
      channel: "chromium",
      headless: false,
      args: [
        `--disable-extensions-except=${extensionPath}`,
        `--load-extension=${extensionPath}`,
      ],
    },
  );

  try {
    let [background] = context.serviceWorkers();
    if (!background) {
      background = await context.waitForEvent("serviceworker");
    }

    const extensionId = background.url().split("/")[2];
    const page = await context.newPage();

    await page.goto(`chrome-extension://${extensionId}/popup.html`);

    await expect(
      page.getByRole("heading", { name: "Char for Google Meet" }),
    ).toBeVisible();
    await expect(
      page.getByText("Native host bridge is enabled for meeting state sync."),
    ).toBeVisible();
  } finally {
    await context.close();
  }
});
