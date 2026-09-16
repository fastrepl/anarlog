import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  EMPTY_TRANSCRIPT_SEARCH,
  SegmentRenderer,
  type TranscriptSearchRenderState,
} from "./segment";
import { TranscriptSelectionProvider } from "./selection-context";

import type { Segment, SegmentWord } from "~/stt/live-segment";

const mocks = vi.hoisted(() => ({
  splitTranscriptSpeaker: vi.fn(() => Promise.resolve()),
  updateTranscriptSegmentText: vi.fn(() => Promise.resolve()),
  wordSpan: vi.fn(
    ({
      displayText,
      isActiveMatch,
    }: {
      displayText: string;
      isActiveMatch?: boolean;
    }) => (
      <span data-active-match={isActiveMatch ? "true" : undefined}>
        {displayText}
      </span>
    ),
  ),
}));

vi.mock("./segment-header", () => ({
  SegmentHeader: () => null,
}));

vi.mock("./speaker-assign", () => ({
  SpeakerParticipantPicker: ({
    onSelect,
  }: {
    onSelect: (id: string) => Promise<void>;
  }) => (
    <button onClick={() => void onSelect("human-2")}>Choose speaker</button>
  ),
}));

vi.mock("./word-span", () => ({
  WordSpan: mocks.wordSpan,
}));

vi.mock("~/stt/queries", () => ({
  updateTranscriptSegmentText: mocks.updateTranscriptSegmentText,
  splitTranscriptSpeaker: mocks.splitTranscriptSpeaker,
}));

describe("SegmentRenderer", () => {
  beforeEach(() => {
    mocks.splitTranscriptSpeaker.mockClear();
    mocks.wordSpan.mockClear();
    mocks.updateTranscriptSegmentText.mockClear();
  });

  it("keeps spaces between rendered words and lines", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(view.container.textContent).toBe("First line. Second line.");
  });

  it("skips playback rerenders while the active line is unchanged", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={500}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);

    view.rerender(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={700}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);
  });

  it("rerenders playback when the active line changes", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={500}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);

    view.rerender(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={1500}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(8);
  });

  it("skips active-match navigation outside the segment", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const search = createSearch("outside-1");
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={search}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);

    view.rerender(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={createSearch("outside-2")}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);
  });

  it("rerenders active-match navigation inside the segment", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={createSearch("outside")}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);

    view.rerender(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={createSearch("word-3")}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(8);
  });

  it("rerenders when the computed speaker label changes", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(4);

    view.rerender(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Alice"
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(8);
  });

  it("draws selected borders inside the segment so list overflow cannot clip them", () => {
    const view = render(
      <TranscriptSelectionProvider
        selectMode
        selectedKeys={new Set(["transcript-1:segment-1"])}
        registerSource={() => () => {}}
      >
        <SegmentRenderer
          segment={createSegment()}
          offsetMs={0}
          transcriptId="transcript-1"
          speakerLabel="Speaker 1"
          currentMs={0}
          seekAndPlay={vi.fn()}
          audioExists
          search={EMPTY_TRANSCRIPT_SEARCH}
        />
      </TranscriptSelectionProvider>,
    );

    const section = view.container.querySelector(
      "[data-transcript-selected='true']",
    );
    expect(section?.className).toContain("ring-inset");
    expect(section?.className).toContain("ring-1");
    expect(section?.className).toContain("ring-primary/30");
  });

  it("persists text corrections when leaving write mode content", async () => {
    const segment = createSegment();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={vi.fn()}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
        editMode
      />,
    );

    const editor = view.container.querySelector<HTMLElement>(
      "[data-transcript-editor]",
    );
    expect(editor?.getAttribute("contenteditable")).toBe("true");
    editor!.innerText = "Corrected transcript text";
    fireEvent.blur(editor!);

    await waitFor(() => {
      expect(mocks.updateTranscriptSegmentText).toHaveBeenCalledWith({
        transcriptId: "transcript-1",
        wordIds: ["word-1", "word-2", "word-3", "word-4"],
        text: "Corrected transcript text",
      });
    });
  });
  it("opens speaker selection on Enter and assigns the text after the cursor", async () => {
    const view = render(
      <SegmentRenderer
        segment={createSegment()}
        offsetMs={0}
        transcriptId="transcript-1"
        sessionId="session-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={vi.fn()}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
        editMode
      />,
    );
    const editor = view.container.querySelector<HTMLElement>(
      "[data-transcript-editor]",
    )!;
    const range = document.createRange();
    range.setStart(editor.firstChild!, 12);
    range.collapse(true);
    window.getSelection()!.removeAllRanges();
    window.getSelection()!.addRange(range);
    fireEvent.keyDown(editor, { key: "Enter", isComposing: true });
    expect(screen.queryByRole("button", { name: "Choose speaker" })).toBeNull();
    fireEvent.keyDown(editor, { key: "Enter", shiftKey: true });
    expect(screen.queryByRole("button", { name: "Choose speaker" })).toBeNull();
    fireEvent.keyDown(editor, { key: "Enter" });
    const chooseSpeaker = await screen.findByRole("button", {
      name: "Choose speaker",
    });
    expect(
      view.container.querySelector("[data-transcript-speaker-split]")
        ?.textContent,
    ).toBe("Second line.");
    expect(editor.hidden).toBe(true);
    expect(view.container.contains(chooseSpeaker)).toBe(false);
    expect(
      chooseSpeaker.closest("[data-radix-popper-content-wrapper]"),
    ).not.toBeNull();
    fireEvent.click(chooseSpeaker);
    await waitFor(() =>
      expect(mocks.splitTranscriptSpeaker).toHaveBeenCalledWith({
        transcriptId: "transcript-1",
        segmentKey: createSegment().key,
        wordIds: ["word-1", "word-2", "word-3", "word-4"],
        text: "First line. Second line.",
        offset: 12,
        humanId: "human-2",
      }),
    );
  });
  it("cancels the split preview without saving or losing edited text", async () => {
    const view = render(
      <SegmentRenderer
        segment={createSegment()}
        offsetMs={0}
        transcriptId="transcript-1"
        speakerLabel="Speaker 1"
        currentMs={0}
        seekAndPlay={vi.fn()}
        audioExists
        search={EMPTY_TRANSCRIPT_SEARCH}
        editMode
      />,
    );
    const editor = view.container.querySelector<HTMLElement>(
      "[data-transcript-editor]",
    )!;
    editor.textContent = "Edited sentence. Next speaker.";
    const range = document.createRange();
    range.setStart(editor.firstChild!, 17);
    range.collapse(true);
    window.getSelection()!.removeAllRanges();
    window.getSelection()!.addRange(range);
    fireEvent.keyDown(editor, { key: "Enter" });
    const picker = await screen.findByRole("button", {
      name: "Choose speaker",
    });
    expect(
      view.container.querySelector("[data-transcript-speaker-split]")
        ?.textContent,
    ).toBe("Next speaker.");
    fireEvent.keyDown(picker, { key: "Escape" });
    await waitFor(() =>
      expect(
        view.container.querySelector("[data-transcript-split-preview]"),
      ).toBeNull(),
    );
    expect(editor.hidden).toBe(false);
    expect(editor.textContent).toBe("Edited sentence. Next speaker.");
    expect(mocks.splitTranscriptSpeaker).not.toHaveBeenCalled();
  });
});

function createSearch(activeMatchId: string): TranscriptSearchRenderState {
  return {
    query: "line",
    activeMatchId,
    caseSensitive: false,
    wholeWord: false,
  };
}

function createSegment(): Segment {
  return {
    id: "segment-1",
    text: "First line. Second line.",
    start_ms: 100,
    end_ms: 1800,
    key: {
      channel: "MixedCapture",
      speaker_index: null,
      speaker_human_id: null,
    },
    words: [
      createWord("word-1", "First", 100, 300),
      createWord("word-2", "line.", 300, 900),
      createWord("word-3", "Second", 1200, 1400),
      createWord("word-4", "line.", 1400, 1800),
    ],
  };
}

function createWord(
  id: string,
  text: string,
  startMs: number,
  endMs: number,
): SegmentWord {
  return {
    id,
    text,
    start_ms: startMs,
    end_ms: endMs,
    channel: "MixedCapture",
    is_final: true,
  };
}
