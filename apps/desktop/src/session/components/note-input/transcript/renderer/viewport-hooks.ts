import {
  type DependencyList,
  type RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

export function useScrollDetection(
  containerRef: RefObject<HTMLDivElement | null>,
) {
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);
  const lastScrollTopRef = useRef(0);
  const userScrolledAwayRef = useRef(false);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) {
      return;
    }

    lastScrollTopRef.current = element.scrollTop;

    const handleScroll = () => {
      const threshold = 100;
      const distanceToBottom =
        element.scrollHeight - element.scrollTop - element.clientHeight;
      const isNearBottom = distanceToBottom < threshold;
      setIsAtBottom(isNearBottom);

      const currentTop = element.scrollTop;
      const prevTop = lastScrollTopRef.current;
      lastScrollTopRef.current = currentTop;

      const scrolledUp = currentTop < prevTop - 2;
      if (scrolledUp) {
        userScrolledAwayRef.current = true;
        setAutoScrollEnabled(false);
      }

      if (isNearBottom && !userScrolledAwayRef.current) {
        setAutoScrollEnabled(true);
      }
    };

    element.addEventListener("scroll", handleScroll);
    return () => element.removeEventListener("scroll", handleScroll);
  }, [containerRef]);

  const scrollToBottom = () => {
    const element = containerRef.current;
    if (!element) {
      return;
    }
    userScrolledAwayRef.current = false;
    setAutoScrollEnabled(true);
    element.scrollTo({ top: element.scrollHeight, behavior: "smooth" });
  };

  return { isAtBottom, autoScrollEnabled, scrollToBottom };
}

export function useAutoScroll(
  containerRef: RefObject<HTMLElement | null>,
  deps: DependencyList,
  enabled = true,
) {
  const rafRef = useRef<number | null>(null);
  const lastHeightRef = useRef(0);
  const initialFlushRef = useRef(enabled);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) {
      return;
    }

    lastHeightRef.current = element.scrollHeight;

    const isPinned = () => {
      const distanceToBottom =
        element.scrollHeight - element.scrollTop - element.clientHeight;
      return distanceToBottom < 80;
    };

    const flush = () => {
      element.scrollTop = element.scrollHeight;
    };

    const schedule = (force = false) => {
      if (!force && (!enabled || !isPinned())) {
        return;
      }

      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
      }

      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = requestAnimationFrame(flush);
      });
    };

    if (initialFlushRef.current) {
      initialFlushRef.current = false;
      schedule(true);
    } else {
      schedule();
    }

    const resizeObserver = new ResizeObserver(() => {
      const nextHeight = element.scrollHeight;
      if (nextHeight === lastHeightRef.current) {
        return;
      }
      lastHeightRef.current = nextHeight;
      schedule();
    });

    const targets = new Set<Element>([element]);
    element
      .querySelectorAll<HTMLElement>("[data-virtual-root]")
      .forEach((target) => targets.add(target));
    targets.forEach((target) => resizeObserver.observe(target));

    return () => {
      resizeObserver.disconnect();
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, deps);
}

export function usePlaybackAutoScroll(
  containerRef: RefObject<HTMLElement | null>,
  currentMs: number,
  isPlaying: boolean,
) {
  const lastScrolledWordIdRef = useRef<string | null>(null);
  const userScrolledRef = useRef(false);
  const lastScrollTimeRef = useRef(0);

  const resetUserScroll = useCallback(() => {
    userScrolledRef.current = false;
  }, []);

  useEffect(() => {
    if (!isPlaying) {
      lastScrolledWordIdRef.current = null;
      userScrolledRef.current = false;
      return;
    }

    const element = containerRef.current;
    if (!element) {
      return;
    }

    const handleUserScroll = () => {
      const now = Date.now();
      if (now - lastScrollTimeRef.current > 100) {
        userScrolledRef.current = true;
      }
    };

    element.addEventListener("wheel", handleUserScroll);
    element.addEventListener("touchmove", handleUserScroll);

    return () => {
      element.removeEventListener("wheel", handleUserScroll);
      element.removeEventListener("touchmove", handleUserScroll);
    };
  }, [containerRef, isPlaying]);

  useEffect(() => {
    if (!isPlaying || userScrolledRef.current) {
      return;
    }

    const now = Date.now();
    if (now - lastScrollTimeRef.current < 200) {
      return;
    }

    const element = containerRef.current;
    if (!element) {
      return;
    }

    const currentLineEl = element.querySelector<HTMLElement>(
      "[data-line-current='true']",
    );

    if (!currentLineEl) {
      return;
    }

    const lineKey = currentLineEl.textContent?.slice(0, 50) ?? "";
    if (lineKey === lastScrolledWordIdRef.current) {
      return;
    }

    lastScrolledWordIdRef.current = lineKey;
    lastScrollTimeRef.current = now;

    currentLineEl.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
  }, [containerRef, currentMs, isPlaying]);

  return { resetUserScroll };
}
