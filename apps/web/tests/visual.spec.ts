import { expect, test } from "@playwright/test";

test.describe("Page load tests", () => {
  test("homepage", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/Char/);
  });

  test("download page", async ({ page }) => {
    await page.goto("/download");
    await expect(page).toHaveURL(/\/download/);
  });

  test("file transcription page", async ({ page }) => {
    await page.goto("/file-transcription");
    await expect(page).toHaveURL(/\/file-transcription/);
  });
});
