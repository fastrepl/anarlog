import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  RenderTranscriptRequest,
  RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

import type { Segment, SegmentWord } from "./live-segment";
import { useResolvedSpeakerSegments } from "./useResolvedSpeakerSegments";

const mocks = vi.hoisted(() => ({
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    renderTranscriptSegments: mocks.renderTranscriptSegments,
  },
}));

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

describe("useResolvedSpeakerSegments", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("keeps the previous speaker resolution while a live update is re-resolved", async () => {
    const request = createRequest();
    const initial = [createSegment("self", 0), createSegment("remote", 1)];
    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        labelSegment(initial[0]!, "John", "human-john"),
        labelSegment(initial[1]!, "Artem", "human-artem"),
      ],
    });

    const { rerender, result } = renderHook(
      ({ segments }) => useResolvedSpeakerSegments(segments, request),
      { initialProps: { segments: initial }, wrapper },
    );
    await waitFor(() =>
      expect(result.current.map((segment) => segment.speaker_label)).toEqual([
        "John",
        "Artem",
      ]),
    );

    let resolveNext:
      | ((value: { status: "ok"; data: RenderedTranscriptSegment[] }) => void)
      | undefined;
    mocks.renderTranscriptSegments.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveNext = resolve;
        }),
    );
    const partial = {
      text: " trip",
      start_ms: 150,
      end_ms: 190,
      channel: "RemoteParty" as const,
      is_final: false,
    };
    const updated = [
      initial[0]!,
      {
        ...initial[1]!,
        end_ms: partial.end_ms,
        text: `${initial[1]!.text}${partial.text}`,
        words: [...initial[1]!.words, partial],
      },
    ];
    rerender({ segments: updated });

    expect(mocks.renderTranscriptSegments).toHaveBeenCalledTimes(2);
    expect(result.current.map((segment) => segment.speaker_label)).toEqual([
      "John",
      "Artem",
    ]);
    expect(result.current[1]?.text).toBe(updated[1]!.text);
    expect(result.current[1]?.words).toEqual(updated[1]!.words);
    expect(result.current[1]?.provisional_speaker?.human_id).toBe(
      "human-artem",
    );

    act(() => {
      resolveNext?.({
        status: "ok",
        data: [
          labelSegment(updated[0]!, "John", "human-john"),
          labelSegment(updated[1]!, "Speaker 1", null),
        ],
      });
    });

    await waitFor(() =>
      expect(result.current.map((segment) => segment.speaker_label)).toEqual([
        "John",
        "Speaker 1",
      ]),
    );
    expect(result.current[1]?.words).toHaveLength(2);
    expect(result.current[1]?.provisional_speaker).toBeUndefined();
  });

  it("shows new segments before the renderer resolves, carrying labels by speaker key", async () => {
    const request = createRequest();
    const initial = [createSegment("self", 0), createSegment("remote", 1)];
    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        labelSegment(initial[0]!, "John", "human-john"),
        labelSegment(initial[1]!, "Artem", "human-artem"),
      ],
    });

    const { rerender, result } = renderHook(
      ({ segments }) => useResolvedSpeakerSegments(segments, request),
      { initialProps: { segments: initial }, wrapper },
    );
    await waitFor(() =>
      expect(result.current.map((segment) => segment.speaker_label)).toEqual([
        "John",
        "Artem",
      ]),
    );

    mocks.renderTranscriptSegments.mockImplementationOnce(
      () => new Promise(() => {}),
    );
    const partial = {
      text: "later",
      start_ms: 300,
      end_ms: 340,
      channel: "RemoteParty" as const,
      is_final: false,
      metadata: { timing: { source: "provider_word" } },
    };
    const sameSpeaker: Segment = {
      id: "segment-remote-later",
      key: initial[1]!.key,
      start_ms: partial.start_ms,
      end_ms: partial.end_ms,
      text: partial.text,
      words: [partial],
    };
    const unknownSpeaker: Segment = {
      ...createSegment("mixed", 4),
      key: {
        channel: "MixedCapture",
        speaker_index: 2,
        speaker_human_id: null,
      },
    };
    rerender({ segments: [...initial, sameSpeaker, unknownSpeaker] });

    expect(result.current.map((segment) => segment.text)).toEqual([
      "word-self",
      "word-remote",
      "later",
      "word-mixed",
    ]);
    expect(result.current.map((segment) => segment.speaker_label)).toEqual([
      "John",
      "Artem",
      "Artem",
      undefined,
    ]);
    expect(result.current[2]?.words[0]?.metadata).toEqual(partial.metadata);
    expect(result.current[3]?.provisional_speaker).toBeUndefined();
  });

  it("keeps a context-boundary split while the extended segment is re-resolved", async () => {
    const request = createRequest();
    const before = {
      id: "w-before",
      text: "before",
      start_ms: 0,
      end_ms: 50,
      channel: "RemoteParty" as const,
      is_final: true,
    };
    const after = {
      ...before,
      id: "w-after",
      text: " after",
      start_ms: 100,
      end_ms: 150,
    };
    const spanning: Segment = {
      id: "segment-span",
      key: { channel: "RemoteParty", speaker_index: 0, speaker_human_id: null },
      start_ms: before.start_ms,
      end_ms: after.end_ms,
      text: "before after",
      words: [before, after],
    };
    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        labelSegment(
          {
            ...spanning,
            id: "segment-span:0",
            end_ms: 50,
            text: "before",
            words: [before],
          },
          "Speaker 1",
          null,
        ),
        labelSegment(
          {
            ...spanning,
            id: "segment-span:100",
            start_ms: 100,
            text: "after",
            words: [after],
          },
          "Artem",
          "human-artem",
        ),
      ],
    });

    const { rerender, result } = renderHook(
      ({ segments }) => useResolvedSpeakerSegments(segments, request),
      { initialProps: { segments: [spanning] }, wrapper },
    );
    await waitFor(() =>
      expect(result.current.map((segment) => segment.speaker_label)).toEqual([
        "Speaker 1",
        "Artem",
      ]),
    );

    mocks.renderTranscriptSegments.mockImplementationOnce(
      () => new Promise(() => {}),
    );
    const partial = {
      text: " still",
      start_ms: 200,
      end_ms: 240,
      channel: "RemoteParty" as const,
      is_final: false,
    };
    rerender({
      segments: [
        {
          ...spanning,
          id: "segment-span:extended",
          end_ms: partial.end_ms,
          text: `${spanning.text}${partial.text}`,
          words: [before, after, partial],
        },
      ],
    });

    expect(
      result.current.map(({ id, speaker_label, text, start_ms, end_ms }) => ({
        id,
        speaker_label,
        text,
        start_ms,
        end_ms,
      })),
    ).toEqual([
      {
        id: "segment-span:extended:0",
        speaker_label: "Speaker 1",
        text: "before",
        start_ms: 0,
        end_ms: 50,
      },
      {
        id: "segment-span:extended:100",
        speaker_label: "Artem",
        text: "after still",
        start_ms: 100,
        end_ms: 240,
      },
    ]);
    expect(result.current[1]?.words).toEqual([after, partial]);
    expect(result.current[1]?.provisional_speaker?.human_id).toBe(
      "human-artem",
    );
  });

  it("scopes carried speaker names to the matching context interval", async () => {
    const base = createRequest();
    const bob = { human_id: "human-bob", name: "Bob" };
    const request: RenderTranscriptRequest = {
      ...base,
      humans: [...base.humans, bob],
      participant_human_ids: ["human-artem", "human-bob"],
      speaker_context: {
        intervals: [
          base.speaker_context!.intervals[0]!,
          {
            ...base.speaker_context!.intervals[0]!,
            start_ms: 60_000,
            end_ms: 120_000,
            title: "John x Bob",
            participants: [bob],
          },
        ],
      },
    };
    const remoteKey = {
      channel: "RemoteParty" as const,
      speaker_index: 0,
      speaker_human_id: null,
    };
    const remoteWord = (text: string, start_ms: number, id?: string) => ({
      ...(id ? { id } : {}),
      text,
      start_ms,
      end_ms: start_ms + 40,
      channel: "RemoteParty" as const,
      is_final: Boolean(id),
    });
    const remoteSegment = (id: string, words: SegmentWord[]): Segment => ({
      id,
      key: remoteKey,
      start_ms: words[0]!.start_ms,
      end_ms: words[words.length - 1]!.end_ms,
      text: words
        .map((word) => word.text)
        .join("")
        .trim(),
      words,
    });
    const early = remoteSegment("segment-early", [
      remoteWord("early", 1_000, "w-early"),
    ]);
    const edge = remoteSegment("segment-edge", [
      remoteWord("edge", 59_900, "w-edge"),
    ]);
    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        labelSegment(early, "Artem", "human-artem"),
        labelSegment(edge, "Artem", "human-artem"),
      ],
    });

    const { rerender, result } = renderHook(
      ({ segments }) => useResolvedSpeakerSegments(segments, request),
      { initialProps: { segments: [early, edge] }, wrapper },
    );
    await waitFor(() =>
      expect(result.current.map((segment) => segment.speaker_label)).toEqual([
        "Artem",
        "Artem",
      ]),
    );

    mocks.renderTranscriptSegments.mockImplementationOnce(
      () => new Promise(() => {}),
    );
    const sameCall = remoteSegment("segment-same-call", [
      remoteWord("still artem", 2_000),
    ]);
    const straddling = remoteSegment("segment-edge:extended", [
      ...edge.words,
      remoteWord(" crossing", 60_100),
    ]);
    const nextCall = remoteSegment("segment-next-call", [
      remoteWord("hello", 61_000),
    ]);
    rerender({ segments: [early, sameCall, straddling, nextCall] });

    expect(
      result.current.map(({ id, speaker_label, text }) => ({
        id,
        speaker_label,
        text,
      })),
    ).toEqual([
      { id: "segment-early", speaker_label: "Artem", text: "early" },
      { id: "segment-same-call", speaker_label: "Artem", text: "still artem" },
      {
        id: "segment-edge:extended:59900",
        speaker_label: "Artem",
        text: "edge",
      },
      {
        id: "segment-edge:extended:60100",
        speaker_label: undefined,
        text: "crossing",
      },
      { id: "segment-next-call", speaker_label: undefined, text: "hello" },
    ]);
    expect(result.current[4]?.provisional_speaker).toBeUndefined();
  });

  it("falls back to unresolved segments before the first resolution lands", () => {
    mocks.renderTranscriptSegments.mockImplementationOnce(
      () => new Promise(() => {}),
    );
    const segments = [createSegment("self", 0)];

    const { result } = renderHook(
      () => useResolvedSpeakerSegments(segments, createRequest()),
      { wrapper },
    );

    expect(result.current).toHaveLength(1);
    expect(result.current[0]?.speaker_label).toBeUndefined();
    expect(result.current[0]?.words).toBe(segments[0]?.words);
  });

  it("returns segments untouched without recording context", () => {
    const segments = [createSegment("self", 0)];

    const { result } = renderHook(
      () => useResolvedSpeakerSegments(segments, null),
      { wrapper },
    );

    expect(result.current).toBe(segments);
    expect(mocks.renderTranscriptSegments).not.toHaveBeenCalled();
  });
});

function createRequest(): RenderTranscriptRequest {
  return {
    humans: [
      { human_id: "human-john", name: "John" },
      { human_id: "human-artem", name: "Artem" },
    ],
    participant_human_ids: ["human-artem"],
    self_human_id: "human-john",
    speaker_context: {
      intervals: [
        {
          start_ms: 0,
          end_ms: 60_000,
          active_call: true,
          calendar_call: false,
          mic_isolated: true,
          shared_microphone: false,
          title: "John x Artem",
          self_names: ["John"],
          participants: [{ human_id: "human-artem", name: "Artem" }],
        },
      ],
    },
    transcripts: [{ started_at: 0, words: [], assignments: [] }],
  };
}

function createSegment(id: string, index: number): Segment {
  const channel = index === 0 ? "DirectMic" : "RemoteParty";
  return {
    id: `segment-${id}`,
    key: { channel, speaker_index: 0, speaker_human_id: null },
    start_ms: index * 100,
    end_ms: index * 100 + 50,
    text: `word-${id}`,
    words: [
      {
        id: `word-${id}`,
        text: `word-${id}`,
        start_ms: index * 100,
        end_ms: index * 100 + 50,
        channel,
        is_final: true,
      },
    ],
  };
}

function labelSegment(
  segment: Segment,
  name: string,
  humanId: string | null,
): RenderedTranscriptSegment {
  return {
    ...segment,
    speaker_label: name,
    provisional_speaker: humanId
      ? { name, human_id: humanId, reason: "sole_remote_participant" }
      : undefined,
  };
}
