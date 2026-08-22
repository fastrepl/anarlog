import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TranscriptViewer } from "./index";

const mocks = vi.hoisted(() => ({
  scrollToBottom: vi.fn(),
  scrollToTop: vi.fn(),
  scrollDetection: {
    isAtTop: true,
    isAtBottom: true,
    isNearBottom: true,
    canScroll: false,
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
  MultiSelectionBar: ({ entryCount }: { entryCount: number }) => (
    <div data-testid="multi-selection-bar">{entryCount}</div>
  ),
}));

vi.mock("./transcript", () => ({
  RenderTranscript: ({
    liveSegments,
    shouldScrollToEnd,
    transcriptId,
    currentActive,
    captureGeneration,
    editMode,
  }: {
    liveSegments: unknown[];
    shouldScrollToEnd: boolean;
    transcriptId: string;
    currentActive: boolean;
    captureGeneration?: number;
    editMode?: boolean;
  }) => (
    <div
      data-testid="render-transcript"
      data-capture-generation={String(captureGeneration ?? 0)}
      data-current-active={String(currentActive)}
      data-live-segment-count={String(liveSegments.length)}
      data-should-scroll-to-end={String(shouldScrollToEnd)}
      data-transcript-id={transcriptId}
    >
      <section
        data-testid={`segment-${transcriptId}`}
        data-transcript-id={transcriptId}
        data-session-id="session-1"
        data-transcript-segment-id={`segment-${transcriptId}`}
        data-segment-channel="RemoteParty"
        data-segment-speaker-index="1"
        data-transcript-offset-ms="0"
      >
        <div data-testid={`segment-header-${transcriptId}`}>Speaker</div>
        {editMode ? (
          <div
            data-transcript-segment-content
            data-transcript-editor
            data-transcript-edit-word-ids={JSON.stringify([
              `word-${transcriptId}`,
            ])}
            data-transcript-edit-word-start-ms={JSON.stringify([0])}
            data-transcript-edit-word-texts={JSON.stringify([
              "Transcript word",
            ])}
            data-testid={`editor-${transcriptId}`}
          >
            Transcript word
          </div>
        ) : (
          <div data-transcript-segment-content>
            <span
              data-transcript-word-id={`word-${transcriptId}`}
              data-transcript-word-start-ms="0"
            >
              Transcript word
            </span>
          </div>
        )}
      </section>
    </div>
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
    mocks.scrollDetection.isNearBottom = true;
    mocks.scrollDetection.canScroll = false;
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

  it("clips horizontal overflow so speaker labels stay aligned with text", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive={false}
        scrollRef={createRef()}
      />,
    );

    const container = document.querySelector("[data-transcript-container]");
    expect(container?.className).toContain("overflow-x-clip");
    expect(container?.className).toContain("min-w-0");
  });

  it("keeps active transcript sessions pinned to the bottom", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        captureGeneration={7}
        scrollRef={createRef()}
      />,
    );

    expect(
      screen
        .getByTestId("render-transcript")
        .getAttribute("data-should-scroll-to-end"),
    ).toBe("true");
    expect(
      screen
        .getByTestId("render-transcript")
        .getAttribute("data-capture-generation"),
    ).toBe("7");
  });

  it("keeps active transcript sessions pinned near the exact bottom edge", () => {
    mocks.scrollDetection.isAtBottom = false;
    mocks.scrollDetection.isNearBottom = true;

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

  it("renders live segments before a transcript row exists", () => {
    render(
      <TranscriptViewer
        transcriptIds={[]}
        liveSegments={[
          {
            end_ms: 1000,
            id: "segment-1",
            key: { channel: "DirectMic" },
            start_ms: 0,
            text: "hello",
            words: [],
          },
        ]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    const transcript = screen.getByTestId("render-transcript");
    expect(transcript.getAttribute("data-live-segment-count")).toBe("1");
    expect(transcript.getAttribute("data-transcript-id")).toBe(
      "__live-transcript__",
    );
  });

  it("keeps prior transcript rows settled during an active capture", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1", "transcript-2"]}
        liveSegments={[
          {
            end_ms: 1000,
            id: "segment-1",
            key: { channel: "DirectMic" },
            start_ms: 0,
            text: "hello",
            words: [],
          },
        ]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    const transcripts = screen.getAllByTestId("render-transcript");
    expect(transcripts[0]?.getAttribute("data-current-active")).toBe("false");
    expect(transcripts[0]?.getAttribute("data-live-segment-count")).toBe("0");
    expect(transcripts[1]?.getAttribute("data-current-active")).toBe("true");
    expect(transcripts[1]?.getAttribute("data-live-segment-count")).toBe("1");
  });

  it("supports scattered entry selection with command-click", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1", "transcript-2"]}
        liveSegments={[]}
        currentActive={false}
        scrollRef={createRef()}
      />,
    );

    fireEvent.click(screen.getByTestId("segment-transcript-1"), {
      metaKey: true,
    });
    fireEvent.click(screen.getByTestId("segment-transcript-2"), {
      metaKey: true,
    });

    expect(screen.getByTestId("multi-selection-bar").textContent).toBe("2");
  });

  it("lets entries be chosen without modifier keys while editing", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1", "transcript-2"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );

    fireEvent.click(screen.getByTestId("segment-header-transcript-1"));
    fireEvent.click(screen.getByTestId("segment-header-transcript-2"));

    expect(screen.getByTestId("multi-selection-bar").textContent).toBe("2");
  });

  it("does not treat clicks in the text editor as segment selection", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );

    fireEvent.click(screen.getByTestId("editor-transcript-1"));

    expect(screen.queryByTestId("multi-selection-bar")).toBeNull();
  });

  it("does not show selection actions until entries are chosen", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );

    expect(screen.queryByTestId("multi-selection-bar")).toBeNull();
    expect(screen.queryByRole("button", { name: "Select All" })).toBeNull();
  });

  it("does not show scroll controls when the transcript cannot scroll", () => {
    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Scroll to top" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Scroll to bottom" }),
    ).toBeNull();
  });

  it("renders right-side scroll controls when the transcript can scroll", () => {
    mocks.scrollDetection.isAtTop = false;
    mocks.scrollDetection.isAtBottom = false;
    mocks.scrollDetection.canScroll = true;

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    const controls = document.querySelector(
      "[data-transcript-scroll-controls]",
    );
    const topButton = screen.getByRole("button", { name: "Scroll to top" });
    const bottomButton = screen.getByRole("button", {
      name: "Scroll to bottom",
    });

    topButton.click();
    bottomButton.click();

    expect(controls?.className).toContain("right-1");
    expect(controls?.className).toContain("top-1/2");
    expect(controls?.className).toContain("bg-transparent");
    expect(controls?.className).toContain("border-transparent");
    expect(controls?.className).toContain("hover:bg-background/65");
    expect(controls?.className).toContain("hover:backdrop-blur-md");
    expect(controls?.className).toContain("focus-within:backdrop-blur-md");
    expect((topButton as HTMLButtonElement).disabled).toBe(false);
    expect((bottomButton as HTMLButtonElement).disabled).toBe(false);
    expect(topButton.firstElementChild?.tagName.toLowerCase()).toBe("svg");
    expect(bottomButton.firstElementChild?.tagName.toLowerCase()).toBe("svg");
    expect(mocks.scrollToTop).toHaveBeenCalledTimes(1);
    expect(mocks.scrollToBottom).toHaveBeenCalledTimes(1);
  });

  it("keeps scroll controls visible inside both edge thresholds", () => {
    mocks.scrollDetection.canScroll = true;

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive
        scrollRef={createRef()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Scroll to top" }),
    ).not.toBeNull();
    expect(
      screen.getByRole("button", { name: "Scroll to bottom" }),
    ).not.toBeNull();
  });

  it("disables the top control at the top", () => {
    mocks.scrollDetection.isAtBottom = false;
    mocks.scrollDetection.canScroll = true;

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive={false}
        scrollRef={createRef()}
      />,
    );

    const topButton = screen.getByRole("button", { name: "Scroll to top" });
    const bottomButton = screen.getByRole("button", {
      name: "Scroll to bottom",
    });

    bottomButton.click();

    expect((topButton as HTMLButtonElement).disabled).toBe(true);
    expect((bottomButton as HTMLButtonElement).disabled).toBe(false);
    expect(mocks.scrollToTop).not.toHaveBeenCalled();
    expect(mocks.scrollToBottom).toHaveBeenCalledTimes(1);
  });

  it("disables the bottom control at the bottom", () => {
    mocks.scrollDetection.isAtTop = false;
    mocks.scrollDetection.canScroll = true;

    render(
      <TranscriptViewer
        transcriptIds={["transcript-1"]}
        liveSegments={[]}
        currentActive={false}
        scrollRef={createRef()}
      />,
    );

    const topButton = screen.getByRole("button", { name: "Scroll to top" });
    const bottomButton = screen.getByRole("button", {
      name: "Scroll to bottom",
    });

    topButton.click();

    expect((topButton as HTMLButtonElement).disabled).toBe(false);
    expect((bottomButton as HTMLButtonElement).disabled).toBe(true);
    expect(mocks.scrollToTop).toHaveBeenCalledTimes(1);
    expect(mocks.scrollToBottom).not.toHaveBeenCalled();
  });
});
