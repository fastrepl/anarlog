import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { isAudioUploadFile, useUploadFile } from "./useUploadFile";

const {
  audioImportDataMock,
  audioImportMock,
  audioImportListenMock,
  handleBatchFailedMock,
  handleBatchStartedMock,
  updateBatchProgressMock,
  clearBatchSessionMock,
  runBatchMock,
  useStoreMock,
  useValuesMock,
  useTabsMock,
  updateSessionTabStateMock,
} = vi.hoisted(() => ({
  audioImportDataMock: vi.fn(),
  audioImportMock: vi.fn(),
  audioImportListenMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
  handleBatchStartedMock: vi.fn(),
  updateBatchProgressMock: vi.fn(),
  clearBatchSessionMock: vi.fn(),
  runBatchMock: vi.fn(),
  useStoreMock: vi.fn(),
  useValuesMock: vi.fn(),
  useTabsMock: vi.fn(),
  updateSessionTabStateMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: vi.fn(),
  resolveResource: vi.fn((path: string) =>
    Promise.resolve(`/resources/${path}`),
  ),
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioImport: audioImportMock,
    audioImportData: audioImportDataMock,
    audioSourceMetadata: vi.fn(),
  },
  events: {
    audioImportEvent: {
      listen: audioImportListenMock,
    },
  },
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
    enhance: vi.fn(),
    queueAutoEnhanceIfSummaryEmpty: vi.fn(),
  })),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useStore: useStoreMock,
    useValues: useValuesMock,
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: useTabsMock,
}));

function createStore() {
  const sessions = new Map<string, Record<string, unknown>>([
    ["session-1", { event_json: "" }],
  ]);

  return {
    getCell: (tableId: string, rowId: string, cellId: string) =>
      tableId === "sessions" ? sessions.get(rowId)?.[cellId] : undefined,
    setCell: vi.fn(),
    setRow: vi.fn(),
  };
}

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
    audioImportListenMock.mockResolvedValue(vi.fn());
    runBatchMock.mockResolvedValue(undefined);
    useStoreMock.mockReturnValue(createStore());
    useValuesMock.mockReturnValue({ user_id: "user-1" });
    useTabsMock.mockImplementation((selector) =>
      selector({
        tabs: [],
        updateSessionTabState: updateSessionTabStateMock,
      }),
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

    await waitFor(() => {
      expect(audioImportDataMock).toHaveBeenCalled();
    });
    expect(audioImportDataMock).toHaveBeenCalledWith(
      "session-1",
      [1, 2, 3],
      "drop.wav",
    );
    expect(audioImportMock).not.toHaveBeenCalled();
    expect(runBatchMock).toHaveBeenCalledWith(
      "/vault/sessions/session-1/audio.wav",
    );
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
      );
    },
  );
});
