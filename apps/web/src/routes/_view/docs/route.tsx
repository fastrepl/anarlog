import { createFileRoute, Outlet, useMatchRoute } from "@tanstack/react-router";
import { motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { SidebarNavigation } from "@/components/sidebar-navigation";

import { getDocsBySection } from "./-structure";

export const Route = createFileRoute("/_view/docs")({
  component: Component,
});

function Component() {
  return (
    <div
      className="min-h-[calc(100vh-4rem)] bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <LeftSidebar />
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <Outlet />
      </div>
    </div>
  );
}

function LeftSidebar() {
  const [isOpen, setIsOpen] = useState(
    () => typeof window !== "undefined" && window.innerWidth > 1400,
  );
  const closeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1400px)");
    const handler = (e: MediaQueryListEvent) => {
      if (e.matches) setIsOpen(true);
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/docs/$/", fuzzy: true });

  const currentSlug = (
    match && typeof match !== "boolean"
      ? (match._splat as string)?.replace(/\/$/, "")
      : undefined
  ) as string | undefined;

  const { sections } = getDocsBySection();
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const open = useCallback(() => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
      closeTimeoutRef.current = null;
    }
    setIsOpen(true);
  }, []);

  const close = useCallback(() => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
    }
    if (window.innerWidth > 1400) return;
    closeTimeoutRef.current = setTimeout(() => {
      setIsOpen(false);
    }, 300);
  }, []);

  useEffect(() => {
    return () => {
      if (closeTimeoutRef.current) {
        clearTimeout(closeTimeoutRef.current);
      }
    };
  }, []);

  return (
    <motion.div
      className="fixed top-1/2 left-0 z-30 hidden h-[80vh] -translate-y-1/2 drop-shadow-lg md:flex"
      initial={false}
      animate={{ x: isOpen ? 0 : -256 }}
      transition={{ type: "spring", stiffness: 400, damping: 35 }}
      onMouseEnter={open}
      onMouseLeave={close}
    >
      <div className="h-full w-64 overflow-hidden rounded-r-2xl border border-l-0 border-neutral-200 bg-white/95 backdrop-blur-sm">
        <div
          ref={scrollContainerRef}
          className="scrollbar-hide h-full overflow-y-auto px-4 py-6"
        >
          <SidebarNavigation
            sections={sections}
            currentSlug={currentSlug}
            onLinkClick={() => {
              if (window.innerWidth <= 1400) setIsOpen(false);
            }}
            scrollContainerRef={scrollContainerRef}
            linkTo="/docs/$/"
          />
        </div>
      </div>
      <motion.div
        className="-ml-px self-center"
        animate={{ opacity: isOpen ? 0 : 1 }}
        transition={{ duration: 0.2 }}
      >
        <div
          className={cn([
            "flex items-center justify-center",
            "h-20 w-7 rounded-r-xl",
            "border border-l-0 border-neutral-200 bg-white/95 backdrop-blur-sm",
            "text-neutral-400 hover:text-neutral-600",
            "cursor-pointer transition-colors",
          ])}
        >
          <motion.svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            animate={{ rotate: isOpen ? 180 : 0 }}
            transition={{ duration: 0.2 }}
          >
            <path
              d="M5 3L9.5 7L5 11"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </motion.svg>
        </div>
      </motion.div>
    </motion.div>
  );
}
