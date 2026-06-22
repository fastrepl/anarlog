import { AnimatePresence, motion } from "motion/react";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { ChatPanelFrame } from "./chat-panel";

import type { ChatSessionRenderProps } from "~/chat/components/session-provider";
import { chatFloatingPanelShellClassNames } from "~/chat/surface";
import { useShell } from "~/contexts/shell";

const FLOATING_PANEL_MIN_WIDTH = 368;
const FLOATING_PANEL_MIN_HEIGHT = 320;
const FLOATING_PANEL_MARGIN = 12;

type FloatingPanelSize = {
  width: number;
  height: number;
};

type FloatingContainerRect = {
  top: number;
  left: number;
  width: number;
  height: number;
};

const RESIZE_HANDLES = [
  {
    id: "top-left",
    className: "top-0 left-0 h-4 w-7 cursor-nwse-resize",
  },
  {
    id: "top",
    className: "top-0 right-7 left-7 h-3 cursor-row-resize",
  },
  {
    id: "top-right",
    className: "top-0 right-0 h-4 w-7 cursor-nesw-resize",
  },
  {
    id: "right",
    className: "top-7 right-0 bottom-7 w-3 cursor-ew-resize",
  },
  {
    id: "bottom",
    className: "right-7 bottom-0 left-7 h-3 cursor-row-resize",
  },
  {
    id: "left",
    className: "top-7 bottom-7 left-0 w-3 cursor-ew-resize",
  },
] as const;

type ResizeHandle = (typeof RESIZE_HANDLES)[number]["id"];

type ResizeState = {
  pointerId: number;
  handle: ResizeHandle;
  startX: number;
  startY: number;
  startSize: FloatingPanelSize;
  containerWidth: number;
  containerHeight: number;
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
  const [floatingSize, setFloatingSize] = useState<FloatingPanelSize | null>(
    null,
  );
  const resizeFrameRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const resizeStateRef = useRef<ResizeState | null>(null);
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

    if (!isVisible) {
      resizeStateRef.current = null;
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
      clipPath: "inset(calc(100% - 5rem) 0 0 0 round 1.75rem)",
    },
    animate: {
      y: 0,
      opacity: 1,
      scale: 1,
      clipPath: "inset(0 0 0 0 round 1.75rem)",
    },
    exit: {
      y: 8,
      opacity: 0,
      scale: 0.99,
      clipPath: "inset(calc(100% - 5rem) 0 0 0 round 1.75rem)",
    },
  };
  const panelTransition = {
    opacity: { duration: 0.18, ease: [0.22, 1, 0.36, 1] },
    scale: { duration: 0.2, ease: [0.22, 1, 0.36, 1] },
    y: { duration: 0.22, ease: [0.22, 1, 0.36, 1] },
    clipPath: { duration: 0.32, ease: [0.22, 1, 0.36, 1] },
  };
  const panelStyle =
    floatingSize && containerRect
      ? getFloatingPanelStyle(floatingSize, containerRect)
      : {
          width: "calc(100% - 1.5rem)",
          minWidth: "min(368px, calc(100% - 1.5rem))",
          maxWidth: "648px",
          maxHeight: "calc(100% - 1rem)",
          transformOrigin: "bottom center",
        };

  const handleResizeStart = (
    handle: ResizeHandle,
    event: PointerEvent<HTMLDivElement>,
  ) => {
    const panel = panelRef.current;
    const frame = resizeFrameRef.current;

    if (!panel || !frame) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture?.(event.pointerId);

    const panelRect = panel.getBoundingClientRect();
    const frameRect = frame.getBoundingClientRect();

    resizeStateRef.current = {
      pointerId: event.pointerId,
      handle,
      startX: event.clientX,
      startY: event.clientY,
      startSize: {
        width: panelRect.width,
        height: panelRect.height,
      },
      containerWidth: frameRect.width,
      containerHeight: frameRect.height,
    };
  };

  const handleResizeMove = (event: PointerEvent<HTMLDivElement>) => {
    const resizeState = resizeStateRef.current;

    if (!resizeState || resizeState.pointerId !== event.pointerId) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    const deltaX = event.clientX - resizeState.startX;
    const deltaY = event.clientY - resizeState.startY;
    const nextSize = getResizedSize(resizeState, deltaX, deltaY);

    setFloatingSize(
      clampFloatingPanelSize(
        nextSize,
        resizeState.containerWidth,
        resizeState.containerHeight,
      ),
    );
  };

  const handleResizeEnd = (event: PointerEvent<HTMLDivElement>) => {
    const resizeState = resizeStateRef.current;

    if (!resizeState || resizeState.pointerId !== event.pointerId) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.releasePointerCapture?.(event.pointerId);
    resizeStateRef.current = null;
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
            ref={resizeFrameRef}
            data-chat-resize-frame
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
              ref={panelRef}
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
              {RESIZE_HANDLES.map((handle) => (
                <div
                  key={handle.id}
                  data-chat-resize-handle={handle.id}
                  className={cn([
                    "absolute z-20 touch-none select-none",
                    handle.className,
                  ])}
                  onPointerDown={(event) => handleResizeStart(handle.id, event)}
                  onPointerMove={handleResizeMove}
                  onPointerUp={handleResizeEnd}
                  onPointerCancel={handleResizeEnd}
                />
              ))}
            </motion.div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function getFloatingPanelStyle(
  size: FloatingPanelSize,
  containerRect: FloatingContainerRect,
): CSSProperties {
  const clampedSize = clampFloatingPanelSize(
    size,
    containerRect.width,
    containerRect.height,
  );

  return {
    width: `${clampedSize.width}px`,
    height: `${clampedSize.height}px`,
    transformOrigin: "bottom center",
  };
}

function toFloatingContainerRect(rect: DOMRect): FloatingContainerRect {
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}

function getResizedSize(
  resizeState: ResizeState,
  deltaX: number,
  deltaY: number,
): FloatingPanelSize {
  const nextSize = { ...resizeState.startSize };

  if (resizeState.handle.includes("left")) {
    nextSize.width -= deltaX;
  }

  if (resizeState.handle.includes("right")) {
    nextSize.width += deltaX;
  }

  if (resizeState.handle.includes("top")) {
    nextSize.height -= deltaY;
  }

  if (resizeState.handle.includes("bottom")) {
    nextSize.height += deltaY;
  }

  return nextSize;
}

function clampFloatingPanelSize(
  size: FloatingPanelSize,
  containerWidth: number,
  containerHeight: number,
): FloatingPanelSize {
  const maxWidth = Math.max(0, containerWidth - FLOATING_PANEL_MARGIN * 2);
  const maxHeight = Math.max(0, containerHeight - FLOATING_PANEL_MARGIN * 2);
  const minWidth = Math.min(FLOATING_PANEL_MIN_WIDTH, maxWidth);
  const minHeight = Math.min(FLOATING_PANEL_MIN_HEIGHT, maxHeight);
  const width = clamp(size.width, minWidth, maxWidth);
  const height = clamp(size.height, minHeight, maxHeight);

  return { width, height };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}
