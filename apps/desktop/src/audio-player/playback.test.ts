import { describe, expect, it, vi } from "vitest";

import { configureCenteredPlayback } from "./playback";

describe("configureCenteredPlayback", () => {
  it("folds stereo playback into a centered mono output", () => {
    const context = {
      close: vi.fn(),
      resume: vi.fn(),
    };
    const gainNode = {
      channelCount: 2,
      channelCountMode: "max",
      channelInterpretation: "discrete",
      context,
    };
    const media = {
      getGainNode: () => gainNode,
    } as unknown as HTMLMediaElement;

    expect(configureCenteredPlayback(media)).toBe(context);
    expect(gainNode).toMatchObject({
      channelCount: 1,
      channelCountMode: "explicit",
      channelInterpretation: "speakers",
    });
  });

  it("keeps media-element playback as a fallback", () => {
    expect(configureCenteredPlayback({} as HTMLMediaElement)).toBeNull();
  });
});
