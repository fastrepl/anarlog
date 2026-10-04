import { describe, expect, it } from "vitest";

import { changelogUrl } from "./source";

describe("changelog sources", () => {
  it("loads Nightly's immutable release notes independently of stable", () => {
    expect(changelogUrl("1.4.24-nightly.123")).toBe(
      "https://api.github.com/repos/fastrepl/anarlog/releases/tags/desktop_nightly_v1.4.24-nightly.123",
    );
    expect(changelogUrl("1.4.24")).toContain("/content/desktop/1.4.24.md");
  });

  it("rejects unsupported versions and path injection", () => {
    for (const version of [
      "../../main",
      "1.4.24-dev.1",
      "1.4.24?query=1",
      "",
    ]) {
      expect(changelogUrl(version)).toBeNull();
    }
  });
});
