import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { useRenderedTranscriptData } from "./data-hooks";

const mocks = vi.hoisted(() => ({
  request: null as unknown,
  renderTranscriptSegments: vi.fn(async (request: unknown) => {
    const transcript = request as {
      transcripts: Array<{ words: Array<{ id: string }> }>;
    };
    const id = transcript.transcripts[0]?.words[0]?.id ?? "empty";

    return [
      {
        end_ms: 1,
        id,
        key: { channel: "DirectMic" },
        start_ms: 0,
        text: id,
        words: [],
      },
    ];
  }),
}));

vi.mock("../render-request-hooks", () => ({
  useTranscriptRenderData: () => ({ request: mocks.request }),
}));

vi.mock("~/stt/queries", () => ({
  useSessionTranscripts: vi.fn(() => []),
  useTranscript: vi.fn(() => null),
}));

vi.mock("~/stt/render-transcript", () => ({
  getRenderTranscriptRequestKey: vi.fn(() => "request-key"),
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

function requestWithWord(id: string) {
  return {
    humans: [],
    participant_human_ids: [],
    self_human_id: null,
    transcripts: [
      {
        assignments: [],
        started_at: null,
        words: [{ id }],
      },
    ],
  };
}

describe("useRenderedTranscriptData cache lifecycle", () => {
  it("reuses one capture baseline across remounts and rotates it for the next capture", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
    const firstRequest = requestWithWord("word-a");
    const nextRequest = requestWithWord("word-b");
    mocks.request = firstRequest;
    mocks.renderTranscriptSegments.mockClear();

    const firstMount = renderHook(
      () => useRenderedTranscriptData("transcript-1", true, 1),
      { wrapper },
    );
    await waitFor(() =>
      expect(firstMount.result.current.segments[0]?.id).toBe("word-a"),
    );
    expect(mocks.renderTranscriptSegments).toHaveBeenCalledTimes(1);
    firstMount.unmount();

    mocks.request = nextRequest;
    const sameCaptureRemount = renderHook(
      () => useRenderedTranscriptData("transcript-1", true, 1),
      { wrapper },
    );
    await waitFor(() =>
      expect(sameCaptureRemount.result.current.segments[0]?.id).toBe("word-a"),
    );
    expect(mocks.renderTranscriptSegments).toHaveBeenCalledTimes(1);
    sameCaptureRemount.unmount();

    const nextCaptureMount = renderHook(
      () => useRenderedTranscriptData("transcript-1", true, 2),
      { wrapper },
    );
    await waitFor(() =>
      expect(nextCaptureMount.result.current.segments[0]?.id).toBe("word-b"),
    );
    expect(mocks.renderTranscriptSegments).toHaveBeenCalledTimes(2);
    expect(mocks.renderTranscriptSegments).toHaveBeenLastCalledWith(
      nextRequest,
    );

    nextCaptureMount.unmount();
    queryClient.clear();
  });
});
