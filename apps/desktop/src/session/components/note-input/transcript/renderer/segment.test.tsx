import { fireEvent, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  EMPTY_TRANSCRIPT_SEARCH,
  SegmentRenderer,
  type TranscriptSearchRenderState,
} from "./segment";

import type { Segment, SegmentWord } from "~/stt/live-segment";

const mocks = vi.hoisted(() => ({
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

vi.mock("./word-span", () => ({
  WordSpan: mocks.wordSpan,
}));

vi.mock("~/stt/queries", () => ({
  updateTranscriptSegmentText: mocks.updateTranscriptSegmentText,
}));

describe("SegmentRenderer", () => {
  beforeEach(() => {
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
