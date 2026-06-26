import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  EMPTY_TRANSCRIPT_SEARCH,
  SegmentRenderer,
  type TranscriptSearchRenderState,
} from "./segment";

import type { Segment, SegmentWord } from "~/stt/live-segment";

const mocks = vi.hoisted(() => ({
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

describe("SegmentRenderer", () => {
  beforeEach(() => {
    mocks.wordSpan.mockClear();
  });

  it("skips playback rerenders while the active line is unchanged", () => {
    const segment = createSegment();
    const seekAndPlay = vi.fn();
    const view = render(
      <SegmentRenderer
        segment={segment}
        offsetMs={0}
        transcriptId="transcript-1"
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
        currentMs={0}
        seekAndPlay={seekAndPlay}
        audioExists
        search={createSearch("word-3")}
      />,
    );

    expect(mocks.wordSpan).toHaveBeenCalledTimes(8);
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
