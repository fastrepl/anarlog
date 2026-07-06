import { act, cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Transcript } from "./index";

const {
  useSliceRowIdsMock,
  useStoreMock,
  useListenerMock,
  useAudioPlayerMock,
} = vi.hoisted(() => ({
  useSliceRowIdsMock: vi.fn(),
  useStoreMock: vi.fn(),
  useListenerMock: vi.fn(),
  useAudioPlayerMock: vi.fn(),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    transcriptBySession: "transcriptBySession",
  },
  UI: {
    useSliceRowIds: useSliceRowIdsMock,
    useStore: useStoreMock,
    useCheckpoints: vi.fn(() => null),
    useIndexes: vi.fn(() => null),
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: useAudioPlayerMock,
}));

vi.mock("./screens/batch", () => ({
  BatchState: () => <div data-testid="batch-state" />,
}));

vi.mock("./screens/empty", () => ({
  TranscriptEmptyState: () => <div data-testid="empty-state" />,
}));

vi.mock("./screens/listening", () => ({
  TranscriptListeningState: ({ status }: { status: string }) => (
    <div data-testid="listening-state">{status}</div>
  ),
}));

vi.mock("./renderer", () => ({
  TranscriptViewer: () => <div data-testid="transcript-viewer" />,
}));

vi.mock("~/stt/useUploadFile", () => ({
  useUploadFile: vi.fn(() => ({
    uploadAudio: vi.fn(),
    uploadTranscript: vi.fn(),
    processFile: vi.fn(),
  })),
}));

vi.mock("~/stt/pending-upload", () => ({
  consumePendingUpload: vi.fn(() => null),
}));

describe("Transcript", () => {
  const sessionId = "session-1";
  const transcriptId = "transcript-1";

  let listenerState: {
    getSessionMode: (id: string) => "inactive" | "active" | "finalizing";
    batch: Record<string, { error?: string | null }>;
    live: {
      degraded: null;
      requestedLiveTranscription: boolean;
      liveTranscriptionActive: boolean;
    };
    liveSegments: unknown[];
    partialWordsByChannel: Record<number, unknown[]>;
    partialHintsByChannel: Record<number, unknown[]>;
  };
  let transcriptRowListener: (() => void) | null;
  let transcriptWordsJson: string;

  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    transcriptRowListener = null;
    transcriptWordsJson = "[]";

    listenerState = {
      getSessionMode: () => "active",
      batch: {},
      live: {
        degraded: null,
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
      },
      liveSegments: [],
      partialWordsByChannel: {},
      partialHintsByChannel: {},
    };

    useSliceRowIdsMock.mockReturnValue([transcriptId]);
    useStoreMock.mockReturnValue({
      addRowListener: vi.fn(
        (tableId: string, rowId: string, listener: () => void) => {
          if (tableId === "transcripts" && rowId === transcriptId) {
            transcriptRowListener = listener;
          }

          return "listener-1";
        },
      ),
      delListener: vi.fn(),
      getCell: vi.fn(
        (tableId: string, rowId: string, cellId: "words" | "speaker_hints") => {
          if (
            tableId === "transcripts" &&
            rowId === transcriptId &&
            cellId === "words"
          ) {
            return transcriptWordsJson;
          }

          return undefined;
        },
      ),
    });
    useListenerMock.mockImplementation((selector) => selector(listenerState));
    useAudioPlayerMock.mockReturnValue({ audioExists: false });
  });

  it("switches to transcript viewer after transcript words persist", () => {
    const scrollRef = createRef<HTMLDivElement>();
    const view = render(
      <Transcript sessionId={sessionId} scrollRef={scrollRef} />,
    );

    expect(screen.getByTestId("listening-state").textContent).toBe("listening");

    transcriptWordsJson = '[{"id":"word-1","text":" Hello"}]';
    act(() => {
      transcriptRowListener?.();
    });

    view.rerender(<Transcript sessionId={sessionId} scrollRef={scrollRef} />);

    expect(screen.queryByTestId("transcript-viewer")).not.toBeNull();
  });

  it("shows finalizing status over existing transcript content", () => {
    listenerState = {
      ...listenerState,
      getSessionMode: () => "finalizing",
    };
    transcriptWordsJson = '[{"id":"word-1","text":" Hello"}]';

    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.getByText("Finalizing transcript...")).not.toBeNull();
    expect(screen.getByTestId("transcript-viewer")).not.toBeNull();
  });

  it("shows recording state for record-only capture sessions", () => {
    listenerState = {
      ...listenerState,
      live: {
        ...listenerState.live,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      },
    };

    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.queryByTestId("listening-state")).toBeNull();
    expect(screen.getByTestId("batch-state")).not.toBeNull();
  });
});
