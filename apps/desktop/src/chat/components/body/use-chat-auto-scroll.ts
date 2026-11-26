import type { ChatStatus } from "ai";
import {
  type KeyboardEvent,
  type PointerEvent,
  type WheelEvent,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

const AUTO_SCROLL_BOTTOM_THRESHOLD = 24;
const PINNED_BOTTOM_THRESHOLD = 1;
const SCROLL_KEYS = new Set([
  "ArrowDown",
  "ArrowUp",
  "End",
  "Home",
  "PageDown",
  "PageUp",
  " ",
]);

function supportsOverflowAnchor() {
  return (
    typeof CSS !== "undefined" &&
    typeof CSS.supports === "function" &&
    CSS.supports("overflow-anchor", "auto")
  );
}

export function useChatAutoScroll(status: ChatStatus) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const initializedRef = useRef(false);
  const shouldAutoScrollRef = useRef(true);
  const previousIsGeneratingRef = useRef(false);
  const previousScrollTopRef = useRef(0);
  const pendingUserScrollIntentRef = useRef(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [showGoToRecent, setShowGoToRecent] = useState(false);
  const isGenerating = status === "submitted" || status === "streaming";

  const scrollToBottom = () => {
    if (!scrollRef.current) {
      return;
    }

    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    previousScrollTopRef.current = scrollRef.current.scrollTop;
    shouldAutoScrollRef.current = true;
    pendingUserScrollIntentRef.current = false;
    setIsAtBottom(true);
    setShowGoToRecent(false);
  };

  const updateAutoScrollState = () => {
    if (!scrollRef.current) {
      return;
    }

    const { scrollTop, clientHeight, scrollHeight } = scrollRef.current;
    const distanceFromBottom = scrollHeight - (scrollTop + clientHeight);
    const nextIsAtBottom = distanceFromBottom <= AUTO_SCROLL_BOTTOM_THRESHOLD;
    const isPinnedAtBottom = distanceFromBottom <= PINNED_BOTTOM_THRESHOLD;
    const scrolledUp = scrollTop < previousScrollTopRef.current;
    previousScrollTopRef.current = scrollTop;
    setIsAtBottom(nextIsAtBottom);

    if (
      (pendingUserScrollIntentRef.current || scrolledUp) &&
      !isPinnedAtBottom
    ) {
      shouldAutoScrollRef.current = false;
      return;
    }

    if (isPinnedAtBottom) {
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
    }
  };

  const markUserScrollIntent = () => {
    shouldAutoScrollRef.current = false;
    pendingUserScrollIntentRef.current = true;
    setShowGoToRecent(false);
  };

  const handleWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY < 0) {
      markUserScrollIntent();
      return;
    }

    if (event.deltaY > 0 && !isAtBottom) {
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(true);
      return;
    }

    if (event.deltaY > 0) {
      pendingUserScrollIntentRef.current = false;
    }
  };

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.target === event.currentTarget) {
      markUserScrollIntent();
    }
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (event.buttons !== 0 || event.pointerType === "touch") {
      markUserScrollIntent();
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (SCROLL_KEYS.has(event.key)) {
      markUserScrollIntent();
    }
  };

  useLayoutEffect(() => {
    const startedGenerating = isGenerating && !previousIsGeneratingRef.current;
    previousIsGeneratingRef.current = isGenerating;

    if (startedGenerating) {
      initializedRef.current = true;
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
      scrollToBottom();
      return;
    }

    const element = scrollRef.current;
    if (!initializedRef.current) {
      initializedRef.current = true;
      if (element && shouldAutoScrollRef.current) {
        scrollToBottom();
      }
      return;
    }

    if (!element || !shouldAutoScrollRef.current) {
      return;
    }

    // CSS scroll anchoring is suppressed while scrollTop === 0; pin once on overflow
    // so the browser keeps the 1px anchor pinned afterwards without rewriting scrollTop.
    if (
      !supportsOverflowAnchor() ||
      (element.scrollTop === 0 && element.scrollHeight > element.clientHeight)
    ) {
      scrollToBottom();
    }
  });

  useMountEffect(() => {
    if (!contentRef.current || typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      const element = scrollRef.current;
      if (!element || !shouldAutoScrollRef.current) {
        return;
      }

      if (
        !supportsOverflowAnchor() ||
        (element.scrollTop === 0 && element.scrollHeight > element.clientHeight)
      ) {
        scrollToBottom();
      }
    });

    observer.observe(contentRef.current);

    return () => observer.disconnect();
  });

  return {
    contentRef,
    isAtBottom,
    scrollRef,
    scrollToBottom,
    showGoToRecent,
    updateAutoScrollState,
    handleKeyDown,
    handlePointerDown,
    handlePointerMove,
    handleWheel,
  };
}
