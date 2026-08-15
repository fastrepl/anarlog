import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TRANSCRIPT_RENDER_CACHE_TIME_MS } from "../cache";
import { useRenderedTranscriptData } from "./data-hooks";

const mocks = vi.hoisted(() => ({
  getRenderTranscriptRequestKey: vi.fn((_request: unknown) => "request-key"),
  keepPreviousData: vi.fn((previous: unknown) => previous),
  renderTranscriptSegments: vi.fn(),
  useQuery: vi.fn(),
  useTranscriptRenderData: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  keepPreviousData: mocks.keepPreviousData,
  useQuery: mocks.useQuery,
}));

vi.mock("../render-request-hooks", () => ({
  useTranscriptRenderData: mocks.useTranscriptRenderData,
}));

vi.mock("~/stt/queries", () => ({
  useSessionTranscriptMetadata: vi.fn(() => []),
  useTranscriptMetadata: vi.fn(() => null),
}));

vi.mock("~/stt/render-transcript", () => ({
  getRenderTranscriptRequestKey: mocks.getRenderTranscriptRequestKey,
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

describe("useRenderedTranscriptData", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useQuery.mockReturnValue({ data: [] });
    mocks.useTranscriptRenderData.mockReturnValue({
      request: {
        humans: [],
        participant_human_ids: [],
        self_human_id: undefined,
        transcripts: [],
      },
    });
  });

  it("keeps settled transcript data cached across short tab remounts", () => {
    renderHook(() => useRenderedTranscriptData("transcript-1"));

    expect(mocks.useQuery).toHaveBeenCalledWith(
      expect.objectContaining({
        queryKey: [
          "rendered-transcript-segments",
          "transcript-1",
          "settled",
          "request-key",
        ],
        enabled: true,
        placeholderData: mocks.keepPreviousData,
        staleTime: Number.POSITIVE_INFINITY,
        gcTime: TRANSCRIPT_RENDER_CACHE_TIME_MS,
      }),
    );
  });

  it("keeps one reusable render baseline while the transcript is active", () => {
    renderHook(() => useRenderedTranscriptData("transcript-1", true));

    expect(mocks.useQuery).toHaveBeenCalledWith(
      expect.objectContaining({
        queryKey: [
          "rendered-transcript-segments",
          "transcript-1",
          "volatile",
          "baseline:0",
        ],
        enabled: true,
        staleTime: Number.POSITIVE_INFINITY,
        gcTime: TRANSCRIPT_RENDER_CACHE_TIME_MS,
      }),
    );
  });

  it("keeps one persisted baseline while exposing current active data", async () => {
    const requestA = {
      humans: [],
      participant_human_ids: [],
      self_human_id: null,
      transcripts: [
        {
          assignments: [],
          started_at: null,
          words: [{ id: "word-a" }],
        },
      ],
    };
    const requestB = {
      humans: [],
      participant_human_ids: [],
      self_human_id: null,
      transcripts: [
        {
          assignments: [],
          started_at: null,
          words: [{ id: "word-b" }],
        },
      ],
    };
    const assignment = {
      human_id: "human-1",
      scope: { channel: "MixedCapture", kind: "channel" },
    };
    const requestC = {
      ...requestB,
      humans: [{ human_id: "human-1", name: "Ada" }],
      transcripts: [{ ...requestB.transcripts[0], assignments: [assignment] }],
    };
    const requestD = {
      ...requestC,
      transcripts: [
        {
          ...requestC.transcripts[0],
          assignments: [
            {
              human_id: "human-1",
              scope: {
                kind: "words",
                word_ids: ["word-b", "word-c"],
              },
            },
          ],
        },
      ],
    };
    mocks.getRenderTranscriptRequestKey.mockImplementation((request) => {
      if (request === requestA) return "baseline-a";
      if (request === requestB) return "request-b";
      if (request === requestC) return "request-c";
      if (request === requestD) return "request-d";
      return "empty";
    });
    mocks.useTranscriptRenderData.mockReturnValue({ request: null });

    const { rerender, result } = renderHook(
      ({ currentActive, captureGeneration }) =>
        useRenderedTranscriptData(
          "transcript-1",
          currentActive,
          captureGeneration,
        ),
      { initialProps: { currentActive: true, captureGeneration: 1 } },
    );

    expect(mocks.useQuery.mock.lastCall?.[0].enabled).toBe(false);

    mocks.useTranscriptRenderData.mockReturnValue({ request: requestA });
    rerender({ currentActive: true, captureGeneration: 1 });
    mocks.useTranscriptRenderData.mockReturnValue({ request: requestB });
    rerender({ currentActive: true, captureGeneration: 1 });

    const activeQuery = mocks.useQuery.mock.lastCall?.[0];
    expect(activeQuery.queryKey).toEqual([
      "rendered-transcript-segments",
      "transcript-1",
      "volatile",
      "baseline:1",
    ]);
    await activeQuery.queryFn();
    expect(mocks.renderTranscriptSegments).toHaveBeenLastCalledWith(
      expect.objectContaining({
        transcripts: [
          expect.objectContaining({
            assignments: [],
            words: requestA.transcripts[0]?.words,
          }),
        ],
      }),
    );

    mocks.useTranscriptRenderData.mockReturnValue({ request: requestC });
    rerender({ currentActive: true, captureGeneration: 1 });
    mocks.useTranscriptRenderData.mockReturnValue({ request: requestD });
    rerender({ currentActive: true, captureGeneration: 1 });

    const updatedActiveQuery = mocks.useQuery.mock.lastCall?.[0];
    expect(updatedActiveQuery.queryKey).toEqual([
      "rendered-transcript-segments",
      "transcript-1",
      "volatile",
      "baseline:1",
    ]);
    await updatedActiveQuery.queryFn();
    expect(mocks.renderTranscriptSegments).toHaveBeenLastCalledWith(
      expect.objectContaining({
        transcripts: [
          expect.objectContaining({
            assignments: [],
            words: requestA.transcripts[0]?.words,
          }),
        ],
      }),
    );
    expect(result.current.request).toBe(requestD);

    rerender({ currentActive: false, captureGeneration: 1 });

    const settledQuery = mocks.useQuery.mock.lastCall?.[0];
    expect(settledQuery.queryKey).toEqual([
      "rendered-transcript-segments",
      "transcript-1",
      "settled",
      "request-d",
    ]);
    await settledQuery.queryFn();
    expect(mocks.renderTranscriptSegments).toHaveBeenLastCalledWith(requestD);

    rerender({ currentActive: true, captureGeneration: 2 });

    const nextCaptureQuery = mocks.useQuery.mock.lastCall?.[0];
    expect(nextCaptureQuery.queryKey).toEqual([
      "rendered-transcript-segments",
      "transcript-1",
      "volatile",
      "baseline:2",
    ]);
    await nextCaptureQuery.queryFn();
    expect(mocks.renderTranscriptSegments).toHaveBeenLastCalledWith(requestD);
  });
});
