import { describe, expect, it } from "vitest";

import { shouldShowSessionBottomAccessory } from "./bottom-accessory-visibility";

describe("shouldShowSessionBottomAccessory", () => {
  it("keeps the bottom area empty on inactive transcript views without accessory state", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        sessionMode: "inactive",
        bottomAccessoryState: null,
      }),
    ).toBe(false);
  });

  it("preserves playback bottom controls on the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        sessionMode: "inactive",
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(true);
  });

  it("hides batch transcription bottom chrome on the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        sessionMode: "running_batch",
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(false);
  });

  it("hides batch transcription bottom chrome outside the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "raw" },
        sessionMode: "running_batch",
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(false);
  });

  it("hides batch transcription status without accessory state", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "raw" },
        sessionMode: "running_batch",
        bottomAccessoryState: null,
      }),
    ).toBe(false);
  });
});
