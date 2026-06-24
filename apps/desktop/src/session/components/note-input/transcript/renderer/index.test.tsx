import { cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TranscriptViewer } from "./index";

const mocks = vi.hoisted(() => ({
  scrollToBottom: vi.fn(),
  scrollToTop: vi.fn(),
  scrollDetection: {
    isAtTop: true,
    isAtBottom: true,
    autoScrollEnabled: true,
    scrollTarget: null as "top" | "bottom" | null,
  },
}));

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
    ...mocks.scrollDetection,
    scrollToBottom: mocks.scrollToBottom,
    scrollToTop: mocks.scrollToTop,
  }),
}));

describe("TranscriptViewer", () => {
  beforeEach(() => {
    cleanup();
    mocks.scrollToBottom.mockReset();
    mocks.scrollToTop.mockReset();
    mocks.scrollDetection.isAtTop = true;
    mocks.scrollDetection.isAtBottom = true;
    mocks.scrollDetection.autoScrollEnabled = true;
    mocks.scrollDetection.scrollTarget = null;
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

  it("does not show a scroll chip before scroll movement starts", () => {
    mocks.scrollDetection.isAtBottom = false;

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Go to bottom" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go to top" })).toBeNull();
  });

  it("shows the bottom chip after upward scroll movement", () => {
    mocks.scrollDetection.isAtTop = false;
    mocks.scrollDetection.isAtBottom = false;
    mocks.scrollDetection.scrollTarget = "bottom";

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    const button = screen.getByRole("button", { name: "Go to bottom" });
    button.click();

    expect(screen.queryByRole("button", { name: "Go to top" })).toBeNull();
    expect(mocks.scrollToBottom).toHaveBeenCalledTimes(1);
  });

  it("shows the top chip after downward scroll movement", () => {
    mocks.scrollDetection.isAtTop = false;
    mocks.scrollDetection.scrollTarget = "top";

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    const button = screen.getByRole("button", { name: "Go to top" });
    button.click();

    expect(screen.queryByRole("button", { name: "Go to bottom" })).toBeNull();
    expect(mocks.scrollToTop).toHaveBeenCalledTimes(1);
  });
});
