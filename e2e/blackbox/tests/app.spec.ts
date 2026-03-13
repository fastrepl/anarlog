import { $, browser, expect } from "@wdio/globals";

describe("Char Desktop App", () => {
  it("should launch the application", async () => {
    const title = await browser.getTitle();
    expect(title).toBeTruthy();
  });

  it("should have a window", async () => {
    const windowHandles = await browser.getWindowHandles();
    expect(windowHandles.length).toBeGreaterThan(0);
  });

  it("should render the main app shell", async () => {
    // Wait for the main app shell to appear, proving frontend booted and backend IPC works
    const mainShell = await $('[data-testid="main-app-shell"]');
    await mainShell.waitForExist({ timeout: 30000 });
    expect(await mainShell.isDisplayed()).toBe(true);
  });
});
