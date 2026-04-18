import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { getPostCaptureAction } from "./useStartListening";
import { useStartListening } from "./useStartListening";

const {
  queueAutoEnhanceMock,
  startMock,
  runBatchMock,
  useListenerMock,
  useCellMock,
  useSliceRowIdsMock,
  useTableMock,
  useResultTableMock,
  useValuesMock,
  useStoreMock,
  useIndexesMock,
  useConfigValueMock,
} = vi.hoisted(() => ({
  queueAutoEnhanceMock: vi.fn(),
  startMock: vi.fn(),
  runBatchMock: vi.fn(),
  useListenerMock: vi.fn(),
  useCellMock: vi.fn(() => ""),
  useSliceRowIdsMock: vi.fn(() => []),
  useTableMock: vi.fn(() => ({})),
  useResultTableMock: vi.fn(() => ({})),
  useValuesMock: vi.fn(),
  useStoreMock: vi.fn(),
  useIndexesMock: vi.fn(),
  useConfigValueMock: vi.fn(),
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("./useKeywords", () => ({
  useKeywords: vi.fn(() => []),
}));

vi.mock("./useRunBatch", () => ({
  STOPPED_TRANSCRIPTION_ERROR_MESSAGE: "Transcription stopped.",
  canRunBatchTranscription: vi.fn(() => true),
  isStoppedTranscriptionError: vi.fn(
    (error: unknown) =>
      (error instanceof Error ? error.message : String(error)) ===
      "Transcription stopped.",
  ),
  useRunBatch: vi.fn(() => runBatchMock),
}));

vi.mock("./useSTTConnection", () => ({
  useSTTConnection: vi.fn(() => ({
    conn: {
      provider: "hyprnote",
      model: "am-test",
      baseUrl: "http://localhost:8080",
      apiKey: "",
    },
  })),
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: vi.fn(() => ({
    queueAutoEnhance: queueAutoEnhanceMock,
  })),
}));

vi.mock("~/session/utils", () => ({
  getSessionEvent: vi.fn(() => null),
  getSessionEventById: vi.fn(() => null),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/utils", () => ({
  id: vi.fn(() => "generated-id"),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    transcriptBySession: "transcriptBySession",
    sessionParticipantsBySession: "sessionParticipantsBySession",
  },
  QUERIES: {
    sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  },
  UI: {
    useValues: useValuesMock,
    useCell: useCellMock,
    useSliceRowIds: useSliceRowIdsMock,
    useTable: useTableMock,
    useResultTable: useResultTableMock,
    useValue: (key: string) => {
      if (key === "user_id") {
        const values = useValuesMock();
        return values?.user_id;
      }
      return undefined;
    },
    useStore: useStoreMock,
    useIndexes: useIndexesMock,
  },
}));

describe("getPostCaptureAction", () => {
  test("runs batch then enhance after record-only capture finishes when audio is available", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: false,
        },
        true,
      ),
    ).toBe("batch_then_enhance");
  });

  test("enhances immediately when live transcription already completed during recording", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
        },
        true,
      ),
    ).toBe("enhance_only");
  });

  test("does nothing when batch fallback is needed but no transcription connection is available", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: false,
        },
        false,
      ),
    ).toBe("none");
  });

  test("does nothing when capture finishes without a saved audio path", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: null,
          liveTranscriptionActive: false,
        },
        true,
      ),
    ).toBe("none");
  });
});

describe("useStartListening", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    useListenerMock.mockImplementation((selector) =>
      selector({
        start: startMock,
      }),
    );
    useCellMock.mockReturnValue("");
    useSliceRowIdsMock.mockReturnValue([]);
    useTableMock.mockReturnValue({});
    useResultTableMock.mockReturnValue({});
    useValuesMock.mockReturnValue({ user_id: "user-1" });
    useIndexesMock.mockReturnValue(null);
    useConfigValueMock.mockReturnValue([]);
    useStoreMock.mockReturnValue({
      getCell: vi.fn(() => ""),
      forEachRow: vi.fn(),
      setRow: vi.fn(),
      delRow: vi.fn(),
      transaction: vi.fn((fn: () => void) => fn()),
    });
    startMock.mockResolvedValue(true);
    runBatchMock.mockResolvedValue(undefined);
  });

  test("runs batch transcription after record-only capture stops", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    expect(onStopped).toBeTypeOf("function");

    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
  });
});
