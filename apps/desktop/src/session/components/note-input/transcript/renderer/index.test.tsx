import { cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TranscriptViewer } from "./index";

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: vi.fn(),
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({
    state: "stopped",
    pause: vi.fn(),
    resume: vi.fn(),
    start: vi.fn(),
    seek: vi.fn(),
    audioExists: true,
  }),
}));

vi.mock("~/audio-player/provider", () => ({
  useAudioTime: () => ({ current: 0 }),
}));

vi.mock("./selection-menu", () => ({
  SelectionMenu: () => null,
}));

vi.mock("./transcript", () => ({
  RenderTranscript: ({ shouldScrollToEnd }: { shouldScrollToEnd: boolean }) => (
    <div
      data-testid="render-transcript"
      data-should-scroll-to-end={String(shouldScrollToEnd)}
    />
  ),
}));

vi.mock("./viewport-hooks", () => ({
  useAutoScroll: vi.fn(),
  usePlaybackAutoScroll: vi.fn(),
  useScrollDetection: () => ({
    isAtBottom: true,
    autoScrollEnabled: true,
    scrollToBottom: vi.fn(),
  }),
}));

describe("TranscriptViewer", () => {
  beforeEach(() => {
    cleanup();
  });

  it("does not pin inactive transcript sessions to the bottom on open", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive={false}
        scrollRef={createRef()}
      />,
    );

    expect(
      screen
        .getByTestId("render-transcript")
        .getAttribute("data-should-scroll-to-end"),
    ).toBe("false");
  });

  it("keeps active transcript sessions pinned to the bottom", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    expect(
      screen
        .getByTestId("render-transcript")
        .getAttribute("data-should-scroll-to-end"),
    ).toBe("true");
  });
});
