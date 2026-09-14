import chroma from "chroma-js";
import { describe, expect, it } from "vitest";

import {
  getActiveLineIndex,
  getSegmentColor,
  getSegmentColorVars,
} from "./utils";

import type { SegmentKey, SegmentWord } from "~/stt/live-segment";

describe("transcript renderer utils", () => {
  it.each(["light", "dark"] as const)(
    "distinguishes people sharing a channel and speaker index in %s mode",
    (mode) => {
      const key: SegmentKey = {
        channel: "MixedCapture",
        speaker_index: null,
        speaker_human_id: "person-1",
      };
      const first = getSegmentColor(key, mode);
      const second = getSegmentColor(
        { ...key, speaker_human_id: "person-2" },
        mode,
      );

      expect(chroma.deltaE(first, second)).toBeGreaterThan(20);
      expect(
        getSegmentColor(
          { ...key, channel: "RemoteParty", speaker_index: 3 },
          mode,
        ),
      ).toBe(first);
    },
  );

  it("spaces unidentified speakers across distinct hues", () => {
    const colors = Array.from({ length: 6 }, (_, speaker_index) =>
      getSegmentColor({
        channel: "MixedCapture",
        speaker_index,
        speaker_human_id: null,
      }),
    );

    for (let index = 1; index < colors.length; index += 1) {
      expect(chroma.deltaE(colors[index - 1], colors[index])).toBeGreaterThan(
        20,
      );
    }
    expect(new Set(colors).size).toBe(6);
  });

  it("uses a brighter speaker color for dark mode", () => {
    const key: SegmentKey = {
      channel: "RemoteParty",
      speaker_index: 1,
      speaker_human_id: null,
    };

    expect(chroma(getSegmentColor(key, "dark")).luminance()).toBeGreaterThan(
      chroma(getSegmentColor(key)).luminance(),
    );
  });

  it("exposes light and dark speaker color variables", () => {
    const key: SegmentKey = {
      channel: "DirectMic",
      speaker_index: 0,
      speaker_human_id: null,
    };

    expect(getSegmentColorVars(key)).toEqual({
      "--segment-color-light": getSegmentColor(key),
      "--segment-color-dark": getSegmentColor(key, "dark"),
    });
  });

  it("finds the active transcript line without building line groups", () => {
    const words: SegmentWord[] = [
      createWord("word-1", "Hello", 100, 400),
      createWord("word-2", "world.", 400, 900),
      createWord("word-3", "Next", 1400, 1600),
      createWord("word-4", "line!", 1600, 2100),
    ];

    expect(getActiveLineIndex(words, 50, 0)).toBeNull();
    expect(getActiveLineIndex(words, 50, 149)).toBeNull();
    expect(getActiveLineIndex(words, 50, 150)).toBe(0);
    expect(getActiveLineIndex(words, 50, 950)).toBe(0);
    expect(getActiveLineIndex(words, 50, 1200)).toBeNull();
    expect(getActiveLineIndex(words, 50, 1450)).toBe(1);
    expect(getActiveLineIndex(words, 50, 2150)).toBe(1);
    expect(getActiveLineIndex(words, 50, 2200)).toBeNull();
  });
});

function createWord(
  id: string,
  text: string,
  startMs: number,
  endMs: number,
): SegmentWord {
  return {
    id,
    text,
    start_ms: startMs,
    end_ms: endMs,
    channel: "MixedCapture",
    is_final: true,
  };
}
