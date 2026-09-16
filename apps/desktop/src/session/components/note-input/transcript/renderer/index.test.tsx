import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TranscriptViewer } from "./index";
import type { TranscriptWordSelection } from "./selection";

const mocks = vi.hoisted(() => ({
  updateTranscriptSegmentText: vi.fn().mockResolvedValue(undefined),
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

vi.mock("./selection-context", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./selection-context")>();
  const { getTranscriptSectionSelection, getTranscriptSectionKeyFromElement } =
    await import("./selection");
  return {
    ...actual,
    useTranscriptSelectionSources: () => ({
      registerSource: () => () => {},
      collectEntries: () => {
        const container = document.querySelector<HTMLElement>(
          "[data-transcript-container]",
        )!;
        const entries = new Map(
          [...container.querySelectorAll<HTMLElement>("section")].map(
            (section) =>
              [
                getTranscriptSectionKeyFromElement(section)!,
                getTranscriptSectionSelection(section, container)!,
              ] as const,
          ),
        );
        return { order: [...entries.keys()], entries };
      },
    }),
  };
});

vi.mock("~/stt/queries", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/stt/queries")>()),
  updateTranscriptSegmentText: mocks.updateTranscriptSegmentText,
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
  SelectionMenu: ({
    onChangeSpeaker,
  }: {
    onChangeSpeaker?: (selection: TranscriptWordSelection) => void;
  }) =>
    onChangeSpeaker && (
      <button
        onClick={() =>
          onChangeSpeaker?.({
            text: "Transcript word",
            startMs: 0,
            groups: [
              {
                transcriptId: "1",
                segmentKey: { channel: "RemoteParty", speaker_index: 1 },
                wordIds: ["word-1"],
              },
            ],
          })
        }
      >
        Change speaker from here
      </button>
    ),
  MultiSelectionBar: ({
    entryCount,
    selection,
    onDelete,
  }: {
    entryCount: number;
    selection: TranscriptWordSelection;
    onDelete?: (selection: TranscriptWordSelection) => Promise<void>;
  }) => (
    <div
      data-testid="multi-selection-bar"
      data-selected-transcripts={selection.groups
        .map((group) => group.transcriptId)
        .join(",")}
    >
      {entryCount}
      {onDelete && (
        <button aria-label="Delete" onClick={() => void onDelete(selection)} />
      )}
    </div>
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
  preserveScrollPosition: (
    _container: unknown,
    action: () => Promise<unknown>,
  ) => action(),
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
    mocks.updateTranscriptSegmentText.mockClear();
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

  it.each([
    ["ArrowUp", 2],
    ["ArrowDown", 3],
  ])("extends block selection with Command-Shift-%s", (key, count) => {
    render(
      <TranscriptViewer
        transcriptIds={["1", "2", "3", "4"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );
    fireEvent.click(screen.getByTestId("segment-header-2"));
    const event = new KeyboardEvent("keydown", {
      key,
      code: key,
      metaKey: true,
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    fireEvent(document, event);
    expect(event.defaultPrevented).toBe(true);
    expect(
      screen
        .getByTestId("multi-selection-bar")
        .getAttribute("data-selected-transcripts"),
    ).toBe(key === "ArrowUp" ? "1,2" : "2,3,4");
    expect(screen.getByTestId("multi-selection-bar").textContent).toBe(
      String(count),
    );
    fireEvent.keyUp(document, { key, code: key });

    const opposite = key === "ArrowUp" ? "ArrowDown" : "ArrowUp";
    fireEvent.keyDown(document, {
      key: opposite,
      code: opposite,
      metaKey: true,
      shiftKey: true,
    });
    expect(screen.getByTestId("multi-selection-bar").textContent).toBe(
      String(5 - count),
    );
    fireEvent.keyUp(document, { key: opposite, code: opposite });
  });

  it("leaves shortcuts alone without a block selection or inside an editor", () => {
    render(
      <TranscriptViewer
        transcriptIds={["1", "2", "3"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );
    fireEvent.keyDown(document, {
      key: "ArrowUp",
      code: "ArrowUp",
      metaKey: true,
      shiftKey: true,
    });
    expect(screen.queryByTestId("multi-selection-bar")).toBeNull();
    fireEvent.keyUp(document, { key: "ArrowUp", code: "ArrowUp" });
    fireEvent.click(screen.getByTestId("segment-header-2"));
    const editor = screen.getByTestId("editor-2");
    editor.setAttribute("contenteditable", "true");
    fireEvent.keyDown(editor, {
      key: "ArrowUp",
      code: "ArrowUp",
      metaKey: true,
      shiftKey: true,
    });
    expect(screen.getByTestId("multi-selection-bar").textContent).toBe("1");
    fireEvent.keyUp(editor, { key: "ArrowUp", code: "ArrowUp" });
  });

  it("routes selected text to the editor speaker split at the selection start", () => {
    const onEditModeChange = vi.fn();
    render(
      <TranscriptViewer
        transcriptIds={["1"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        onEditModeChange={onEditModeChange}
        scrollRef={createRef()}
      />,
    );
    const editor = screen.getByTestId("editor-1");
    const onEnter = vi.fn((event: Event) => {
      expect((event as KeyboardEvent).key).toBe("Enter");
      expect(window.getSelection()?.isCollapsed).toBe(true);
      expect(window.getSelection()?.anchorOffset).toBe(0);
    });
    editor.addEventListener("keydown", onEnter);
    fireEvent.click(
      screen.getByRole("button", { name: "Change speaker from here" }),
    );
    expect(onEditModeChange).toHaveBeenCalledWith(true);
    expect(onEnter).toHaveBeenCalledTimes(1);
  });

  it("does not offer the speaker picker without an edit-mode callback", () => {
    render(
      <TranscriptViewer
        transcriptIds={["1"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );
    expect(
      screen.queryByRole("button", { name: "Change speaker from here" }),
    ).toBeNull();
  });

  it("saves removal of selected blocks across transcripts", async () => {
    render(
      <TranscriptViewer
        transcriptIds={["1", "2", "3"]}
        liveSegments={[]}
        currentActive={false}
        editMode
        scrollRef={createRef()}
      />,
    );
    fireEvent.click(screen.getByTestId("segment-header-1"));
    fireEvent.click(screen.getByTestId("segment-header-3"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(mocks.updateTranscriptSegmentText).toHaveBeenCalledTimes(2),
    );
    expect(mocks.updateTranscriptSegmentText).toHaveBeenCalledWith({
      transcriptId: "1",
      wordIds: ["word-1"],
      text: "",
    });
    expect(mocks.updateTranscriptSegmentText).toHaveBeenCalledWith({
      transcriptId: "3",
      wordIds: ["word-3"],
      text: "",
    });
  });

  it.each([
    [false, false],
    [true, true],
  ])(
    "does not offer deletion outside inactive edit mode (%s, %s)",
    (editMode, currentActive) => {
      render(
        <TranscriptViewer
          transcriptIds={["1"]}
          liveSegments={[]}
          currentActive={currentActive}
          editMode={editMode}
          scrollRef={createRef()}
        />,
      );
      fireEvent.click(screen.getByTestId("segment-header-1"), {
        metaKey: true,
      });
      expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
    },
  );

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
