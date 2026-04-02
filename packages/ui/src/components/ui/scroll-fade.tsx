import {
  memo,
  type RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { cn } from "@hypr/utils";

type ScrollDirection = "horizontal" | "vertical";

const VERTICAL_FADE_SIZE = 24;
const HORIZONTAL_FADE_SIZE = 32;

function getFadePercent(fadeSize: number, visibleSize: number) {
  if (visibleSize <= 0) {
    return 0;
  }

  return Math.min((fadeSize / visibleSize) * 100, 50);
}

function applyScrollFadeMask(
  el: HTMLElement,
  direction: ScrollDirection,
  atStart: boolean,
  atEnd: boolean,
) {
  if (direction === "vertical") {
    const fadePercent = getFadePercent(VERTICAL_FADE_SIZE, el.clientHeight);

    el.style.setProperty("--scroll-fade-top-start", "0%");
    el.style.setProperty(
      "--scroll-fade-top-end",
      atStart ? "0%" : `${fadePercent}%`,
    );
    el.style.setProperty(
      "--scroll-fade-bottom-start",
      atEnd ? "100%" : `${100 - fadePercent}%`,
    );
    el.style.setProperty("--scroll-fade-bottom-end", "100%");
    el.style.removeProperty("--scroll-fade-left-start");
    el.style.removeProperty("--scroll-fade-left-end");
    el.style.removeProperty("--scroll-fade-right-start");
    el.style.removeProperty("--scroll-fade-right-end");
    return;
  }

  const fadePercent = getFadePercent(HORIZONTAL_FADE_SIZE, el.clientWidth);

  el.style.setProperty("--scroll-fade-left-start", "0%");
  el.style.setProperty(
    "--scroll-fade-left-end",
    atStart ? "0%" : `${fadePercent}%`,
  );
  el.style.setProperty(
    "--scroll-fade-right-start",
    atEnd ? "100%" : `${100 - fadePercent}%`,
  );
  el.style.setProperty("--scroll-fade-right-end", "100%");
  el.style.removeProperty("--scroll-fade-top-start");
  el.style.removeProperty("--scroll-fade-top-end");
  el.style.removeProperty("--scroll-fade-bottom-start");
  el.style.removeProperty("--scroll-fade-bottom-end");
}

export function getScrollFadeMaskClassName(direction: ScrollDirection) {
  return direction === "vertical" ? "scroll-fade-mask-y" : "scroll-fade-mask-x";
}

export function useScrollFade<T extends HTMLElement>(
  ref: RefObject<T | null>,
  direction: ScrollDirection = "vertical",
  deps: unknown[] = [],
) {
  const [state, setState] = useState({ atStart: true, atEnd: true });
  const rafRef = useRef<number | null>(null);

  const update = useCallback(() => {
    const el = ref.current;
    if (!el) return;

    const hasOverflow =
      direction === "vertical"
        ? el.scrollHeight > el.clientHeight + 1
        : el.scrollWidth > el.clientWidth + 1;
    const newState =
      direction === "vertical"
        ? {
            atStart: !hasOverflow || el.scrollTop <= 1,
            atEnd:
              !hasOverflow ||
              el.scrollTop + el.clientHeight >= el.scrollHeight - 1,
          }
        : {
            atStart: !hasOverflow || el.scrollLeft <= 1,
            atEnd:
              !hasOverflow ||
              el.scrollLeft + el.clientWidth >= el.scrollWidth - 1,
          };

    applyScrollFadeMask(el, direction, newState.atStart, newState.atEnd);

    setState((prev) => {
      if (prev.atStart === newState.atStart && prev.atEnd === newState.atEnd) {
        return prev;
      }
      return newState;
    });
  }, [ref, direction]);

  const throttledUpdate = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
    }
    rafRef.current = requestAnimationFrame(() => {
      update();
      rafRef.current = null;
    });
  }, [update]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    update();
    el.addEventListener("scroll", throttledUpdate, { passive: true });

    const resizeObserver = new ResizeObserver(throttledUpdate);
    resizeObserver.observe(el);
    const mutationObserver = new MutationObserver(throttledUpdate);
    mutationObserver.observe(el, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    const animationFrame = requestAnimationFrame(update);
    const timerId = window.setTimeout(update, 200);

    return () => {
      el.removeEventListener("scroll", throttledUpdate);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      cancelAnimationFrame(animationFrame);
      window.clearTimeout(timerId);
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
      }
    };
  }, [ref, update, throttledUpdate, ...deps]);

  return state;
}

export const ScrollFadeOverlay = memo(function ScrollFadeOverlay({
  position,
}: {
  position: "top" | "bottom" | "left" | "right";
}) {
  const isHorizontal = position === "left" || position === "right";

  return (
    <div
      className={cn([
        "pointer-events-none absolute z-20",
        isHorizontal ? ["top-0 h-full w-8"] : ["left-0 h-6 w-full"],
        position === "top" && "top-0 bg-linear-to-b from-white to-transparent",
        position === "bottom" &&
          "bottom-0 bg-linear-to-t from-white to-transparent",
        position === "left" &&
          "left-0 bg-linear-to-r from-white to-transparent",
        position === "right" &&
          "right-0 bg-linear-to-l from-white to-transparent",
      ])}
    />
  );
});
