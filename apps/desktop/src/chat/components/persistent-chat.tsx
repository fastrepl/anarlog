import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { ChatPanelFrame } from "./chat-panel";

import type { ChatSessionRenderProps } from "~/chat/components/session-provider";
import { chatFloatingPanelShellClassNames } from "~/chat/surface";
import { useShell } from "~/contexts/shell";

const FLOATING_CHAT_INPUT_MAX_WIDTH = 640;
const FLOATING_CHAT_INPUT_HEIGHT = 40;
const FLOATING_CHAT_SHELL_INSET = 4;
const FLOATING_PANEL_DEFAULT_MAX_WIDTH =
  FLOATING_CHAT_INPUT_MAX_WIDTH + FLOATING_CHAT_SHELL_INSET * 2;
const FLOATING_PANEL_REVEAL_HEIGHT =
  FLOATING_CHAT_INPUT_HEIGHT + FLOATING_CHAT_SHELL_INSET;
const FLOATING_PANEL_RADIUS = 28;

type FloatingContainerRect = {
  top: number;
  left: number;
  width: number;
  height: number;
};

export function PersistentChatPanel({
  floatingContainerRef,
  sessionProps,
}: {
  floatingContainerRef: React.RefObject<HTMLDivElement | null>;
  sessionProps: ChatSessionRenderProps | null;
}) {
  const { chat } = useShell();
  const isVisible = chat.mode === "FloatingOpen";

  const [hasBeenOpened, setHasBeenOpened] = useState(false);
  const [containerRect, setContainerRect] =
    useState<FloatingContainerRect | null>(null);
  const [draftHasContent, setDraftHasContent] = useState(false);
  const observerRef = useRef<ResizeObserver | null>(null);

  const getActiveContainer = () => {
    return (
      floatingContainerRef.current?.querySelector<HTMLDivElement>(
        "[data-chat-floating-anchor]",
      ) ?? floatingContainerRef.current
    );
  };

  const getContainerRect = () => {
    const anchor = getActiveContainer();

    if (!anchor) {
      return null;
    }

    return toFloatingContainerRect(anchor.getBoundingClientRect());
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

  useLayoutEffect(() => {
    const nextRect = getContainerRect();

    if (!isVisible || !nextRect) {
      setContainerRect(null);
      return;
    }
    setContainerRect(nextRect);
  }, [isVisible, hasBeenOpened, floatingContainerRef]);

  useEffect(() => {
    const root = floatingContainerRef.current;
    const container = getActiveContainer();

    if (!isVisible || !root || !container) {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }
      return;
    }

    const updateRect = () => {
      setContainerRect(getContainerRect());
    };

    observerRef.current = new ResizeObserver(updateRect);
    if (root !== container) {
      observerRef.current.observe(root);
    }
    observerRef.current.observe(container);
    window.addEventListener("resize", updateRect);
    window.addEventListener("scroll", updateRect, true);

    return () => {
      observerRef.current?.disconnect();
      observerRef.current = null;
      window.removeEventListener("resize", updateRect);
      window.removeEventListener("scroll", updateRect, true);
    };
  }, [isVisible, hasBeenOpened, floatingContainerRef]);

  if (!hasBeenOpened) {
    return null;
  }

  const panelMotion = {
    initial: {
      y: 0,
      opacity: 1,
      scale: 1,
      clipPath: `inset(calc(100% - ${FLOATING_PANEL_REVEAL_HEIGHT}px) 0 0 0 round ${FLOATING_PANEL_RADIUS}px)`,
    },
    animate: {
      y: 0,
      opacity: 1,
      scale: 1,
      clipPath: `inset(0 0 0 0 round ${FLOATING_PANEL_RADIUS}px)`,
    },
    exit: {
      y: 8,
      opacity: 0,
      scale: 0.99,
      clipPath: `inset(calc(100% - ${FLOATING_PANEL_REVEAL_HEIGHT}px) 0 0 0 round ${FLOATING_PANEL_RADIUS}px)`,
    },
  };
  const panelTransition = {
    opacity: { duration: 0.18, ease: [0.22, 1, 0.36, 1] },
    scale: { duration: 0.2, ease: [0.22, 1, 0.36, 1] },
    y: { duration: 0.22, ease: [0.22, 1, 0.36, 1] },
    clipPath: { duration: 0.32, ease: [0.22, 1, 0.36, 1] },
  };
  const panelStyle = {
    width: "calc(100% - 1.5rem)",
    minWidth: "min(368px, calc(100% - 1.5rem))",
    maxWidth: `${FLOATING_PANEL_DEFAULT_MAX_WIDTH}px`,
    maxHeight: "calc(100% - 1rem)",
    transformOrigin: "bottom center",
  };

  return (
    <AnimatePresence>
      {isVisible && (
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
            data-chat-floating-frame
            className={cn([
              "pointer-events-auto relative flex h-full min-h-0",
              "items-end justify-center px-3 pt-4 pb-2",
            ])}
            onClick={(event) => {
              if (event.target === event.currentTarget) {
                if (draftHasContent) {
                  return;
                }

                chat.sendEvent({ type: "CLOSE" });
              }
            }}
          >
            <motion.div
              data-chat-panel
              data-chat-panel-reveal="bottom-up"
              data-chat-size="floating"
              className={cn([
                "relative flex min-h-0 flex-col overflow-hidden",
                chatFloatingPanelShellClassNames(),
              ])}
              style={panelStyle}
              initial={panelMotion.initial}
              animate={panelMotion.animate}
              exit={panelMotion.exit}
              transition={panelTransition}
            >
              <ChatPanelFrame
                layout="floating"
                onDraftContentChange={setDraftHasContent}
                onOpenRightPanel={() =>
                  chat.sendEvent({ type: "OPEN_RIGHT_PANEL" })
                }
                sessionProps={sessionProps}
              />
            </motion.div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function toFloatingContainerRect(rect: DOMRect): FloatingContainerRect {
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}
