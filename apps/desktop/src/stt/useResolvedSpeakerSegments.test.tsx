import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  RenderTranscriptRequest,
  RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

import type { Segment } from "./live-segment";
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

    act(() => {
      resolveNext?.({
        status: "ok",
        data: [
          labelSegment(updated[0]!, "John", "human-john"),
          labelSegment(updated[1]!, "Artem", "human-artem"),
        ],
      });
    });

    await waitFor(() => expect(result.current[1]?.words).toHaveLength(2));
    expect(result.current.map((segment) => segment.speaker_label)).toEqual([
      "John",
      "Artem",
    ]);
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
  humanId: string,
): RenderedTranscriptSegment {
  return {
    ...segment,
    speaker_label: name,
    provisional_speaker: {
      name,
      human_id: humanId,
      reason: "sole_remote_participant",
    },
  };
}
