import { cleanup, render, screen } from "@testing-library/react";
import { createRef, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Transcript } from "./index";

const { useIncompleteCaptureMock } = vi.hoisted(() => ({
  useIncompleteCaptureMock: vi.fn(),
}));
vi.mock("~/stt/capture-result", () => ({
  useIncompleteCapture: useIncompleteCaptureMock,
}));

const {
  useListenerMock,
  useAudioPlayerMock,
  useSessionTranscriptMetadataMock,
  regenerateTranscriptMock,
} = vi.hoisted(() => ({
  useListenerMock: vi.fn(),
  useAudioPlayerMock: vi.fn(),
  useSessionTranscriptMetadataMock: vi.fn(),
  regenerateTranscriptMock: vi.fn(),
}));

vi.mock("./actions", () => ({
  useRegenerateTranscript: () => regenerateTranscriptMock,
}));

vi.mock("~/stt/queries", () => ({
  useSessionTranscriptMetadata: useSessionTranscriptMetadataMock,
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
  TranscriptViewer: ({
    captureGeneration,
    editMode,
    footer,
  }: {
    captureGeneration: number;
    editMode?: boolean;
    footer?: ReactNode;
  }) => (
    <div
      data-testid="transcript-viewer"
      data-capture-generation={captureGeneration}
      data-edit-mode={String(editMode ?? false)}
    >
      <div data-testid="latest-transcript-line" />
      {footer}
    </div>
  ),
}));

vi.mock("./screens/interrupted", () => ({
  LiveTranscriptInterruptedNotice: () => (
    <p data-testid="live-transcript-paused" />
  ),
}));

vi.mock("~/stt/useUploadFile", () => ({
  useUploadFile: vi.fn(() => ({
    uploadAudio: vi.fn(),
    uploadTranscript: vi.fn(),
    processFile: vi.fn(),
  })),
}));

describe("Transcript", () => {
  const sessionId = "session-1";
  const transcriptId = "transcript-1";

  let listenerState: {
    getSessionMode: (id: string) => "inactive" | "active" | "finalizing";
    batch: Record<string, { error?: string | null }>;
    live: {
      captureGenerationCounter: number;
      captureGenerationBySession: Record<string, number>;
      status: "active" | "inactive";
      sessionId: string | null;
      degraded: { type: "connection_timeout" } | null;
      transcriptionStalled: boolean;
      requestedLiveTranscription: boolean;
      liveTranscriptionActive: boolean;
    };
    liveSegments: unknown[];
    partialWordsByChannel: Record<number, unknown[]>;
    partialHintsByChannel: Record<number, unknown[]>;
  };
  let transcripts: Array<{ id: string; hasWords: boolean }>;

  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    useIncompleteCaptureMock.mockReturnValue(null);
    transcripts = [{ id: transcriptId, hasWords: false }];

    listenerState = {
      getSessionMode: () => "active",
      batch: {},
      live: {
        captureGenerationCounter: 2,
        captureGenerationBySession: {
          [sessionId]: 1,
          "session-2": 2,
        },
        status: "active",
        sessionId,
        degraded: null,
        transcriptionStalled: false,
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
      },
      liveSegments: [],
      partialWordsByChannel: {},
      partialHintsByChannel: {},
    };

    useSessionTranscriptMetadataMock.mockImplementation(() => transcripts);
    useListenerMock.mockImplementation((selector) => selector(listenerState));
    useAudioPlayerMock.mockReturnValue({ audioExists: false });
  });

  it("switches to transcript viewer after transcript words persist", () => {
    const scrollRef = createRef<HTMLDivElement>();
    const view = render(
      <Transcript sessionId={sessionId} scrollRef={scrollRef} />,
    );

    expect(screen.getByTestId("listening-state").textContent).toBe("listening");

    transcripts = [{ id: transcriptId, hasWords: true }];

    view.rerender(<Transcript sessionId={sessionId} scrollRef={scrollRef} />);

    expect(
      screen
        .getByTestId("transcript-viewer")
        .getAttribute("data-capture-generation"),
    ).toBe("1");
  });

  it("marks the live transcript gap while live transcription is interrupted", () => {
    transcripts = [{ id: transcriptId, hasWords: true }];
    listenerState = {
      ...listenerState,
      live: { ...listenerState.live, degraded: { type: "connection_timeout" } },
    };

    const view = render(
      <Transcript sessionId={sessionId} scrollRef={createRef()} />,
    );

    const notice = screen.getByTestId("live-transcript-paused");
    expect(notice.parentElement).toBe(screen.getByTestId("transcript-viewer"));
    expect(notice.previousElementSibling).toBe(
      screen.getByTestId("latest-transcript-line"),
    );

    listenerState = {
      ...listenerState,
      live: { ...listenerState.live, degraded: null },
    };
    view.rerender(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.queryByTestId("live-transcript-paused")).toBeNull();
  });

  it("shows the interruption before any live transcript arrives", () => {
    listenerState = {
      ...listenerState,
      live: { ...listenerState.live, transcriptionStalled: true },
    };

    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.queryByTestId("listening-state")).toBeNull();
    expect(screen.getByTestId("batch-state")).not.toBeNull();
  });

  it("keeps existing transcript content unobstructed while finalizing", () => {
    listenerState = {
      ...listenerState,
      getSessionMode: () => "finalizing",
    };
    transcripts = [{ id: transcriptId, hasWords: true }];

    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.queryByText("Finalizing transcript...")).toBeNull();
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

  it("keeps the transcript visible when the live provider disconnects", () => {
    listenerState = {
      ...listenerState,
      live: { ...listenerState.live, liveTranscriptionActive: false },
    };
    transcripts = [{ id: transcriptId, hasWords: true }];

    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.getByTestId("transcript-viewer")).not.toBeNull();
    expect(screen.queryByTestId("batch-state")).toBeNull();
  });

  it("keeps batch status for record-only capture with an earlier transcript", () => {
    listenerState = {
      ...listenerState,
      live: {
        ...listenerState.live,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      },
    };
    transcripts = [{ id: transcriptId, hasWords: true }];

    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);

    expect(screen.getByTestId("batch-state")).not.toBeNull();
  });

  it("renders finalized transcripts in the requested edit mode", () => {
    listenerState = {
      ...listenerState,
      getSessionMode: () => "inactive",
    };
    transcripts = [{ id: transcriptId, hasWords: true }];

    const view = render(
      <Transcript
        sessionId={sessionId}
        scrollRef={createRef()}
        editMode={false}
      />,
    );

    expect(
      screen.getByTestId("transcript-viewer").getAttribute("data-edit-mode"),
    ).toBe("false");
    expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();

    view.rerender(
      <Transcript sessionId={sessionId} scrollRef={createRef()} editMode />,
    );

    expect(
      screen.getByTestId("transcript-viewer").getAttribute("data-edit-mode"),
    ).toBe("true");
  });
  it("keeps the zero-retention incomplete message visible after recording", () => {
    listenerState.getSessionMode = () => "inactive";
    transcripts = [{ id: transcriptId, hasWords: true }];
    useIncompleteCaptureMock.mockReturnValue({ audioDeleted: true });
    render(<Transcript sessionId={sessionId} scrollRef={createRef()} />);
    expect(screen.getByTestId("transcript-viewer")).not.toBeNull();
    expect(screen.getByRole("status").textContent).toContain(
      "Audio was deleted according to your retention setting",
    );
  });
});
