import { useLayoutEffect, useState } from "react";

const CHAT_PANEL_CONTAINER_SELECTOR = "[data-chat-panel-container]";
const FALLBACK_CHAT_PANEL_WIDTH = 400;
const CHAT_TOOLBAR_LEFT_BLEED_PX = 12;

export function useChatPanelToolbarWidth(isEnabled: boolean) {
  const [width, setWidth] = useState<number | null>(null);

  useLayoutEffect(() => {
    if (!isEnabled) {
      setWidth(null);
      return;
    }

    const getContainer = () =>
      document.querySelector<HTMLElement>(CHAT_PANEL_CONTAINER_SELECTOR);

    const updateWidth = () => {
      const container = getContainer();
      setWidth(container?.getBoundingClientRect().width ?? null);
    };

    updateWidth();

    const resizeObserver = new ResizeObserver(updateWidth);
    const container = getContainer();
    if (container) {
      resizeObserver.observe(container);
    }

    window.addEventListener("resize", updateWidth);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", updateWidth);
    };
  }, [isEnabled]);

  const measuredWidth = width ?? (isEnabled ? FALLBACK_CHAT_PANEL_WIDTH : null);
  return measuredWidth === null
    ? null
    : measuredWidth + CHAT_TOOLBAR_LEFT_BLEED_PX;
}
