import { describe, expect, it } from "vitest";

import { shouldShowSessionBottomAccessory } from "./bottom-accessory-visibility";

describe("shouldShowSessionBottomAccessory", () => {
  it("keeps transcript-only insights reachable on the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        sessionMode: "inactive",
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
        sessionMode: "inactive",
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(true);
  });

  it("keeps the bottom area empty on the transcript tab without accessory state", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        sessionMode: "inactive",
        bottomAccessoryState: null,
      }),
    ).toBe(false);
  });

  it("keeps batch transcription stop controls on the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "transcript" },
        sessionMode: "running_batch",
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(true);
  });

  it("keeps batch transcription stop controls outside the transcript tab", () => {
    expect(
      shouldShowSessionBottomAccessory({
        currentView: { type: "raw" },
        sessionMode: "running_batch",
        bottomAccessoryState: {
          mode: "playback",
          expanded: false,
        },
      }),
    ).toBe(true);
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
