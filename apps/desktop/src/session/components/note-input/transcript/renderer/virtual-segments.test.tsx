import { act, cleanup, render, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  estimateTranscriptRowHeight,
  useVirtualSegments,
  VirtualSegmentRow,
} from "./virtual-segments";

import type { Segment } from "~/stt/live-segment";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
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
  it.each([false, true])(
    "measures once at mount and uses observer sizes during resize (editMode=%s)",
    (editMode) => {
      let notify: ResizeObserverCallback = () => {};
      const observe = vi.fn();
      const disconnect = vi.fn();
      vi.stubGlobal(
        "ResizeObserver",
        class {
          constructor(callback: ResizeObserverCallback) {
            notify = callback;
          }
          observe = observe;
          disconnect = disconnect;
        },
      );
      const onMeasure = vi.fn();
      const props = {
        rowKey: "segment-1",
        index: 0,
        onMeasure,
        onFocus: vi.fn(),
        onBlur: vi.fn(),
      };
      const content = (
        <div contentEditable={editMode} suppressContentEditableWarning>
          Original text
        </div>
      );
      const readHeight = vi.spyOn(HTMLElement.prototype, "offsetHeight", "get");
      const view = render(
        <VirtualSegmentRow {...props} top={0}>
          {content}
        </VirtualSegmentRow>,
      );
      const row = view.container.firstElementChild as HTMLDivElement;
      const editor = row.firstElementChild as HTMLDivElement;
      if (editMode) {
        editor.focus();
        editor.textContent = "Unsaved text";
      }
      expect(readHeight).toHaveBeenCalledOnce();
      readHeight.mockClear();
      onMeasure.mockClear();
      act(() => {
        notify(
          [
            {
              target: row,
              borderBoxSize: [{ blockSize: 123.5 }],
            } as unknown as ResizeObserverEntry,
          ],
          {} as ResizeObserver,
        );
      });
      expect(onMeasure).toHaveBeenCalledExactlyOnceWith("segment-1", 123.5);
      expect(readHeight).not.toHaveBeenCalled();
      expect(observe).toHaveBeenCalledWith(row, { box: "border-box" });
      view.rerender(
        <VirtualSegmentRow {...props} top={80}>
          {content}
        </VirtualSegmentRow>,
      );
      expect(row.firstElementChild).toBe(editor);
      expect(editor.textContent).toBe(
        editMode ? "Unsaved text" : "Original text",
      );
      if (editMode) expect(document.activeElement).toBe(editor);
      view.unmount();
      expect(disconnect).toHaveBeenCalledOnce();
    },
  );

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

describe("useVirtualSegments measurements", () => {
  it.each(["removed", "unchanged"])(
    "prunes stale heights when pending measurements are %s",
    (pending) => {
      const frames: FrameRequestCallback[] = [];
      vi.stubGlobal(
        "requestAnimationFrame",
        (callback: FrameRequestCallback) => {
          frames.push(callback);
          return frames.length;
        },
      );
      vi.stubGlobal("cancelAnimationFrame", vi.fn());
      const segments = [createSegment("First"), createSegment("Second")];
      const { result, rerender } = renderHook(
        ({ keys }) =>
          useVirtualSegments({
            segments,
            segmentKeys: keys,
            scrollElement: null,
            activeMatchId: null,
            searchEnabled: false,
            currentMs: 0,
            offsetMs: 0,
          }),
        { initialProps: { keys: ["first", "second"] } },
      );
      const estimatedFirstHeight = result.current.virtualItems[1].top;
      act(() => {
        result.current.measureRow("first", 500);
        result.current.measureRow("second", 120);
      });
      act(() => frames.shift()!(0));
      rerender({ keys: ["replacement", "second"] });
      act(() =>
        result.current.measureRow(
          pending === "removed" ? "first" : "second",
          pending === "removed" ? 500 : 120,
        ),
      );
      act(() => frames.shift()!(0));
      rerender({ keys: ["first", "second"] });
      expect(result.current.totalHeight).toBe(estimatedFirstHeight + 120);
    },
  );

  it("batches a resize burst into one frame and uses the latest row sizes", () => {
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal(
      "requestAnimationFrame",
      vi.fn((callback: FrameRequestCallback) => {
        frames.push(callback);
        return frames.length;
      }),
    );
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const segments = [createSegment("First"), createSegment("Second")];
    const segmentKeys = ["first", "second"];
    const { result, unmount } = renderHook(() =>
      useVirtualSegments({
        segments,
        segmentKeys,
        scrollElement: null,
        activeMatchId: null,
        searchEnabled: false,
        currentMs: 0,
        offsetMs: 0,
      }),
    );
    const initialHeight = result.current.totalHeight;
    act(() => {
      result.current.measureRow("first", 100);
      result.current.measureRow("second", 120);
      result.current.measureRow("first", 140);
      result.current.measureRow("removed", 900);
      result.current.measureRow("second", Number.NaN);
    });
    expect(frames).toHaveLength(1);
    expect(result.current.totalHeight).toBe(initialHeight);
    act(() => frames[0](0));
    expect(result.current.totalHeight).toBe(260);
    expect(result.current.virtualItems[1].top).toBe(140);
    const items = result.current.virtualItems;
    act(() => result.current.measureRow("first", 140));
    act(() => frames[1](0));
    expect(result.current.virtualItems).toBe(items);
    act(() => result.current.measureRow("first", 160));
    unmount();
    expect(cancelAnimationFrame).toHaveBeenCalledWith(3);
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
