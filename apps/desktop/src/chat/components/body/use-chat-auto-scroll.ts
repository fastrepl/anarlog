import type { ChatStatus } from "ai";
import { type WheelEvent, useEffect, useRef, useState } from "react";

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
    const nextIsAtBottom = distanceFromBottom <= 24;
    setIsAtBottom(nextIsAtBottom);

    if (nextIsAtBottom) {
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
      return;
    }

    shouldAutoScrollRef.current = false;
  };

  const handleWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY > 0 && !isAtBottom) {
      setShowGoToRecent(true);
      return;
    }

    if (event.deltaY < 0) {
      setShowGoToRecent(false);
    }

    if (!isGenerating || event.deltaY >= 0) {
      return;
    }

    pendingUserScrollIntentRef.current = true;
  };

  useEffect(() => {
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

  useEffect(() => {
    if (!contentRef.current) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (shouldAutoScrollRef.current) {
        scrollToBottom();
      }
    });

    observer.observe(contentRef.current);

    return () => observer.disconnect();
  }, []);

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
