import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { ChatView } from "./chat-panel";

import { useShell } from "~/contexts/shell";

export function PersistentChatPanel({
  panelContainerRef,
}: {
  panelContainerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const { chat } = useShell();
  const isVisible = chat.mode === "RightPanelOpen";

  const [hasBeenOpened, setHasBeenOpened] = useState(false);
  const [containerRect, setContainerRect] = useState<DOMRect | null>(null);
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

  useLayoutEffect(() => {
    const container = panelContainerRef.current;

    if (!isVisible || !container) {
      setContainerRect(null);
      return;
    }
    setContainerRect(container.getBoundingClientRect());
  }, [isVisible, panelContainerRef]);

  useEffect(() => {
    const container = panelContainerRef.current;

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
  }, [isVisible, panelContainerRef]);

  if (!hasBeenOpened || !isVisible) {
    return null;
  }

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
