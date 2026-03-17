import { type RefObject, useCallback, useEffect, useRef } from "react";

export function useScrollPreservation(
  key: string,
  options: { skipRestoration?: boolean } = {},
): {
  scrollRef: RefObject<HTMLDivElement | null>;
  onBeforeTabChange: () => void;
} {
  const scrollPositions = useRef(new Map<string, number>());
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const currentKeyRef = useRef(key);

  const onBeforeTabChange = useCallback(() => {
    if (scrollRef.current) {
      scrollPositions.current.set(
        currentKeyRef.current,
        scrollRef.current.scrollTop,
      );
    }
  }, []);

  useEffect(() => {
    currentKeyRef.current = key;

    const container = scrollRef.current;
    if (!container) return;

    if (options.skipRestoration) return;

    const savedPosition = scrollPositions.current.get(key);
    if (savedPosition === undefined) return;

    const rafId = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        container.scrollTop = savedPosition;
      });
    });

    return () => cancelAnimationFrame(rafId);
  }, [key, options.skipRestoration]);

  return { scrollRef, onBeforeTabChange };
}
