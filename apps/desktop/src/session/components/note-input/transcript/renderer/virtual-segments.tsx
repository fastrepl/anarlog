import {
  type CSSProperties,
  type FocusEvent,
  type ReactNode,
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import {
  createTranscriptSearchIndex,
  getTranscriptSearchIndexMatches,
  registerTranscriptSearchSource,
  type TranscriptSearchSource,
} from "../../search/matching";

import type { Segment } from "~/stt/live-segment";

const ESTIMATED_LINE_HEIGHT = 24;
const ESTIMATED_ROW_CHARS = 90;
const ESTIMATED_ROW_CHROME = 64;
const MINIMUM_ROW_HEIGHT = 88;
const SEGMENT_GAP = 16;
const OVERSCAN_PX = 800;
const FALLBACK_VIEWPORT_HEIGHT = 800;

export function useVirtualSegments({
  segments,
  segmentKeys,
  scrollElement,
  activeMatchId,
  currentMs,
  offsetMs,
}: {
  segments: Segment[];
  segmentKeys: string[];
  scrollElement: HTMLDivElement | null;
  activeMatchId: string | null;
  currentMs: number;
  offsetMs: number;
}) {
  const [listElement, setListElement] = useState<HTMLDivElement | null>(null);
  const [focusedIndex, setFocusedIndex] = useState<number | null>(null);
  const [measuredHeights, setMeasuredHeights] = useState<
    ReadonlyMap<string, number>
  >(() => new Map());
  const unregisterSearchRef = useRef<(() => void) | null>(null);
  const searchSourceRef = useRef<TranscriptSearchSource>(() => []);
  const scrollToIndexRef = useRef<
    (index: number, behavior: ScrollBehavior) => void
  >(() => {});
  const latestKeysRef = useRef(new Set(segmentKeys));
  latestKeysRef.current = new Set(segmentKeys);

  const estimatedHeights = useMemo(
    () =>
      segments.map((segment, index) => {
        const characterCount = segment.words.reduce(
          (total, word) => total + word.text.length + 1,
          0,
        );
        const contentHeight =
          ESTIMATED_ROW_CHROME +
          Math.ceil(characterCount / ESTIMATED_ROW_CHARS) *
            ESTIMATED_LINE_HEIGHT;
        return (
          Math.max(MINIMUM_ROW_HEIGHT, contentHeight) +
          (index > 0 ? SEGMENT_GAP : 0)
        );
      }),
    [segments],
  );
  const offsets = useMemo(() => {
    const values = [0];
    for (const [index, key] of segmentKeys.entries()) {
      values.push(
        values[index] +
          (measuredHeights.get(key) ??
            estimatedHeights[index] ??
            MINIMUM_ROW_HEIGHT),
      );
    }
    return values;
  }, [estimatedHeights, measuredHeights, segmentKeys]);
  const totalHeight = offsets[offsets.length - 1] ?? 0;
  const viewport = useVirtualViewport(scrollElement, listElement);
  const segmentIndex = useMemo(() => createSegmentIndex(segments), [segments]);

  const activeMatchIndex = activeMatchId
    ? (segmentIndex.wordIndexes.get(activeMatchId) ?? null)
    : null;
  const playbackIndex = useMemo(
    () => findPlaybackSegmentIndex(segmentIndex, currentMs, offsetMs),
    [currentMs, offsetMs, segmentIndex],
  );
  const selectedIndexes = useSelectedSegmentIndexes(listElement);
  const virtualItems = useMemo(() => {
    const visibleTop = viewport.scrollTop - viewport.listTop;
    const visibleBottom = visibleTop + viewport.height;
    const indexes = new Set<number>();

    if (visibleBottom > 0 && visibleTop < totalHeight) {
      const start = findRowIndex(
        offsets,
        Math.max(0, visibleTop - OVERSCAN_PX),
      );
      const end = findRowIndex(
        offsets,
        Math.min(totalHeight, visibleBottom + OVERSCAN_PX),
      );
      for (
        let index = start;
        index <= end && index < segments.length;
        index++
      ) {
        indexes.add(index);
      }
    } else if (!scrollElement && segments.length > 0) {
      const end = findRowIndex(offsets, FALLBACK_VIEWPORT_HEIGHT + OVERSCAN_PX);
      for (let index = 0; index <= end && index < segments.length; index++) {
        indexes.add(index);
      }
    }

    if (activeMatchIndex !== null) indexes.add(activeMatchIndex);
    if (playbackIndex !== null) indexes.add(playbackIndex);
    if (focusedIndex !== null) indexes.add(focusedIndex);
    for (const index of selectedIndexes) indexes.add(index);

    return Array.from(indexes)
      .sort((left, right) => left - right)
      .map((index) => ({
        index,
        key: segmentKeys[index]!,
        top: offsets[index]!,
      }));
  }, [
    activeMatchIndex,
    focusedIndex,
    offsets,
    playbackIndex,
    scrollElement,
    selectedIndexes,
    segmentKeys,
    segments.length,
    totalHeight,
    viewport.height,
    viewport.listTop,
    viewport.scrollTop,
  ]);

  const measureRow = useCallback((key: string, height: number) => {
    if (!Number.isFinite(height) || height <= 0) return;
    setMeasuredHeights((current) => {
      const previous = current.get(key);
      if (previous !== undefined && Math.abs(previous - height) < 1) {
        return current;
      }
      const next = new Map<string, number>();
      for (const [existingKey, existingHeight] of current) {
        if (latestKeysRef.current.has(existingKey)) {
          next.set(existingKey, existingHeight);
        }
      }
      next.set(key, height);
      return next;
    });
  }, []);

  const scrollToIndex = useCallback(
    (index: number, behavior: ScrollBehavior = "auto") => {
      if (
        !scrollElement ||
        !listElement ||
        index < 0 ||
        index >= segments.length
      ) {
        return;
      }
      const scrollRect = scrollElement.getBoundingClientRect();
      const listRect = listElement.getBoundingClientRect();
      const listTop = scrollElement.scrollTop + listRect.top - scrollRect.top;
      const rowHeight = offsets[index + 1]! - offsets[index]!;
      const top =
        listTop +
        offsets[index]! -
        Math.max(0, (scrollElement.clientHeight - rowHeight) / 2);
      scrollElement.scrollTo({ top: Math.max(0, top), behavior });
    },
    [listElement, offsets, scrollElement, segments.length],
  );
  scrollToIndexRef.current = scrollToIndex;

  const previousActiveMatchRef = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (
      activeMatchId &&
      activeMatchId !== previousActiveMatchRef.current &&
      activeMatchIndex !== null &&
      listElement &&
      scrollElement
    ) {
      scrollToIndex(activeMatchIndex, "smooth");
      previousActiveMatchRef.current = activeMatchId;
    } else if (!activeMatchId) {
      previousActiveMatchRef.current = null;
    }
  }, [
    activeMatchId,
    activeMatchIndex,
    listElement,
    scrollElement,
    scrollToIndex,
  ]);

  const searchIndex = useMemo(
    () =>
      createTranscriptSearchIndex(
        segments.flatMap((segment, index) =>
          segment.words.map((word) => ({
            id: word.id ?? null,
            text: word.text.trim(),
            scrollIntoView: () => scrollToIndexRef.current(index, "smooth"),
          })),
        ),
      ),
    [segments],
  );
  searchSourceRef.current = (preparedQuery, options) =>
    getTranscriptSearchIndexMatches(searchIndex, preparedQuery, options);

  const listRef = useCallback(
    (node: HTMLDivElement | null) => {
      unregisterSearchRef.current?.();
      unregisterSearchRef.current = null;
      setListElement(node);
      if (node && scrollElement) {
        unregisterSearchRef.current = registerTranscriptSearchSource(
          scrollElement,
          (preparedQuery, options) =>
            searchSourceRef.current(preparedQuery, options),
        );
      }
    },
    [scrollElement],
  );

  const handleRowFocus = useCallback((index: number) => {
    setFocusedIndex(index);
  }, []);
  const handleRowBlur = useCallback((event: FocusEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget)) {
      setFocusedIndex(null);
    }
  }, []);

  return {
    handleRowBlur,
    handleRowFocus,
    listRef,
    measureRow,
    totalHeight,
    virtualItems,
  };
}

function useSelectedSegmentIndexes(listElement: HTMLDivElement | null) {
  const subscribe = useCallback((notify: () => void) => {
    document.addEventListener("selectionchange", notify);
    return () => document.removeEventListener("selectionchange", notify);
  }, []);
  const getSnapshot = useCallback(() => {
    if (!listElement) return "";
    const selection = window.getSelection();
    const indexes = [selection?.anchorNode, selection?.focusNode]
      .flatMap((node) => {
        const element =
          node instanceof Element ? node : (node?.parentElement ?? null);
        const row = element?.closest<HTMLElement>(
          "[data-transcript-virtual-index]",
        );
        if (!row || !listElement.contains(row)) return [];
        const index = Number(row.dataset.transcriptVirtualIndex);
        return Number.isInteger(index) ? [index] : [];
      })
      .sort((left, right) => left - right);
    return Array.from(new Set(indexes)).join(",");
  }, [listElement]);
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return useMemo(
    () =>
      snapshot ? snapshot.split(",").map(Number).filter(Number.isInteger) : [],
    [snapshot],
  );
}

export function VirtualSegmentRow({
  rowKey,
  top,
  index,
  onMeasure,
  onFocus,
  onBlur,
  children,
}: {
  rowKey: string;
  top: number;
  index: number;
  onMeasure: (key: string, height: number) => void;
  onFocus: (index: number) => void;
  onBlur: (event: FocusEvent<HTMLDivElement>) => void;
  children: ReactNode;
}) {
  const observerRef = useRef<ResizeObserver | null>(null);
  const rowRef = useCallback(
    (node: HTMLDivElement | null) => {
      observerRef.current?.disconnect();
      observerRef.current = null;
      if (!node) return;

      const measure = () =>
        onMeasure(rowKey, node.getBoundingClientRect().height);
      measure();
      if (typeof ResizeObserver !== "undefined") {
        observerRef.current = new ResizeObserver(measure);
        observerRef.current.observe(node);
      }
    },
    [onMeasure, rowKey],
  );
  const style = useMemo<CSSProperties>(
    () => ({
      left: 0,
      position: "absolute",
      right: 0,
      top: 0,
      transform: `translateY(${top}px)`,
    }),
    [top],
  );

  return (
    <div
      ref={rowRef}
      data-transcript-virtual-index={index}
      style={style}
      onFocusCapture={() => onFocus(index)}
      onBlurCapture={onBlur}
    >
      {children}
    </div>
  );
}

function useVirtualViewport(
  scrollElement: HTMLDivElement | null,
  listElement: HTMLDivElement | null,
) {
  const store = useMemo(
    () => createVirtualViewportStore(scrollElement, listElement),
    [listElement, scrollElement],
  );
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
  const [scrollTop, height, listTop] = snapshot.split("|").map(Number);
  return { scrollTop, height, listTop };
}

function createVirtualViewportStore(
  scrollElement: HTMLDivElement | null,
  listElement: HTMLDivElement | null,
) {
  let listTop: number | null = null;
  let snapshot = "0|800|0";
  let animationFrame: number | null = null;
  let shouldMeasureList = true;
  const listeners = new Set<() => void>();

  const refresh = (measureList: boolean) => {
    if (!scrollElement) {
      snapshot = `0|${FALLBACK_VIEWPORT_HEIGHT}|0`;
      return;
    }

    if (measureList || listTop === null) {
      const scrollRect = scrollElement.getBoundingClientRect();
      const listRect = listElement?.getBoundingClientRect();
      listTop = listRect
        ? scrollElement.scrollTop + listRect.top - scrollRect.top
        : scrollElement.scrollTop;
    }
    const next = `${scrollElement.scrollTop}|${scrollElement.clientHeight || FALLBACK_VIEWPORT_HEIGHT}|${listTop}`;
    if (next !== snapshot) {
      snapshot = next;
      listeners.forEach((listener) => listener());
    }
  };

  const scheduleRefresh = (measureList = false) => {
    shouldMeasureList ||= measureList;
    if (animationFrame !== null) return;
    animationFrame = requestAnimationFrame(() => {
      animationFrame = null;
      const measure = shouldMeasureList;
      shouldMeasureList = false;
      refresh(measure);
    });
  };

  refresh(true);
  const subscribe = (listener: () => void) => {
    if (!scrollElement) return () => {};
    listeners.add(listener);
    const handleScroll = () => scheduleRefresh();
    const handleLayout = () => scheduleRefresh(true);
    scrollElement.addEventListener("scroll", handleScroll, { passive: true });
    window.addEventListener("resize", handleLayout);
    const resizeObserver =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(handleLayout);
    resizeObserver?.observe(scrollElement);
    if (listElement) resizeObserver?.observe(listElement);
    const mutationObserver =
      typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver(handleLayout);
    mutationObserver?.observe(scrollElement, {
      childList: true,
      subtree: true,
    });
    scheduleRefresh(true);

    return () => {
      listeners.delete(listener);
      scrollElement.removeEventListener("scroll", handleScroll);
      window.removeEventListener("resize", handleLayout);
      resizeObserver?.disconnect();
      mutationObserver?.disconnect();
      if (animationFrame !== null) {
        cancelAnimationFrame(animationFrame);
        animationFrame = null;
      }
    };
  };

  return { getSnapshot: () => snapshot, subscribe };
}

function findRowIndex(offsets: number[], target: number): number {
  if (offsets.length <= 1) return 0;
  let low = 0;
  let high = offsets.length - 2;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (offsets[middle + 1]! <= target) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }
  return low;
}

function createSegmentIndex(segments: Segment[]) {
  const wordIndexes = new Map<string, number>();
  const playbackStarts: number[] = [];
  const playbackEnds: number[] = [];
  const playbackSegmentIndexes: number[] = [];
  const playbackPrefixMaxEnds: number[] = [];

  segments.forEach((segment, segmentIndex) => {
    for (const word of segment.words) {
      if (word.id && !wordIndexes.has(word.id)) {
        wordIndexes.set(word.id, segmentIndex);
      }
    }
    const first = segment.words[0];
    const last = segment.words[segment.words.length - 1];
    if (!first || !last) return;
    const start = first.start_ms ?? 0;
    const end = last.end_ms ?? 0;
    playbackStarts.push(start);
    playbackEnds.push(end);
    playbackSegmentIndexes.push(segmentIndex);
    playbackPrefixMaxEnds.push(
      Math.max(
        playbackPrefixMaxEnds[playbackPrefixMaxEnds.length - 1] ??
          Number.NEGATIVE_INFINITY,
        end,
      ),
    );
  });

  return {
    wordIndexes,
    playbackStarts,
    playbackEnds,
    playbackSegmentIndexes,
    playbackPrefixMaxEnds,
  };
}

function findPlaybackSegmentIndex(
  index: ReturnType<typeof createSegmentIndex>,
  currentMs: number,
  offsetMs: number,
): number | null {
  if (currentMs <= 0) return null;
  const target = currentMs - offsetMs;
  let low = 0;
  let high = index.playbackStarts.length;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (index.playbackStarts[middle]! <= target) low = middle + 1;
    else high = middle;
  }
  const lastStarted = low - 1;
  if (lastStarted < 0) return null;

  low = 0;
  high = lastStarted;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (index.playbackPrefixMaxEnds[middle]! < target) low = middle + 1;
    else high = middle;
  }
  return index.playbackEnds[low]! >= target
    ? index.playbackSegmentIndexes[low]!
    : null;
}
