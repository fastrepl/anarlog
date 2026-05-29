import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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

vi.mock("~/audio-player", () => ({
  Timeline: () => <div data-testid="timeline" />,
  TimelineShell: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TimelineMeta: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  useAudioPlayer: () => ({
    audioExists: true,
    deleteRecording: vi.fn(),
    isDeletingRecording: false,
  }),
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

describe("PostSessionAccessory", () => {
  beforeEach(() => {
    cleanup();
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
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => {
      expect(audioPathMock).toHaveBeenCalledWith("session-1");
      expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    });

    expect(handleBatchFailedMock).not.toHaveBeenCalled();
  });

  it("shows Regenerate button without upload or reserved height in empty panel", async () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "ready",
      transcriptIds: [],
      liveSegments: [],
      currentActive: false,
    });

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript={false}
        isTranscriptExpanded
        fillHeight
      />,
    );

    const noTranscript = screen.getByText("No transcript yet");
    const transcriptCard = noTranscript.parentElement?.parentElement;
    const transcriptSlot = transcriptCard?.parentElement;
    expect(transcriptCard?.className).not.toContain("min-h-[114px]");
    expect(transcriptCard?.className).not.toContain("min-h-[96px]");
    expect(transcriptSlot?.className).not.toContain("min-h-[114px]");
    expect(transcriptSlot?.className).toContain("shrink-0");
    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Regenerate/ }));

    await waitFor(() => {
      expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    });
  });

  it("keeps the audio timeline visible while the transcript panel is collapsed", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded={false}
      />,
    );

    expect(screen.getByTestId("timeline")).toBeTruthy();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });

  it("keeps the audio timeline slot height stable between collapsed and expanded states", () => {
    const { unmount } = render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded={false}
      />,
    );

    const collapsedSlotClassName =
      screen.getByTestId("timeline").parentElement?.className;
    expect(collapsedSlotClassName).toContain("h-10");

    unmount();

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
        fillHeight
      />,
    );

    expect(screen.getByTestId("timeline").parentElement?.className).toBe(
      collapsedSlotClassName,
    );
  });

  it("lets expanded transcript content fill the resizable bottom panel", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
        fillHeight
      />,
    );

    const scrollArea = screen.getByTestId("transcript").parentElement;
    expect(scrollArea?.className).toContain("flex-1");
    expect(scrollArea?.className).not.toContain("h-[300px]");

    const transcriptCard = scrollArea?.parentElement;
    const transcriptSlot = transcriptCard?.parentElement;
    expect(transcriptCard?.className).toContain("rounded-b-xl");
    expect(transcriptCard?.className).toContain("border");
    expect(transcriptCard?.className).toContain("h-full");
    expect(transcriptCard?.className).toContain("min-h-[114px]");
    expect(transcriptCard?.className).not.toContain("min-h-[96px]");
    expect(transcriptSlot?.className).toContain("flex-1");
    expect(transcriptSlot?.className).toContain("min-h-[114px]");
  });

  it("shows transcript skeletons instead of duplicating batch progress in the body", () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "running_batch",
      percentage: 0.25,
      phase: "transcribing",
    });

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
      />,
    );

    expect(screen.getByText("Transcript")).toBeTruthy();
    expect(screen.getAllByText("Transcribing...")).toHaveLength(1);
    expect(screen.getAllByTestId("spinner")).toHaveLength(1);
    expect(screen.getByTestId("transcript-skeleton")).toBeTruthy();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });
});
