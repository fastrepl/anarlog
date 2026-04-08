import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { PostSessionAccessory } from "./post-session";

const {
  audioPathMock,
  useTranscriptScreenMock,
  useRunBatchMock,
  useListenerMock,
  runBatchMock,
  handleBatchFailedMock,
} = vi.hoisted(() => ({
  audioPathMock: vi.fn(),
  useTranscriptScreenMock: vi.fn(),
  useRunBatchMock: vi.fn(),
  useListenerMock: vi.fn(),
  runBatchMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioPath: audioPathMock,
  },
}));

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <div data-testid="spinner" />,
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("./expand-toggle", () => ({
  ExpandToggle: ({
    label,
  }: {
    isExpanded: boolean;
    onToggle: () => void;
    label: string;
  }) => <div>{label}</div>,
}));

vi.mock("~/audio-player", () => ({
  Timeline: () => <div data-testid="timeline" />,
  TimelineShell: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TimelineMeta: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/session/components/note-input/transcript", () => ({
  Transcript: () => <div data-testid="transcript" />,
}));

vi.mock("~/session/components/note-input/transcript/state", () => ({
  useTranscriptScreen: useTranscriptScreenMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  UI: {
    useStore: vi.fn(() => null),
    useIndexes: vi.fn(() => null),
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("~/stt/useRunBatch", () => ({
  useRunBatch: useRunBatchMock,
  isStoppedTranscriptionError: vi.fn(() => false),
}));

vi.mock("~/stt/useUploadFile", () => ({
  useUploadFile: vi.fn(() => ({
    uploadAudio: vi.fn(),
  })),
}));

describe("PostSessionAccessory", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    audioPathMock.mockResolvedValue({
      status: "ok",
      data: "/tmp/session.wav",
    });

    useTranscriptScreenMock.mockReturnValue({
      kind: "ready",
      transcriptIds: ["transcript-1"],
      liveSegments: [],
      currentActive: false,
    });

    runBatchMock.mockResolvedValue(undefined);
    useRunBatchMock.mockReturnValue(runBatchMock);

    useListenerMock.mockImplementation((selector) =>
      selector({
        handleBatchFailed: handleBatchFailedMock,
        stopTranscription: vi.fn(),
      }),
    );
  });

  it("starts regeneration without local batch state bookkeeping", async () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
        onToggleTranscript={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => {
      expect(audioPathMock).toHaveBeenCalledWith("session-1");
      expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    });

    expect(handleBatchFailedMock).not.toHaveBeenCalled();
  });
});
