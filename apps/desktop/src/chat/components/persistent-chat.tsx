import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { ChatView } from "./chat-panel";

import { useShell } from "~/contexts/shell";

export function PersistentChatPanel({
  panelContainerRef,
  floatingContainerRef,
}: {
  panelContainerRef: React.RefObject<HTMLDivElement | null>;
  floatingContainerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const { chat } = useShell();
  const mode = chat.mode;
  const isFloating = mode === "FloatingOpen";
  const isPanel = mode === "RightPanelOpen";
  const isVisible = isFloating || isPanel;

  const [hasBeenOpened, setHasBeenOpened] = useState(false);
  const [containerRect, setContainerRect] = useState<DOMRect | null>(null);
  const observerRef = useRef<ResizeObserver | null>(null);

  const getActiveContainer = () => {
    if (isPanel) {
      return panelContainerRef.current;
    }

    return (
      floatingContainerRef.current?.querySelector<HTMLDivElement>(
        "[data-chat-floating-anchor]",
      ) ?? floatingContainerRef.current
    );
  };

  useEffect(() => {
    if (isVisible && !hasBeenOpened) {
      setHasBeenOpened(true);
    }
  }, [isVisible, hasBeenOpened]);

  useHotkeys(
    "esc",
    () => chat.sendEvent({ type: "CLOSE" }),
    {
      enabled: isVisible,
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [chat, isVisible],
  );

  useHotkeys(
    "mod+r",
    () => chat.sendEvent({ type: "SHIFT" }),
    {
      enabled: isVisible,
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [chat, isVisible],
  );

  useLayoutEffect(() => {
    const container = getActiveContainer();

    if (!isVisible || !container) {
      setContainerRect(null);
      return;
    }
    setContainerRect(container.getBoundingClientRect());
  }, [isVisible, isPanel, panelContainerRef, floatingContainerRef]);

  useEffect(() => {
    const container = getActiveContainer();

    if (!isVisible || !container) {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }
      return;
    }

    const updateRect = () => {
      setContainerRect(container.getBoundingClientRect());
    };

    observerRef.current = new ResizeObserver(updateRect);
    observerRef.current.observe(container);
    window.addEventListener("resize", updateRect);
    window.addEventListener("scroll", updateRect, true);

    return () => {
      observerRef.current?.disconnect();
      observerRef.current = null;
      window.removeEventListener("resize", updateRect);
      window.removeEventListener("scroll", updateRect, true);
    };
  }, [isVisible, isPanel, panelContainerRef, floatingContainerRef]);

  if (!hasBeenOpened) {
    return null;
  }

  if (isPanel) {
    return (
      <div
        className="pointer-events-none fixed z-100"
        style={
          containerRect
            ? {
                top: containerRect.top,
                left: containerRect.left,
                width: containerRect.width,
                height: containerRect.height,
              }
            : { display: "none" }
        }
      >
        <div className="pointer-events-auto flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden">
          <ChatView />
        </div>
      </div>
    );
  }

  return (
    <AnimatePresence>
      {isFloating && (
        <motion.div
          className="pointer-events-none fixed z-100"
          style={
            containerRect
              ? {
                  top: containerRect.top,
                  left: containerRect.left,
                  width: containerRect.width,
                  height: containerRect.height,
                }
              : { display: "none" }
          }
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2 }}
        >
          <div
            className="pointer-events-auto flex h-full min-h-0 items-end justify-center px-4 pb-4"
            onClick={(event) => {
              if (event.target === event.currentTarget) {
                chat.sendEvent({ type: "CLOSE" });
              }
            }}
          >
            <motion.div
              className={cn([
                "relative flex min-h-0 flex-col overflow-hidden",
                "max-h-[min(70vh,calc(100%_-_1rem))] w-full max-w-[640px]",
                "rounded-2xl bg-white shadow-2xl",
                "border border-neutral-200",
              ])}
              initial={{ y: 40, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: 40, opacity: 0 }}
              transition={{ duration: 0.25, ease: [0.32, 0.72, 0, 1] }}
            >
              <ChatView />
            </motion.div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
