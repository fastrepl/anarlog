import { render, screen } from "@testing-library/react";
import { createRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { Transcript } from "./index";

const {
  useSliceRowIdsMock,
  useStoreMock,
  useTableMock,
  useListenerMock,
  useAudioPlayerMock,
} = vi.hoisted(() => ({
  useSliceRowIdsMock: vi.fn(),
  useStoreMock: vi.fn(),
  useTableMock: vi.fn(),
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
    useTable: useTableMock,
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

describe("Transcript", () => {
  const sessionId = "session-1";
  const transcriptId = "transcript-1";

  let listenerState: {
    getSessionMode: (id: string) => "inactive" | "active" | "finalizing";
    batch: Record<string, { error?: string | null }>;
    live: {
      degraded: null;
      requestedTranscriptionMode: "live";
      currentTranscriptionMode: "live";
      recordingMode: "disk";
    };
    partialWordsByChannel: Record<number, unknown[]>;
    partialHintsByChannel: Record<number, unknown[]>;
  };
  let transcriptWordsJson: string;
  let transcriptsTable: Record<string, { words: string }>;

  beforeEach(() => {
    transcriptWordsJson = "[]";
    transcriptsTable = {
      [transcriptId]: {
        words: transcriptWordsJson,
      },
    };

    listenerState = {
      getSessionMode: () => "active",
      batch: {},
      live: {
        degraded: null,
        requestedTranscriptionMode: "live",
        currentTranscriptionMode: "live",
        recordingMode: "disk",
      },
      partialWordsByChannel: {},
      partialHintsByChannel: {},
    };

    useSliceRowIdsMock.mockReturnValue([transcriptId]);
    useStoreMock.mockReturnValue({
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
    useTableMock.mockImplementation(() => transcriptsTable);
    useListenerMock.mockImplementation((selector) => selector(listenerState));
    useAudioPlayerMock.mockReturnValue({ audioExists: false });
  });

  it("switches to transcript viewer after transcript words persist", () => {
    const scrollRef = createRef<HTMLDivElement>();
    const view = render(
      <Transcript
        sessionId={sessionId}
        isEditing={false}
        scrollRef={scrollRef}
      />,
    );

    expect(screen.getByTestId("listening-state").textContent).toBe("listening");

    transcriptWordsJson = '[{"id":"word-1","text":" Hello"}]';
    transcriptsTable = {
      [transcriptId]: {
        words: transcriptWordsJson,
      },
    };

    view.rerender(
      <Transcript
        sessionId={sessionId}
        isEditing={false}
        scrollRef={scrollRef}
      />,
    );

    expect(screen.queryByTestId("transcript-viewer")).not.toBeNull();
  });
});
