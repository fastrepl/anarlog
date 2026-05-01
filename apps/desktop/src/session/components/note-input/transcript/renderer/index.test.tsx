import { render } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";

import { TranscriptViewer } from "./index";

const {
  renderTranscriptMock,
  useHotkeysMock,
  useAudioPlayerMock,
  useAudioTimeMock,
  useScrollDetectionMock,
  useAutoScrollMock,
  usePlaybackAutoScrollMock,
} = vi.hoisted(() => ({
  renderTranscriptMock: vi.fn(),
  useHotkeysMock: vi.fn(),
  useAudioPlayerMock: vi.fn(),
  useAudioTimeMock: vi.fn(),
  useScrollDetectionMock: vi.fn(),
  useAutoScrollMock: vi.fn(),
  usePlaybackAutoScrollMock: vi.fn(),
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: useHotkeysMock,
}));

vi.mock("./transcript", () => ({
  RenderTranscript: (props: unknown) => {
    renderTranscriptMock(props);
    return <div data-testid="render-transcript" />;
  },
}));

vi.mock("./selection-menu", () => ({
  SelectionMenu: () => null,
}));

vi.mock("./separator", () => ({
  TranscriptSeparator: () => <div data-testid="separator" />,
}));

vi.mock("./viewport-hooks", () => ({
  useScrollDetection: useScrollDetectionMock,
  useAutoScroll: useAutoScrollMock,
  usePlaybackAutoScroll: usePlaybackAutoScrollMock,
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: useAudioPlayerMock,
}));

vi.mock("~/audio-player/provider", () => ({
  useAudioTime: useAudioTimeMock,
}));

describe("TranscriptViewer", () => {
  it("renders live segments without a persisted transcript id", () => {
    useScrollDetectionMock.mockReturnValue({
      isAtBottom: true,
      autoScrollEnabled: true,
      scrollToBottom: vi.fn(),
    });
    useAudioPlayerMock.mockReturnValue({
      state: "stopped",
      pause: vi.fn(),
      resume: vi.fn(),
      start: vi.fn(),
      seek: vi.fn(),
      audioExists: false,
    });
    useAudioTimeMock.mockReturnValue({ current: 0 });

    const liveSegments = [
      {
        id: "segment-1",
        key: {
          channel: "DirectMic",
          speaker_index: null,
          speaker_human_id: null,
        },
        words: [
          {
            id: "word-1",
            text: "Hello",
            start_ms: 0,
            end_ms: 500,
            channel: 0,
            is_final: true,
          },
        ],
      },
    ];

    const { container } = render(
      <TranscriptViewer
        transcriptIds={[]}
        liveSegments={liveSegments as never[]}
        currentActive
        scrollRef={createRef()}
        enablePlaybackControls={false}
      />,
    );

    expect(renderTranscriptMock).toHaveBeenCalled();
    expect(
      renderTranscriptMock.mock.calls.some(
        ([props]) =>
          props &&
          typeof props === "object" &&
          "transcriptId" in props &&
          "liveSegments" in props &&
          (props as { transcriptId?: string }).transcriptId === undefined &&
          (props as { liveSegments: unknown[] }).liveSegments === liveSegments,
      ),
    ).toBe(true);
    expect(useHotkeysMock).toHaveBeenCalledWith(
      "space",
      expect.any(Function),
      expect.objectContaining({ enabled: false }),
    );
    const transcriptContainer = container.querySelector(
      "[data-transcript-container]",
    );
    expect(transcriptContainer?.classList.contains("pb-0")).toBe(true);
    expect(transcriptContainer?.classList.contains("pb-16")).toBe(false);
  });
});
