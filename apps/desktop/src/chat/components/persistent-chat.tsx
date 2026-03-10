import { Resizable } from "re-resizable";
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
  const [floatingSize, setFloatingSize] = useState({
    width: 400,
    height: window.innerHeight * 0.7,
  });
  const [isResizing, setIsResizing] = useState(false);
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
      enabled: isFloating,
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [chat, isFloating],
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

  const panelStyle: React.CSSProperties | undefined =
    isPanel && panelRect
      ? {
          top: panelRect.top,
          left: panelRect.left,
          width: panelRect.width,
          height: panelRect.height,
        }
      : undefined;

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
          : (panelStyle ?? { display: "none" })
      }
    >
      <Resizable
        size={isFloating ? floatingSize : { width: "100%", height: "100%" }}
        onResizeStart={isFloating ? () => setIsResizing(true) : undefined}
        onResizeStop={
          isFloating
            ? (_, __, ___, d) => {
                setFloatingSize((prev) => ({
                  width: prev.width + d.width,
                  height: prev.height + d.height,
                }));
                setIsResizing(false);
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
        bounds={isFloating ? "window" : undefined}
        className={cn([
          "pointer-events-auto flex flex-col",
          isFloating && [
            "overflow-hidden rounded-t-xl rounded-b-2xl bg-white shadow-2xl",
            "border border-neutral-200",
            !isResizing && "transition-all duration-200",
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
