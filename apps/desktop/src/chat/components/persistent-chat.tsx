import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, motion } from "motion/react";
import { useLayoutEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { ChatPanelFrame } from "./chat-panel";

import type { ChatSessionRenderProps } from "~/chat/components/session-provider";
import { chatFloatingPanelShellStyle } from "~/chat/surface";
import { useShell } from "~/contexts/shell";

const FLOATING_CHAT_INPUT_MAX_WIDTH = 640;
const FLOATING_CHAT_SHELL_INSET = 4;
const FLOATING_PANEL_MIN_WIDTH = 476;
const FLOATING_PANEL_DEFAULT_MAX_WIDTH =
  FLOATING_CHAT_INPUT_MAX_WIDTH + FLOATING_CHAT_SHELL_INSET * 2;
const FLOATING_PANEL_TOP_CLEARANCE = 46;
const FLOATING_PANEL_EASE = [0.22, 1, 0.36, 1] as const;

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

  const [containerRect, setContainerRect] =
    useState<FloatingContainerRect | null>(null);
  const [draftHasContent, setDraftHasContent] = useState(false);

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
    const root = floatingContainerRef.current;
    const container = getActiveContainer();

    if (!isVisible || !root || !container) {
      return;
    }

    const updateRect = () => {
      const nextRect = getContainerRect();
      setContainerRect((currentRect) =>
        areFloatingContainerRectsEqual(currentRect, nextRect)
          ? currentRect
          : nextRect,
      );
    };

    updateRect();
    const observer = new ResizeObserver(updateRect);
    if (root !== container) {
      observer.observe(root);
    }
    observer.observe(container);
    window.addEventListener("resize", updateRect);
    window.addEventListener("scroll", updateRect, true);

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", updateRect);
      window.removeEventListener("scroll", updateRect, true);
    };
  }, [isVisible, floatingContainerRef]);

  const panelMotion = {
    initial: { y: 10, scale: 0.985 },
    animate: { y: 0, scale: 1 },
    exit: { y: 6, scale: 0.99 },
  };
  const panelTransition = { duration: 0.18, ease: FLOATING_PANEL_EASE };
  const panelStyle = {
    width: "100%",
    minWidth: `min(${FLOATING_PANEL_MIN_WIDTH}px, 100%)`,
    maxWidth: `${FLOATING_PANEL_DEFAULT_MAX_WIDTH}px`,
    maxHeight: "100%",
    transformOrigin: "bottom center",
    willChange: "transform",
  };

  if (typeof document === "undefined") {
    return null;
  }

  return createPortal(
    <AnimatePresence initial={false}>
      {isVisible && (
        <motion.div
          data-chat-floating-overlay
          {...mergeStyleXProps(
            styles.overlay,
            undefined,
            containerRect
              ? {
                  top: containerRect.top,
                  left: containerRect.left,
                  width: containerRect.width,
                  height: containerRect.height,
                  willChange: "opacity",
                }
              : { display: "none" },
          )}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12, ease: FLOATING_PANEL_EASE }}
        >
          <div
            data-chat-floating-frame
            {...mergeStyleXProps(styles.frame, undefined, {
              paddingTop: FLOATING_PANEL_TOP_CLEARANCE,
            })}
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
              data-chat-panel-reveal="lift"
              data-chat-size="floating"
              {...mergeStyleXProps(
                [styles.panel, chatFloatingPanelShellStyle],
                undefined,
                panelStyle,
              )}
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
    </AnimatePresence>,
    document.body,
  );
}

const styles = stylex.create({
  overlay: {
    pointerEvents: "none",
    position: "fixed",
  },
  frame: {
    alignItems: "flex-end",
    display: "flex",
    height: "100%",
    justifyContent: "center",
    minHeight: 0,
    paddingBottom: "0.5rem",
    paddingInline: "0.75rem",
    pointerEvents: "auto",
    position: "relative",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    minHeight: 0,
    overflow: "hidden",
    position: "relative",
  },
});

function toFloatingContainerRect(rect: DOMRect): FloatingContainerRect {
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}

function areFloatingContainerRectsEqual(
  currentRect: FloatingContainerRect | null,
  nextRect: FloatingContainerRect | null,
) {
  return (
    currentRect?.top === nextRect?.top &&
    currentRect?.left === nextRect?.left &&
    currentRect?.width === nextRect?.width &&
    currentRect?.height === nextRect?.height
  );
}
