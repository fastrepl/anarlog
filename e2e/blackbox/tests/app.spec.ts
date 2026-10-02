import { $, expect } from "@wdio/globals";

describe("Char Desktop App", () => {
  it("launches and renders the main app shell", async () => {
    const mainShell = await $('[data-testid="main-app-shell"]');
    await mainShell.waitForExist({ timeout: 30000 });
    expect(await mainShell.isDisplayed()).toBe(true);
  });
});
