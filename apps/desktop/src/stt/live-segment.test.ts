import { describe, expect, it } from "vitest";

import type {
  IdentityAssignment,
  RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import {
  applyRenderRequestIdentitiesToSegments,
  getMaxSpeakerNumberForParticipants,
  mergeAdjacentSpeakerSegments,
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
  it.each([
    {
      name: "diarized direct-mic segments are self",
      key: {
        channel: "DirectMic",
        speaker_index: 2,
        speaker_human_id: null,
      } as Parameters<typeof SegmentKeyUtils.renderLabel>[0],
      context: ctx,
      known: true,
      label: "Me",
    },
    {
      name: "assigned direct-mic segments are not self when the name is unavailable",
      key: {
        channel: "DirectMic",
        speaker_index: 1,
        speaker_human_id: "remote",
      } as Parameters<typeof SegmentKeyUtils.renderLabel>[0],
      context: ctx,
      known: undefined,
      label: "Speaker 2",
    },
    {
      name: "an unnamed assigned self is You on the direct mic",
      key: {
        channel: "DirectMic",
        speaker_index: null,
        speaker_human_id: "self",
      } as Parameters<typeof SegmentKeyUtils.renderLabel>[0],
      context: {
        getSelfHumanId: () => "self",
        getHumanName: () => undefined,
      } as RenderLabelContext,
      known: undefined,
      label: "You",
    },
    {
      name: "remote-party segments label as the unique other participant",
      key: {
        channel: "RemoteParty",
        speaker_index: 0,
        speaker_human_id: null,
      } as Parameters<typeof SegmentKeyUtils.renderLabel>[0],
      context: twoPersonCtx,
      known: true,
      label: "Artem",
    },
  ])("$name", ({ key, context, known, label }) => {
    if (known !== undefined) {
      expect(SegmentKeyUtils.isKnownSpeaker(key, context)).toBe(known);
    }
    expect(SegmentKeyUtils.renderLabel(key, context)).toBe(label);
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

describe("mergeRenderedAndLiveSegments", () => {
  it("preserves the rendered order when there is no live tail", () => {
    const micEarly = createSegment("mic-early", [
      { id: "mic-early", startMs: 0 },
    ]);
    const micLate = createSegment("mic-late", [
      { id: "mic-late", startMs: 29_500 },
    ]);
    const remoteEarly = createSegment("remote-early", [
      { id: "remote-early", startMs: 0 },
    ]);

    expect(
      mergeRenderedAndLiveSegments([micEarly, micLate, remoteEarly], []),
    ).toEqual([micEarly, micLate, remoteEarly]);
  });

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

  it("clears the stale speaker label when the applied human changes", () => {
    const segment = createSegment("live", [{ id: "word-a", startMs: 0 }]);
    segment.key.speaker_index = 1;
    segment.speaker_label = "Speaker 1";
    const request = createRequest(
      ["word-a"],
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

    const [result] = applyRenderRequestIdentitiesToSegments([segment], request);

    expect(result?.key.speaker_human_id).toBe("human-1");
    expect(result?.speaker_label).toBeUndefined();
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

  it("does not apply channel defaults to distinct mixed-capture speakers", () => {
    const first = createSegment("first", [{ id: "word-a", startMs: 0 }]);
    const second = createSegment("second", [{ id: "word-b", startMs: 100 }]);
    first.key.speaker_index = 0;
    second.key.speaker_index = 1;
    const request = createRequest(
      ["word-a", "word-b"],
      [
        {
          human_id: "human-1",
          scope: { kind: "channel", channel: "MixedCapture" },
        },
      ],
    );
    request.participant_human_ids = ["self", "remote", "guest"];
    request.self_human_id = "self";

    const result = applyRenderRequestIdentitiesToSegments(
      [first, second],
      request,
    );

    expect(result.map((segment) => segment.key.speaker_human_id)).toEqual([
      null,
      null,
    ]);
  });
});

describe("mergeAdjacentSpeakerSegments", () => {
  it.each([
    {
      name: "a speaker tag resolves to the same human",
      firstKey: { speaker_index: 1 } as Partial<Segment["key"]>,
      secondKey: { speaker_index: 1 } as Partial<Segment["key"]>,
      request: () =>
        createRequest(
          ["word-a", "word-b"],
          [
            {
              human_id: "human-1",
              scope: {
                kind: "channel_speaker" as const,
                channel: "MixedCapture" as const,
                speaker_index: 1,
              },
            },
          ],
        ),
      extra: (merged: Segment[]) => {
        expect(merged[0]?.key.speaker_human_id).toBe("human-1");
        expect(merged[0]?.words.map((word) => word.id)).toEqual([
          "word-a",
          "word-b",
        ]);
        expect(merged[0]?.start_ms).toBe(0);
        expect(merged[0]?.end_ms).toBe(200);
        expect(merged[0]?.text).toBe("word-word-a word-word-b");
      },
    },
    {
      name: "adjacent segments share a diarized speaker without a human",
      firstKey: { speaker_index: 2 } as Partial<Segment["key"]>,
      secondKey: { speaker_index: 2 } as Partial<Segment["key"]>,
      request: undefined,
      extra: (merged: Segment[], first: Segment) => {
        expect(merged[0]?.id).not.toBe(first.id);
        expect(merged[0]?.key).toEqual(first.key);
      },
    },
    {
      name: "same-human segments across different diarized indices",
      firstKey: {
        channel: "RemoteParty",
        speaker_index: 0,
        speaker_human_id: "human-1",
      } as Partial<Segment["key"]>,
      secondKey: {
        channel: "RemoteParty",
        speaker_index: 1,
        speaker_human_id: "human-1",
      } as Partial<Segment["key"]>,
      request: undefined,
      extra: (merged: Segment[], first: Segment) => {
        expect(merged[0]?.key).toEqual(first.key);
      },
    },
  ])("merges $name", ({ firstKey, secondKey, request, extra }) => {
    const first = createSegment("first", [{ id: "word-a", startMs: 0 }]);
    const second = createSegment("second", [{ id: "word-b", startMs: 100 }]);
    Object.assign(first.key, firstKey);
    Object.assign(second.key, secondKey);

    const segments = request
      ? applyRenderRequestIdentitiesToSegments([first, second], request())
      : [first, second];
    const merged = mergeAdjacentSpeakerSegments(segments);

    expect(merged).toHaveLength(1);
    extra(merged, first);
  });

  it.each([
    {
      name: "segments for different speakers",
      firstKey: {
        channel: "RemoteParty",
        speaker_index: 0,
        speaker_human_id: "human-1",
      } as Partial<Segment["key"]>,
      secondKey: {
        channel: "RemoteParty",
        speaker_index: 0,
        speaker_human_id: "human-2",
      } as Partial<Segment["key"]>,
    },
    {
      name: "a human-tagged segment into an untagged one",
      firstKey: {
        channel: "MixedCapture",
        speaker_index: 1,
        speaker_human_id: "human-1",
      } as Partial<Segment["key"]>,
      secondKey: {
        channel: "MixedCapture",
        speaker_index: 1,
        speaker_human_id: null,
      } as Partial<Segment["key"]>,
    },
    {
      name: "across channels",
      firstKey: {
        channel: "DirectMic",
        speaker_index: null,
        speaker_human_id: "human-1",
      } as Partial<Segment["key"]>,
      secondKey: {
        channel: "RemoteParty",
        speaker_index: null,
        speaker_human_id: "human-1",
      } as Partial<Segment["key"]>,
    },
    {
      name: "anonymous same-channel segments",
      firstKey: {} as Partial<Segment["key"]>,
      secondKey: {} as Partial<Segment["key"]>,
    },
  ])("does not merge $name", ({ firstKey, secondKey }) => {
    const first = createSegment("first", [{ id: "word-a", startMs: 0 }]);
    const second = createSegment("second", [{ id: "word-b", startMs: 100 }]);
    Object.assign(first.key, firstKey);
    Object.assign(second.key, secondKey);

    expect(mergeAdjacentSpeakerSegments([first, second])).toHaveLength(2);
  });

  it.each([
    {
      name: "restores the leading space that first-word normalization stripped",
      adjust: (first: Segment, second: Segment) => {
        first.words = first.words.map((word) => ({
          ...word,
          text: word.text.trimStart(),
        }));
        second.words = second.words.map((word) => ({
          ...word,
          text: word.text.trimStart(),
        }));
      },
      expectedText: "word-word-a word-word-b",
      expectedWords: ["word-word-a", " word-word-b"],
    },
    {
      name: "does not insert a space before a punctuation-leading word",
      adjust: (_first: Segment, second: Segment) => {
        second.words = [{ ...second.words[0]!, text: ". Next" }];
      },
      expectedText: "word-word-a. Next",
      expectedWords: undefined,
    },
  ])("$name", ({ adjust, expectedText, expectedWords }) => {
    const first = createSegment("first", [{ id: "word-a", startMs: 0 }]);
    const second = createSegment("second", [{ id: "word-b", startMs: 100 }]);
    first.key.speaker_index = 1;
    second.key.speaker_index = 1;
    adjust(first, second);

    const merged = mergeAdjacentSpeakerSegments([first, second]);

    expect(merged).toHaveLength(1);
    expect(merged[0]?.text).toBe(expectedText);
    if (expectedWords) {
      expect(merged[0]?.words.map((word) => word.text)).toEqual(expectedWords);
    }
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
