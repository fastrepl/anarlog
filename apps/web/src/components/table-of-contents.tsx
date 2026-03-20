import { motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

const ITEM_HEIGHT = 40;

export function TableOfContents({
  toc,
}: {
  toc: Array<{ id: string; text: string; level: number }>;
}) {
  const [activeId, setActiveId] = useState<string | null>(
    toc.length > 0 ? toc[0].id : null,
  );
  const observerRef = useRef<IntersectionObserver | null>(null);
  const headingElementsRef = useRef<Record<string, IntersectionObserverEntry>>(
    {},
  );
  const isUserScrollingToc = useRef(false);
  const userScrollTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wheelAccumulator = useRef(0);

  const scrollToHeading = useCallback((id: string) => {
    isUserScrollingToc.current = true;
    if (userScrollTimeout.current) {
      clearTimeout(userScrollTimeout.current);
    }
    userScrollTimeout.current = setTimeout(() => {
      isUserScrollingToc.current = false;
    }, 1000);

    setActiveId(id);
    document.getElementById(id)?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  }, []);

  const getActiveHeading = useCallback(() => {
    const visibleHeadings: IntersectionObserverEntry[] = [];
    for (const entry of Object.values(headingElementsRef.current)) {
      if (entry.isIntersecting) {
        visibleHeadings.push(entry);
      }
    }

    if (visibleHeadings.length > 0) {
      const sorted = visibleHeadings.sort(
        (a, b) =>
          (a.target as HTMLElement).getBoundingClientRect().top -
          (b.target as HTMLElement).getBoundingClientRect().top,
      );
      return sorted[0].target.id;
    }
    return null;
  }, []);

  useEffect(() => {
    if (toc.length === 0) return;

    observerRef.current = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          headingElementsRef.current[entry.target.id] = entry;
        }
        if (!isUserScrollingToc.current) {
          const active = getActiveHeading();
          if (active) {
            setActiveId(active);
          }
        }
      },
      { rootMargin: "-80px 0px -60% 0px", threshold: 0 },
    );

    const headingIds = toc.map((item) => item.id);
    for (const id of headingIds) {
      const el = document.getElementById(id);
      if (el) {
        observerRef.current.observe(el);
      }
    }

    return () => {
      observerRef.current?.disconnect();
    };
  }, [toc, getActiveHeading]);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.stopPropagation();

      const THRESHOLD = 50;
      wheelAccumulator.current += e.deltaY;

      if (Math.abs(wheelAccumulator.current) < THRESHOLD) return;

      const direction = wheelAccumulator.current > 0 ? 1 : -1;
      wheelAccumulator.current = 0;

      const currentIndex = toc.findIndex((item) => item.id === activeId);
      const nextIndex = Math.max(
        0,
        Math.min(toc.length - 1, currentIndex + direction),
      );

      if (nextIndex !== currentIndex) {
        scrollToHeading(toc[nextIndex].id);
      }
    },
    [toc, activeId, scrollToHeading],
  );

  if (toc.length === 0) {
    return null;
  }

  const activeIndex = toc.findIndex((item) => item.id === activeId);

  return (
    <aside
      className={cn([
        "fixed top-0 right-0 z-10 hidden h-screen xl:flex",
        "w-64 items-center",
      ])}
    >
      <nav
        className="relative w-full cursor-ns-resize overflow-hidden"
        style={{ height: ITEM_HEIGHT * 5 }}
        onWheel={handleWheel}
      >
        <motion.div
          className="flex flex-col"
          animate={{ y: -activeIndex * ITEM_HEIGHT + ITEM_HEIGHT * 2 }}
          transition={{ type: "spring", stiffness: 300, damping: 30 }}
        >
          {toc.map((item, index) => {
            const distance = Math.abs(index - activeIndex);
            const isActive = index === activeIndex;

            return (
              <a
                key={item.id}
                href={`#${item.id}`}
                onClick={(e) => {
                  e.preventDefault();
                  scrollToHeading(item.id);
                }}
                className={cn([
                  "flex shrink-0 items-center pr-4 pl-6 transition-colors duration-200",
                  isActive
                    ? "font-medium text-stone-800"
                    : "text-neutral-400 hover:text-neutral-600",
                  item.level === 3 && "pl-9",
                  item.level === 4 && "pl-12",
                ])}
                style={{
                  height: ITEM_HEIGHT,
                  opacity: isActive
                    ? 1
                    : distance === 1
                      ? 0.45
                      : distance === 2
                        ? 0.2
                        : 0.08,
                  fontSize: isActive ? 14 : 13,
                }}
              >
                <span className="line-clamp-1">{item.text}</span>
              </a>
            );
          })}
        </motion.div>
      </nav>
    </aside>
  );
}
