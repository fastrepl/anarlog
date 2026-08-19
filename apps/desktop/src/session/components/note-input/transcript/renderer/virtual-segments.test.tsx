import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  estimateTranscriptRowHeight,
  VirtualSegmentRow,
} from "./virtual-segments";

import type { Segment } from "~/stt/live-segment";

afterEach(() => {
  cleanup();
});

describe("estimateTranscriptRowHeight", () => {
  it("keeps short speaker turns compact instead of reserving a large empty row", () => {
    expect(
      estimateTranscriptRowHeight(createSegment("Hi there."), 0),
    ).toBeLessThan(80);
    expect(estimateTranscriptRowHeight(createSegment(""), 1)).toBeLessThan(60);
  });

  it("grows with wrapped content instead of a flat minimum", () => {
    const short = estimateTranscriptRowHeight(createSegment("Hi"), 0);
    const long = estimateTranscriptRowHeight(
      createSegment("word ".repeat(400).trim()),
      0,
    );
    expect(long).toBeGreaterThan(short + 80);
  });
});

describe("VirtualSegmentRow", () => {
  it("positions rows with top offsets instead of transforms", () => {
    const onMeasure = vi.fn();
    const view = render(
      <VirtualSegmentRow
        rowKey="segment-1"
        index={3}
        top={240}
        onMeasure={onMeasure}
        onFocus={vi.fn()}
        onBlur={vi.fn()}
      >
        <div>Speaker 1</div>
      </VirtualSegmentRow>,
    );

    const row = view.container.querySelector(
      "[data-transcript-virtual-index='3']",
    );
    expect(row).toBeInstanceOf(HTMLElement);
    if (!(row instanceof HTMLElement)) {
      throw new Error("expected virtual row");
    }
    expect(row.style.position).toBe("absolute");
    expect(row.style.top).toBe("240px");
    expect(row.style.width).toBe("100%");
    expect(row.style.transform).toBe("");
  });
});

function createSegment(text: string): Segment {
  const words = text
    ? text.split(/\s+/).map((word, index) => ({
        id: `word-${index}`,
        text: word,
        start_ms: index * 100,
        end_ms: index * 100 + 80,
        channel: "MixedCapture" as const,
        is_final: true,
      }))
    : [];

  return {
    id: "segment-1",
    key: {
      channel: "MixedCapture",
      speaker_index: 0,
      speaker_human_id: null,
    },
    start_ms: 0,
    end_ms: Math.max(0, words.length * 100),
    text,
    words,
  };
}
