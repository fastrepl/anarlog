import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { isAudioUploadFile, useUploadFile } from "./useUploadFile";

const {
  audioSourceMetadataMock,
  audioImportDataMock,
  audioImportMock,
  audioImportListenMock,
  downloadDirMock,
  selectFileMock,
  parseSubtitleMock,
  createTranscriptMock,
  enhanceMock,
  handleBatchFailedMock,
  handleBatchStartedMock,
  updateBatchProgressMock,
  clearBatchSessionMock,
  catalogLocalSessionAudioMock,
  runBatchMock,
  useSessionMock,
  updateSessionMock,
  useTabsMock,
  updateSessionTabStateMock,
} = vi.hoisted(() => ({
  audioSourceMetadataMock: vi.fn(),
  audioImportDataMock: vi.fn(),
  audioImportMock: vi.fn(),
  audioImportListenMock: vi.fn(),
  downloadDirMock: vi.fn(),
  selectFileMock: vi.fn(),
  parseSubtitleMock: vi.fn(),
  createTranscriptMock: vi.fn(),
  enhanceMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
  handleBatchStartedMock: vi.fn(),
  updateBatchProgressMock: vi.fn(),
  clearBatchSessionMock: vi.fn(),
  catalogLocalSessionAudioMock: vi.fn(),
  runBatchMock: vi.fn(),
  useSessionMock: vi.fn(),
  updateSessionMock: vi.fn(),
  useTabsMock: vi.fn(),
  updateSessionTabStateMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: downloadDirMock,
  resolveResource: vi.fn((path: string) =>
    Promise.resolve(`/resources/${path}`),
  ),
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: selectFileMock,
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioImport: audioImportMock,
    audioImportData: audioImportDataMock,
    audioSourceMetadata: audioSourceMetadataMock,
  },
  events: {
    audioImportEvent: {
      listen: audioImportListenMock,
    },
  },
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: { parseSubtitle: parseSubtitleMock },
}));

vi.mock("./contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      handleBatchStarted: handleBatchStartedMock,
      handleBatchFailed: handleBatchFailedMock,
      updateBatchProgress: updateBatchProgressMock,
      clearBatchSession: clearBatchSessionMock,
    }),
}));

vi.mock("./useRunBatch", () => ({
  isStoppedTranscriptionError: vi.fn(() => false),
  useRunBatch: vi.fn(() => runBatchMock),
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: vi.fn(() => ({
    enhance: enhanceMock,
    queueAutoEnhanceIfSummaryEmpty: vi.fn().mockResolvedValue({
      type: "queued",
    }),
  })),
}));

vi.mock("~/session/attachments", () => ({
  catalogLocalSessionAudio: catalogLocalSessionAudioMock,
}));

vi.mock("~/session/queries", () => ({
  useSession: useSessionMock,
  useUpdateSession: () => updateSessionMock,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: useTabsMock,
}));

vi.mock("~/stt/queries", () => ({
  createTranscript: createTranscriptMock,
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("useUploadFile", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    audioImportDataMock.mockResolvedValue({
      status: "ok",
      data: "/vault/sessions/session-1/audio.wav",
    });
    audioImportMock.mockResolvedValue({
      status: "ok",
      data: "/vault/sessions/session-1/audio.wav",
    });
    audioImportListenMock.mockResolvedValue(vi.fn());
    audioSourceMetadataMock.mockResolvedValue({
      status: "ok",
      data: {
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: null,
        durationMs: 30_000,
      },
    });
    downloadDirMock.mockResolvedValue("/downloads");
    selectFileMock.mockResolvedValue("/tmp/replacement.wav");
    catalogLocalSessionAudioMock.mockResolvedValue(undefined);
    runBatchMock.mockResolvedValue(undefined);
    createTranscriptMock.mockResolvedValue(undefined);
    enhanceMock.mockResolvedValue({ type: "started", noteId: "note-1" });
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "",
      event_json: "",
    });
    updateSessionMock.mockResolvedValue(undefined);
    useTabsMock.mockImplementation((selector) =>
      selector({
        tabs: [],
        updateSessionTabState: updateSessionTabStateMock,
      }),
    );
  });

  test("infers the session date for an ordinary audio upload", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.uploadAudio();
    });

    await waitFor(() => {
      expect(updateSessionMock).toHaveBeenCalledWith({
        created_at: "2026-03-26T11:59:30.000Z",
      });
    });
    expect(audioSourceMetadataMock).toHaveBeenCalledWith(
      "/tmp/replacement.wav",
    );
  });

  test("imports pathless dropped audio using file bytes", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });
    const file = new File([new Uint8Array([1, 2, 3])], "drop.wav", {
      type: "audio/wav",
      lastModified: 1_700_000_000_000,
    });
    Object.defineProperty(file, "arrayBuffer", {
      value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
    });

    act(() => {
      result.current.processAudioFile(file);
    });

    await waitFor(() => expect(runBatchMock).toHaveBeenCalled());
    expect(audioImportDataMock).toHaveBeenCalledWith(
      "session-1",
      [1, 2, 3],
      "drop.wav",
      "audio/wav",
    );
    expect(audioImportMock).not.toHaveBeenCalled();
    expect(runBatchMock).toHaveBeenCalledWith(
      "/vault/sessions/session-1/audio.wav",
    );
    expect(catalogLocalSessionAudioMock).toHaveBeenCalledWith("session-1");
    expect(audioImportDataMock.mock.invocationCallOrder[0]).toBeLessThan(
      catalogLocalSessionAudioMock.mock.invocationCallOrder[0]!,
    );
    expect(
      catalogLocalSessionAudioMock.mock.invocationCallOrder[0],
    ).toBeLessThan(runBatchMock.mock.invocationCallOrder[0]!);
    expect(handleBatchFailedMock).not.toHaveBeenCalled();
  });

  test.each(["webm", "aac"])(
    "imports pathless .%s drops without MIME",
    async (extension) => {
      const { result } = renderHook(() => useUploadFile("session-1"), {
        wrapper: createWrapper(),
      });
      const file = new File([new Uint8Array([1, 2, 3])], `drop.${extension}`, {
        type: "",
        lastModified: 1_700_000_000_000,
      });
      Object.defineProperty(file, "arrayBuffer", {
        value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
      });

      expect(isAudioUploadFile(file)).toBe(true);

      act(() => {
        result.current.processAudioFile(file);
      });

      await waitFor(() => {
        expect(audioImportDataMock).toHaveBeenCalled();
      });
      expect(audioImportDataMock).toHaveBeenCalledWith(
        "session-1",
        [1, 2, 3],
        `drop.${extension}`,
        null,
      );
    },
  );

  test("imports copied Voice Memos audio using its QuickTime MIME", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });
    const file = new File([new Uint8Array([1, 2, 3])], "Brian Shin.qta", {
      type: "audio/quicktime",
      lastModified: 1_700_000_000_000,
    });
    Object.defineProperty(file, "arrayBuffer", {
      value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
    });

    act(() => {
      result.current.processAudioFile(file);
    });

    await waitFor(() => expect(runBatchMock).toHaveBeenCalled());
    expect(audioImportDataMock).toHaveBeenCalledWith(
      "session-1",
      [1, 2, 3],
      "Brian Shin.qta",
      "audio/quicktime",
    );
  });

  test("continues transcription when imported audio cataloging fails", async () => {
    catalogLocalSessionAudioMock.mockRejectedValueOnce(
      new Error("catalog unavailable"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });
    const file = new File([new Uint8Array([1, 2, 3])], "drop.wav", {
      type: "audio/wav",
    });
    Object.defineProperty(file, "arrayBuffer", {
      value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
    });

    act(() => result.current.processAudioFile(file));

    await waitFor(() => expect(runBatchMock).toHaveBeenCalled());
    expect(handleBatchFailedMock).not.toHaveBeenCalled();
    expect(consoleError).toHaveBeenCalledWith(
      "[upload] failed to catalog imported audio",
      expect.any(Error),
    );
    consoleError.mockRestore();
  });

  test("persists imported subtitles before enhancing", async () => {
    let resolveWrite: (() => void) | undefined;
    createTranscriptMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveWrite = resolve;
        }),
    );
    parseSubtitleMock.mockResolvedValue({
      status: "ok",
      data: {
        tokens: [{ text: "Hello", start_time: 0, end_time: 500 }],
      },
    });
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/session.vtt", "transcript");
    });

    await waitFor(() => {
      expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    });
    expect(enhanceMock).not.toHaveBeenCalled();

    await act(async () => {
      resolveWrite?.();
    });
    await waitFor(() => {
      expect(enhanceMock).toHaveBeenCalledWith("session-1");
    });
    expect(createTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: "session-1",
        ownerUserId: "user-1",
        source: "subtitle_import",
        words: [
          expect.objectContaining({
            text: "Hello",
            start_ms: 0,
            end_ms: 500,
          }),
        ],
      }),
    );
    expect(createTranscriptMock.mock.invocationCallOrder[0]).toBeLessThan(
      enhanceMock.mock.invocationCallOrder[0],
    );
  });
});
