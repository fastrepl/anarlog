import { describe, expect, it } from "vitest";

import { SegmentKeyUtils, type RenderLabelContext } from "./live-segment";

const ctx: RenderLabelContext = {
  getSelfHumanId: () => "self",
  getHumanName: (id) => (id === "self" ? "Me" : undefined),
};

describe("SegmentKeyUtils", () => {
  it("does not treat diarized direct-mic segments as self", () => {
    const key: Parameters<typeof SegmentKeyUtils.isKnownSpeaker>[0] = {
      channel: "DirectMic",
      speaker_index: 2,
      speaker_human_id: null,
    };

    expect(SegmentKeyUtils.isKnownSpeaker(key, ctx)).toBe(false);
    expect(SegmentKeyUtils.renderLabel(key, ctx)).toBe("Speaker 3");
  });
});
