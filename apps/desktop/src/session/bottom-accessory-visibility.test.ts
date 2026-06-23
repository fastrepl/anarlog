import { describe, expect, it } from "vitest";

import { shouldShowSessionBottomAccessory } from "./bottom-accessory-visibility";

describe("shouldShowSessionBottomAccessory", () => {
  it("keeps transcript-only insights reachable on the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        bottomAccessoryState: {
          mode: "transcript_only",
          expanded: false,
        },
      }),
    ).toBe(true);
  });

  it("preserves playback bottom controls on the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(true);
  });

  it("keeps live bottom controls off the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        bottomAccessoryState: {
          mode: "live",
          expanded: false,
        },
      }),
    ).toBe(false);
  });
});
