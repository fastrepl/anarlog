import { describe, expect, it, vi } from "vitest";

import type {
  IdentityAssignment,
  RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import {
  applyRenderRequestIdentitiesToSegments,
  getMaxSpeakerNumberForParticipants,
  mergeRenderedAndLiveSegments,
  SegmentKeyUtils,
  SpeakerLabelManager,
  type RenderLabelContext,
  type Segment,
} from "./live-segment";

const ctx: RenderLabelContext = {
  getSelfHumanId: () => "self",
  getHumanName: (id) => (id === "self" ? "Me" : undefined),
};
const twoPersonCtx: RenderLabelContext = {
  getSelfHumanId: () => "self",
  getHumanName: (id) =>
    id === "self" ? "Me" : id === "remote" ? "Artem" : undefined,
  getParticipantHumanIds: () => ["self", "remote"],
};

describe("SegmentKeyUtils", () => {
  it("treats diarized direct-mic segments as self", () => {
    const key: Parameters<typeof SegmentKeyUtils.isKnownSpeaker>[0] = {
      channel: "DirectMic",
      speaker_index: 2,
      speaker_human_id: null,
    };

    expect(SegmentKeyUtils.isKnownSpeaker(key, ctx)).toBe(true);
    expect(SegmentKeyUtils.renderLabel(key, ctx)).toBe("Me");
  });

  it("does not label assigned direct-mic segments as self when the name is unavailable", () => {
    const key: Parameters<typeof SegmentKeyUtils.renderLabel>[0] = {
      channel: "DirectMic",
      speaker_index: 1,
      speaker_human_id: "remote",
    };

    expect(SegmentKeyUtils.renderLabel(key, ctx)).toBe("Speaker 2");
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

  it("labels remote-party segments as the unique other participant", () => {
    const key: Parameters<typeof SegmentKeyUtils.renderLabel>[0] = {
      channel: "RemoteParty",
      speaker_index: 0,
      speaker_human_id: null,
    };

    expect(SegmentKeyUtils.isKnownSpeaker(key, twoPersonCtx)).toBe(true);
    expect(SegmentKeyUtils.renderLabel(key, twoPersonCtx)).toBe("Artem");
  });

  it("derives max speaker number from distinct participants plus self", () => {
    expect(getMaxSpeakerNumberForParticipants(["remote"], "self")).toBe(2);
    expect(getMaxSpeakerNumberForParticipants(["self", "remote"], "self")).toBe(
      2,
    );
    expect(getMaxSpeakerNumberForParticipants([], "self")).toBeUndefined();
  });
});

describe("mergeRenderedAndLiveSegments", () => {
  it("preserves persisted words that share a segment with the live tail", () => {
    const persisted = createSegment("persisted", [
      { id: "word-prefix", startMs: 0 },
      { id: "word-tail", startMs: 100 },
    ]);
    const live = createSegment("live", [{ id: "word-tail", startMs: 100 }]);

    const merged = mergeRenderedAndLiveSegments([persisted], [live]);

    expect(
      merged.flatMap((segment) => segment.words.map((word) => word.id)),
    ).toEqual(["word-prefix", "word-tail"]);
  });

  it("keeps split persisted fragments in timestamp order", () => {
    const persisted = createSegment("persisted", [
      { id: "word-before", startMs: 0 },
      { id: "word-live", startMs: 100 },
      { id: "word-after", startMs: 200 },
    ]);
    const live = createSegment("live", [{ id: "word-live", startMs: 100 }]);

    const merged = mergeRenderedAndLiveSegments([persisted], [live]);

    expect(merged.map((segment) => segment.words[0]?.id)).toEqual([
      "word-before",
      "word-live",
      "word-after",
    ]);
  });

  it("preserves the persisted prefix beside an id-less live partial", () => {
    const persisted = createSegment("persisted", [
      { id: "word-prefix", startMs: 0 },
    ]);
    const live = createSegment("live", [{ startMs: 100 }]);

    const merged = mergeRenderedAndLiveSegments([persisted], [live]);

    expect(merged).toEqual([persisted, live]);
  });

  it("replaces a frozen word before its new id reaches SQLite", () => {
    const persisted = createSegment("persisted", [
      { id: "word-old", startMs: 100 },
    ]);
    const live = createSegment("live", [
      { id: "word-replacement", startMs: 100 },
    ]);
    const request = createRequest(["word-old"]);

    const merged = mergeRenderedAndLiveSegments([persisted], [live], request);

    expect(
      merged.flatMap((segment) => segment.words.map((word) => word.id)),
    ).toEqual(["word-replacement"]);
  });

  it("removes frozen words that SQLite has already replaced", () => {
    const persisted = createSegment("persisted", [
      { id: "word-old", startMs: 100 },
    ]);
    const live = createSegment("live", [
      { id: "word-replacement", startMs: 100 },
    ]);
    const request = createRequest(["word-replacement"]);

    const merged = mergeRenderedAndLiveSegments([persisted], [live], request);

    expect(
      merged.flatMap((segment) => segment.words.map((word) => word.id)),
    ).toEqual(["word-replacement"]);
  });

  it("indexes pending replacements instead of scanning the full live tail", () => {
    const persisted = createSegment(
      "persisted",
      Array.from({ length: 1_000 }, (_, index) => ({
        id: `persisted-${index}`,
        startMs: index * 100,
      })),
    );
    const live = createSegment(
      "live",
      Array.from({ length: 1_000 }, (_, index) => ({
        id: `live-${index}`,
        startMs: 100_000 + index * 100,
      })),
    );
    const request = createRequest(
      [...persisted.words, ...live.words].flatMap((word) =>
        word.id ? [word.id] : [],
      ),
    );
    const hasSpy = vi.spyOn(Set.prototype, "has");

    try {
      mergeRenderedAndLiveSegments([persisted], [live], request);
      expect(hasSpy.mock.calls.length).toBeLessThan(10_000);
    } finally {
      hasSpy.mockRestore();
    }
  });
});

describe("applyRenderRequestIdentitiesToSegments", () => {
  it("applies a speaker assignment to current and future matching segments", () => {
    const current = createSegment("current", [
      { id: "word-current", startMs: 0 },
    ]);
    const future = createSegment("future", [
      { id: "word-future", startMs: 100 },
    ]);
    const other = createSegment("other", [{ id: "word-other", startMs: 200 }]);
    current.key.speaker_index = 1;
    future.key.speaker_index = 1;
    other.key.speaker_index = 2;
    const request = createRequest(
      ["word-current", "word-future", "word-other"],
      [
        {
          human_id: "human-1",
          scope: {
            kind: "channel_speaker",
            channel: "MixedCapture",
            speaker_index: 1,
          },
        },
      ],
    );

    const result = applyRenderRequestIdentitiesToSegments(
      [current, future, other],
      request,
    );

    expect(result.map((segment) => segment.key.speaker_human_id)).toEqual([
      "human-1",
      "human-1",
      null,
    ]);
  });

  it("splits word-scoped assignments and carries them into a trailing partial", () => {
    const segment = createSegment("live", [
      { id: "word-before", startMs: 0 },
      { id: "word-assigned", startMs: 100 },
      { startMs: 200 },
    ]);
    segment.key.speaker_index = 1;
    const request = createRequest(
      ["word-before", "word-assigned"],
      [
        {
          human_id: "speaker-human",
          scope: {
            kind: "channel_speaker",
            channel: "MixedCapture",
            speaker_index: 1,
          },
        },
        {
          human_id: "word-human",
          scope: { kind: "words", word_ids: ["word-assigned"] },
        },
      ],
    );

    const result = applyRenderRequestIdentitiesToSegments([segment], request);

    expect(result).toHaveLength(2);
    expect(
      result.map((current) => ({
        humanId: current.key.speaker_human_id,
        wordIds: current.words.map((word) => word.id),
      })),
    ).toEqual([
      { humanId: "speaker-human", wordIds: ["word-before"] },
      {
        humanId: "word-human",
        wordIds: ["word-assigned", undefined],
      },
    ]);
  });

  it("gives word assignments precedence over channel speaker assignments", () => {
    const segment = createSegment("live", [
      { id: "word-a", startMs: 0 },
      { id: "word-b", startMs: 100 },
    ]);
    segment.key.speaker_index = 1;
    const request = createRequest(
      ["word-a", "word-b"],
      [
        {
          human_id: "speaker-human",
          scope: {
            kind: "channel_speaker",
            channel: "MixedCapture",
            speaker_index: 1,
          },
        },
        {
          human_id: "word-human",
          scope: { kind: "words", word_ids: ["word-b"] },
        },
      ],
    );

    const result = applyRenderRequestIdentitiesToSegments([segment], request);

    expect(result.map((current) => current.key.speaker_human_id)).toEqual([
      "speaker-human",
      "word-human",
    ]);
  });

  it("keeps participant identity ahead of complete-channel assignments", () => {
    const segment = createSegment("live", [{ id: "word-a", startMs: 0 }]);
    segment.key = {
      channel: "DirectMic",
      speaker_index: null,
      speaker_human_id: "self",
    };
    const request = createRequest(
      ["word-a"],
      [
        {
          human_id: "other-human",
          scope: { kind: "channel", channel: "DirectMic" },
        },
      ],
    );
    request.self_human_id = "self";

    const result = applyRenderRequestIdentitiesToSegments([segment], request);

    expect(result[0]?.key.speaker_human_id).toBe("self");
  });
});

function createSegment(
  id: string,
  words: Array<{ id?: string; startMs: number }>,
): Segment {
  return {
    id,
    key: {
      channel: "MixedCapture",
      speaker_index: null,
      speaker_human_id: null,
    },
    start_ms: words[0]?.startMs ?? 0,
    end_ms: (words[words.length - 1]?.startMs ?? 0) + 100,
    text: words.map((word) => ` word-${word.id ?? "partial"}`).join(""),
    words: words.map((word) => ({
      id: word.id,
      text: ` word-${word.id ?? "partial"}`,
      start_ms: word.startMs,
      end_ms: word.startMs + 100,
      channel: "MixedCapture",
      is_final: Boolean(word.id),
    })),
  };
}

function createRequest(
  wordIds: string[],
  assignments: IdentityAssignment[] = [],
): RenderTranscriptRequest {
  return {
    humans: [],
    participant_human_ids: [],
    self_human_id: null,
    transcripts: [
      {
        assignments,
        started_at: null,
        words: wordIds.map((id, index) => ({
          id,
          text: id,
          start_ms: index * 100,
          end_ms: index * 100 + 100,
          channel: 2,
          speaker_index: null,
        })),
      },
    ],
  };
}
