import type { ChatStatus } from "ai";
import { type WheelEvent, useLayoutEffect, useRef, useState } from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

const AUTO_SCROLL_BOTTOM_THRESHOLD = 24;
const PINNED_BOTTOM_THRESHOLD = 1;

export function useChatAutoScroll(status: ChatStatus) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const shouldAutoScrollRef = useRef(true);
  const previousIsGeneratingRef = useRef(false);
  const pendingUserScrollIntentRef = useRef(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [showGoToRecent, setShowGoToRecent] = useState(false);
  const isGenerating = status === "submitted" || status === "streaming";

  const scrollToBottom = () => {
    if (!scrollRef.current) {
      return;
    }

    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
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
    setIsAtBottom(nextIsAtBottom);

    if (pendingUserScrollIntentRef.current && !isPinnedAtBottom) {
      shouldAutoScrollRef.current = false;
      return;
    }

    if (nextIsAtBottom) {
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
      return;
    }

    shouldAutoScrollRef.current = false;
  };

  const handleWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY < 0) {
      shouldAutoScrollRef.current = false;
      pendingUserScrollIntentRef.current = true;
      setShowGoToRecent(false);
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

  useLayoutEffect(() => {
    if (isGenerating && !previousIsGeneratingRef.current) {
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
    }

    previousIsGeneratingRef.current = isGenerating;

    if (shouldAutoScrollRef.current) {
      scrollToBottom();
    }
  });

  useMountEffect(() => {
    if (!contentRef.current || typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (shouldAutoScrollRef.current) {
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
    handleWheel,
  };
}
