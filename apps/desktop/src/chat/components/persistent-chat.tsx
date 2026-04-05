import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { ChatView } from "./chat-panel";

import { useShell } from "~/contexts/shell";

export function PersistentChatPanel({
  panelContainerRef,
}: {
  panelContainerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const { chat } = useShell();
  const mode = chat.mode;
  const isFloating = mode === "FloatingOpen";
  const isPanel = mode === "RightPanelOpen";
  const isVisible = isFloating || isPanel;

  const [hasBeenOpened, setHasBeenOpened] = useState(false);
  const [panelRect, setPanelRect] = useState<DOMRect | null>(null);
  const observerRef = useRef<ResizeObserver | null>(null);

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
    if (!isPanel || !panelContainerRef.current) {
      setPanelRect(null);
      return;
    }
    setPanelRect(panelContainerRef.current.getBoundingClientRect());
  }, [isPanel, panelContainerRef]);

  useEffect(() => {
    if (!isPanel || !panelContainerRef.current) {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }
      return;
    }

    const el = panelContainerRef.current;
    const updateRect = () => {
      setPanelRect(el.getBoundingClientRect());
    };

    observerRef.current = new ResizeObserver(updateRect);
    observerRef.current.observe(el);
    window.addEventListener("resize", updateRect);

    return () => {
      observerRef.current?.disconnect();
      observerRef.current = null;
      window.removeEventListener("resize", updateRect);
    };
  }, [isPanel, panelContainerRef]);

  if (!hasBeenOpened) {
    return null;
  }

  if (isPanel) {
    return (
      <div
        className="pointer-events-none fixed z-100"
        style={
          panelRect
            ? {
                top: panelRect.top,
                left: panelRect.left,
                width: panelRect.width,
                height: panelRect.height,
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
          className="fixed inset-0 z-100 flex items-end justify-center px-4 pb-4"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2 }}
        >
          <div
            className="absolute inset-0 bg-black/20"
            onClick={() => chat.sendEvent({ type: "CLOSE" })}
          />
          <motion.div
            className={cn([
              "relative flex flex-col overflow-hidden",
              "max-h-[70vh] w-full max-w-[640px]",
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
        </motion.div>
      )}
    </AnimatePresence>
  );
}
