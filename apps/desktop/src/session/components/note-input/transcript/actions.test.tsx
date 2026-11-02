import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { beginCloudsyncActivity, endCloudsyncActivity } from "@anlg/plugin-db";

const mocks = vi.hoisted(() => ({
  audioPath: vi.fn(),
  handleBatchFailed: vi.fn(),
  queueAutoEnhanceIfSummaryEmpty: vi.fn(),
  runBatch: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: { audioPath: mocks.audioPath },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.toastError },
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({
    queueAutoEnhanceIfSummaryEmpty: mocks.queueAutoEnhanceIfSummaryEmpty,
  }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({ handleBatchFailed: mocks.handleBatchFailed }),
}));

vi.mock("~/stt/useRunBatch", () => ({
  isStoppedTranscriptionError: (error: unknown) =>
    error instanceof Error && error.message === "Transcription stopped.",
  useRunBatch: () => mocks.runBatch,
}));

import { useRegenerateTranscript } from "./actions";

describe("useRegenerateTranscript", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.audioPath.mockResolvedValue({
      status: "ok",
      data: "/tmp/session.wav",
    });
  });

  it("shows batch transcription failures even when an old transcript exists", async () => {
    mocks.runBatch.mockRejectedValue(new Error("Authentication failed"));
    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(mocks.runBatch).toHaveBeenCalledWith("/tmp/session.wav", {
      promotion: { scope: "whole_session" },
    });
    expect(mocks.handleBatchFailed).toHaveBeenCalledWith(
      "session-1",
      "Authentication failed",
    );
    expect(mocks.toastError).toHaveBeenCalledWith("Re-transcription failed", {
      id: "transcript-regenerate-failed-session-1",
      description: "Authentication failed",
    });
  });

  it("finishes retranscription without generating a summary", async () => {
    mocks.runBatch.mockResolvedValue(undefined);
    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => await result.current());
    expect(mocks.queueAutoEnhanceIfSummaryEmpty).not.toHaveBeenCalled();

    expect(beginCloudsyncActivity).toHaveBeenCalledWith(
      "transcription",
      expect.stringMatching(/^session-1:retranscription:/),
    );
    expect(endCloudsyncActivity).toHaveBeenCalledWith(
      "transcription",
      vi.mocked(beginCloudsyncActivity).mock.calls[0]?.[1],
    );
  });
});
