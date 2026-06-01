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

import { ChatView } from "./chat-panel";

import { useShell } from "~/contexts/shell";

const FLOATING_PANEL_MIN_WIDTH = 360;
const FLOATING_PANEL_MIN_HEIGHT = 320;
const FLOATING_PANEL_MARGIN = 16;

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
  {
    id: "bottom-left",
    className: "bottom-0 left-0 h-7 w-7 cursor-nesw-resize",
  },
  {
    id: "bottom-right",
    className: "right-0 bottom-0 h-7 w-7 cursor-nwse-resize",
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
}: {
  floatingContainerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const { chat } = useShell();
  const isVisible = chat.mode === "FloatingOpen";

  const [hasBeenOpened, setHasBeenOpened] = useState(false);
  const [containerRect, setContainerRect] =
    useState<FloatingContainerRect | null>(null);
  const [isExpanded, setIsExpanded] = useState(false);
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
    const root = floatingContainerRef.current;
    const anchor = getActiveContainer();

    if (!root || !anchor) {
      return null;
    }

    const anchorRect = anchor.getBoundingClientRect();

    if (!isExpanded) {
      return toFloatingContainerRect(anchorRect);
    }

    const rootRect = root.getBoundingClientRect();
    const bottom = Math.max(rootRect.bottom, anchorRect.bottom);

    return {
      top: anchorRect.top,
      left: anchorRect.left,
      width: anchorRect.width,
      height: bottom - anchorRect.top,
    };
  };

  useEffect(() => {
    if (isVisible && !hasBeenOpened) {
      setHasBeenOpened(true);
    }

    if (!isVisible) {
      setIsExpanded(false);
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
  }, [isVisible, isExpanded, hasBeenOpened, floatingContainerRef]);

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
  }, [isVisible, isExpanded, hasBeenOpened, floatingContainerRef]);

  if (!hasBeenOpened) {
    return null;
  }

  const panelMotion = isExpanded
    ? {
        initial: { opacity: 0, scale: 0.985, filter: "blur(4px)" },
        animate: { opacity: 1, scale: 1, filter: "blur(0px)" },
        exit: { opacity: 0, scale: 0.99, filter: "blur(3px)" },
      }
    : {
        initial: { y: 24, opacity: 0, scale: 0.96, filter: "blur(6px)" },
        animate: { y: 0, opacity: 1, scale: 1, filter: "blur(0px)" },
        exit: { y: 18, opacity: 0, scale: 0.97, filter: "blur(4px)" },
      };
  const panelTransition = {
    opacity: { duration: 0.18, ease: [0.22, 1, 0.36, 1] },
    scale: { duration: 0.24, ease: [0.22, 1, 0.36, 1] },
    y: { duration: 0.28, ease: [0.22, 1, 0.36, 1] },
    filter: { duration: 0.16, ease: "easeOut" },
  };
  const panelStyle = isExpanded
    ? { transformOrigin: "center" }
    : floatingSize && containerRect
      ? getFloatingPanelStyle(floatingSize, containerRect)
      : {
          width: "min(640px, calc(100% - 2rem))",
          height: "min(560px, calc(100% - 1rem))",
          minWidth: "min(360px, calc(100% - 2rem))",
          minHeight: "min(320px, calc(100% - 1rem))",
          maxWidth: "calc(100% - 2rem)",
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
              isExpanded
                ? "items-stretch justify-center p-0"
                : "items-end justify-center p-4",
            ])}
            onClick={(event) => {
              if (!isExpanded && event.target === event.currentTarget) {
                chat.sendEvent({ type: "CLOSE" });
              }
            }}
          >
            <motion.div
              ref={panelRef}
              data-chat-panel
              data-chat-size={isExpanded ? "expanded" : "floating"}
              className={cn([
                "relative flex min-h-0 flex-col overflow-hidden",
                "bg-stone-800 text-white",
                isExpanded
                  ? "h-full w-full rounded-none border-0"
                  : [
                      "rounded-2xl border-2 border-stone-600",
                      "shadow-[0_4px_28px_rgba(87,83,78,0.45)]",
                    ],
              ])}
              style={panelStyle}
              initial={panelMotion.initial}
              animate={panelMotion.animate}
              exit={panelMotion.exit}
              transition={panelTransition}
            >
              <ChatView
                isExpanded={isExpanded}
                onToggleExpanded={() => setIsExpanded((value) => !value)}
              />
              {!isExpanded &&
                RESIZE_HANDLES.map((handle) => (
                  <div
                    key={handle.id}
                    data-chat-resize-handle={handle.id}
                    className={cn([
                      "absolute z-20 touch-none select-none",
                      handle.className,
                    ])}
                    onPointerDown={(event) =>
                      handleResizeStart(handle.id, event)
                    }
                    onPointerMove={handleResizeMove}
                    onPointerUp={handleResizeEnd}
                    onPointerCancel={handleResizeEnd}
                  >
                    <ResizeHandleIndicator handle={handle.id} />
                  </div>
                ))}
            </motion.div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function ResizeHandleIndicator({ handle }: { handle: ResizeHandle }) {
  const className = getResizeHandleIndicatorClassName(handle);

  if (!className) {
    return null;
  }

  return (
    <span
      className={cn([
        "pointer-events-none absolute h-3 w-3 border-stone-300/45",
        className,
      ])}
    />
  );
}

function getResizeHandleIndicatorClassName(handle: ResizeHandle) {
  switch (handle) {
    case "top-left":
      return "top-1.5 left-1.5 rounded-tl-md border-t border-l";
    case "top-right":
      return "top-1.5 right-1.5 rounded-tr-md border-t border-r";
    case "bottom-left":
      return "bottom-1.5 left-1.5 rounded-bl-md border-b border-l";
    case "bottom-right":
      return "right-1.5 bottom-1.5 rounded-br-md border-r border-b";
    default:
      return null;
  }
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
    nextSize.width -= deltaX * 2;
  }

  if (resizeState.handle.includes("right")) {
    nextSize.width += deltaX * 2;
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
