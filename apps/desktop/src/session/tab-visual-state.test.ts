import { describe, expect, it } from "vitest";

import { getSessionTabVisualState } from "./tab-visual-state";

describe("getSessionTabVisualState", () => {
  it("keeps active sessions red without a spinner", () => {
    expect(getSessionTabVisualState("active", false, true)).toEqual({
      isActive: true,
      accent: "red",
      showSpinner: false,
    });
  });

  it("shows a spinner for finalizing selected tabs and removes the red accent", () => {
    expect(getSessionTabVisualState("finalizing", false, true)).toEqual({
      isActive: true,
      accent: "neutral",
      showSpinner: true,
    });
  });

  it("shows a spinner for enhancing or batching only when the tab is not selected", () => {
    expect(getSessionTabVisualState("running_batch", false, false)).toEqual({
      isActive: false,
      accent: "neutral",
      showSpinner: true,
    });

    expect(getSessionTabVisualState("inactive", true, false)).toEqual({
      isActive: false,
      accent: "neutral",
      showSpinner: true,
    });

    expect(getSessionTabVisualState("inactive", true, true)).toEqual({
      isActive: false,
      accent: "neutral",
      showSpinner: false,
    });
  });
});
