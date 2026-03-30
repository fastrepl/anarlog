import { Resizable } from "re-resizable";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { ChatView } from "./chat-panel";

import { useShell } from "~/contexts/shell";

const FLOATING_CHAT_PADDING_PX = 32;
const FLOATING_CHAT_MIN_WIDTH_PX = 320;
const FLOATING_CHAT_MIN_HEIGHT_PX = 400;
const DEFAULT_FLOATING_CHAT_WIDTH_PX = 400;
const DEFAULT_FLOATING_CHAT_HEIGHT_RATIO = 0.7;

function getFloatingViewportSize() {
  return {
    width: Math.max(window.innerWidth - FLOATING_CHAT_PADDING_PX, 1),
    height: Math.max(window.innerHeight - FLOATING_CHAT_PADDING_PX, 1),
  };
}

function clampFloatingSize(size: { width: number; height: number }) {
  const viewport = getFloatingViewportSize();

  return {
    width: Math.min(
      Math.max(
        size.width,
        Math.min(FLOATING_CHAT_MIN_WIDTH_PX, viewport.width),
      ),
      viewport.width,
    ),
    height: Math.min(
      Math.max(
        size.height,
        Math.min(FLOATING_CHAT_MIN_HEIGHT_PX, viewport.height),
      ),
      viewport.height,
    ),
  };
}

function getDefaultFloatingSize() {
  return clampFloatingSize({
    width: DEFAULT_FLOATING_CHAT_WIDTH_PX,
    height: window.innerHeight * DEFAULT_FLOATING_CHAT_HEIGHT_RATIO,
  });
}

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
  const [floatingSize, setFloatingSize] = useState(getDefaultFloatingSize);
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

  return (
    <div
      className={cn([
        "fixed z-100",
        !isVisible && "hidden!",
        isPanel && "pointer-events-none",
      ])}
      style={
        isFloating
          ? { right: 16, bottom: 16 }
          : isPanel && panelRect
            ? {
                top: panelRect.top,
                left: panelRect.left,
                width: panelRect.width,
                height: panelRect.height,
              }
            : { display: "none" }
      }
    >
      <Resizable
        size={isPanel ? { width: "100%", height: "100%" } : floatingSize}
        onResizeStop={
          isFloating
            ? (_, __, ___, d) => {
                setFloatingSize((prev) =>
                  clampFloatingSize({
                    width: prev.width + d.width,
                    height: prev.height + d.height,
                  }),
                );
              }
            : undefined
        }
        enable={
          isFloating
            ? {
                top: true,
                right: false,
                bottom: false,
                left: true,
                topRight: false,
                bottomRight: false,
                bottomLeft: false,
                topLeft: true,
              }
            : false
        }
        minWidth={isFloating ? FLOATING_CHAT_MIN_WIDTH_PX : undefined}
        minHeight={isFloating ? FLOATING_CHAT_MIN_HEIGHT_PX : undefined}
        bounds={isFloating ? "window" : undefined}
        boundsByDirection={isFloating}
        className={cn([
          "pointer-events-auto flex min-h-0 min-w-0 flex-col overflow-hidden",
          isFloating && [
            "overflow-hidden rounded-t-xl rounded-b-2xl bg-stone-50 shadow-2xl",
            "border border-neutral-200",
          ],
          isPanel && "h-full w-full",
        ])}
        handleStyles={
          isFloating
            ? {
                top: { height: "4px", top: 0 },
                left: { width: "4px", left: 0 },
                topLeft: {
                  width: "12px",
                  height: "12px",
                  top: 0,
                  left: 0,
                },
              }
            : undefined
        }
      >
        <ChatView />
      </Resizable>
    </div>
  );
}
