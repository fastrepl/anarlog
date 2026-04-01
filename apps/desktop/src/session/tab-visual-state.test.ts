import { describe, expect, it } from "vitest";

import { getSessionTabStatus } from "./tab-visual-state";

describe("getSessionTabStatus", () => {
  it("returns listening for active sessions", () => {
    expect(getSessionTabStatus("active", false, false, true)).toBe("listening");
  });

  it("returns listening-degraded when active and degraded", () => {
    expect(getSessionTabStatus("active", false, true, true)).toBe(
      "listening-degraded",
    );
  });

  it("returns finalizing for finalizing sessions", () => {
    expect(getSessionTabStatus("finalizing", false, false, true)).toBe(
      "finalizing",
    );
  });

  it("returns processing for enhancing or batching only when the tab is not selected", () => {
    expect(getSessionTabStatus("running_batch", false, false, false)).toBe(
      "processing",
    );
    expect(getSessionTabStatus("inactive", true, false, false)).toBe(
      "processing",
    );
    expect(getSessionTabStatus("inactive", true, false, true)).toBeUndefined();
  });

  it("returns undefined for inactive sessions", () => {
    expect(getSessionTabStatus("inactive", false, false, true)).toBeUndefined();
  });
});
