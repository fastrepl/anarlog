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

    const newState =
      direction === "vertical"
        ? {
            atStart: el.scrollTop <= 1,
            atEnd: el.scrollTop + el.clientHeight >= el.scrollHeight - 1,
          }
        : {
            atStart: el.scrollLeft <= 1,
            atEnd: el.scrollLeft + el.clientWidth >= el.scrollWidth - 1,
          };

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

    const resizeObserver = new ResizeObserver(update);
    resizeObserver.observe(el);

    return () => {
      el.removeEventListener("scroll", throttledUpdate);
      resizeObserver.disconnect();
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
