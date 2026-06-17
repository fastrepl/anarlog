import { describe, expect, it } from "vitest";

import {
  getMaxSpeakerNumberForParticipants,
  SegmentKeyUtils,
  SpeakerLabelManager,
  type RenderLabelContext,
  type Segment,
} from "./live-segment";

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

  it("caps unknown speaker labels when a participant max is provided", () => {
    const segments: Segment[] = [0, 1, 2].map(
      (speakerIndex) =>
        ({
          id: `segment-${speakerIndex}`,
          key: {
            channel: "RemoteParty",
            speaker_index: speakerIndex,
            speaker_human_id: null,
          },
          words: [],
          start_ms: 0,
          end_ms: 0,
          text: "",
        }) as Segment,
    );
    const manager = SpeakerLabelManager.fromSegments(segments, undefined, 2);

    expect(
      SegmentKeyUtils.renderLabel(segments[0]!.key, undefined, manager),
    ).toBe("Speaker 1");
    expect(
      SegmentKeyUtils.renderLabel(segments[1]!.key, undefined, manager),
    ).toBe("Speaker 2");
    expect(
      SegmentKeyUtils.renderLabel(segments[2]!.key, undefined, manager),
    ).toBe("Speaker 2");
  });

  it("derives max speaker number from distinct participants plus self", () => {
    expect(getMaxSpeakerNumberForParticipants(["remote"], "self")).toBe(2);
    expect(getMaxSpeakerNumberForParticipants(["self", "remote"], "self")).toBe(
      2,
    );
    expect(getMaxSpeakerNumberForParticipants([], "self")).toBeUndefined();
  });
});
