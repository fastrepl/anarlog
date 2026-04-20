import { describe, expect, it } from "vitest";

import { isAllowedExternalUrl } from "./url-allowlist.js";

describe("isAllowedExternalUrl", () => {
  it.each([
    "https://char.com/docs",
    "http://example.test/",
    "HTTPS://char.com/uppercase-scheme",
    "mailto:team@char.com?subject=hi",
  ])("allows %s", (url) => {
    expect(isAllowedExternalUrl(url)).toBe(true);
  });

  it.each([
    "file:///etc/passwd",
    "javascript:alert(1)",
    "vscode://file/tmp",
    "slack://open",
    "data:text/html,<script>alert(1)</script>",
    "chrome://settings",
    "hyprnote://deep-link",
  ])("rejects %s", (url) => {
    expect(isAllowedExternalUrl(url)).toBe(false);
  });

  it("rejects malformed URLs", () => {
    expect(isAllowedExternalUrl("not a url")).toBe(false);
    expect(isAllowedExternalUrl("")).toBe(false);
    expect(isAllowedExternalUrl("//example.com/no-scheme")).toBe(false);
  });
});
