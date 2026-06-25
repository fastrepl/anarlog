import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TRANSCRIPT_RENDER_CACHE_TIME_MS } from "../cache";
import { useRenderedTranscriptData } from "./data-hooks";

const mocks = vi.hoisted(() => ({
  getRenderTranscriptRequestKey: vi.fn(() => "request-key"),
  renderTranscriptSegments: vi.fn(),
  useQuery: vi.fn(),
  useTranscriptRenderData: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: mocks.useQuery,
}));

vi.mock("../render-request-hooks", () => ({
  useTranscriptRenderData: mocks.useTranscriptRenderData,
  useTranscriptRowsRevision: vi.fn(() => 0),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: vi.fn(() => "session-1"),
    useSliceRowIds: vi.fn(() => ["transcript-1"]),
    useStore: vi.fn(() => ({
      getCell: vi.fn(() => 0),
    })),
  },
  INDEXES: {
    transcriptBySession: "transcriptBySession",
  },
}));

vi.mock("~/stt/render-transcript", () => ({
  getRenderTranscriptRequestKey: mocks.getRenderTranscriptRequestKey,
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

describe("useRenderedTranscriptData", () => {
  beforeEach(() => {
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

  it("keeps rendered transcript data cached across short tab remounts", () => {
    renderHook(() => useRenderedTranscriptData("transcript-1"));

    expect(mocks.useQuery).toHaveBeenCalledWith(
      expect.objectContaining({
        queryKey: [
          "rendered-transcript-segments",
          "transcript-1",
          "request-key",
        ],
        enabled: true,
        staleTime: Number.POSITIVE_INFINITY,
        gcTime: TRANSCRIPT_RENDER_CACHE_TIME_MS,
      }),
    );
  });
});
