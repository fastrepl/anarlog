import type { ImperativePanelHandle } from "@anlg/ui/components/ui/resizable";

export const LEFT_SIDEBAR_DEFAULT_WIDTH_PX = 200;
export const LEFT_SIDEBAR_MIN_WIDTH_PX = 200;
export const LEFT_SIDEBAR_MAX_WIDTH_PX = 360;
export const LEFT_SIDEBAR_COLLAPSED_SIZE = 0;

const LEFT_SIDEBAR_FALLBACK_CONTAINER_WIDTH_PX = 1000;
const LEFT_SIDEBAR_PANEL_SIZE_EPSILON = 0.01;

export function createLeftSidebarPanelConstraints(widthPx?: number) {
  const containerWidthPx = Math.max(
    widthPx ?? getInitialMainAreaWidthPx(),
    LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
  );
  const minSize = percentageFromPixels(
    LEFT_SIDEBAR_MIN_WIDTH_PX,
    containerWidthPx,
  );

  return {
    defaultSize: percentageFromPixels(
      LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
      containerWidthPx,
    ),
    minSize,
    maxSize: Math.max(
      minSize,
      percentageFromPixels(LEFT_SIDEBAR_MAX_WIDTH_PX, containerWidthPx),
    ),
  };
}

export function createFixedLeftSidebarPanelConstraints(defaultSize: number) {
  return {
    defaultSize,
    minSize: defaultSize,
    maxSize: defaultSize,
  };
}

export function getMeasuredMainAreaWidthPx(element: HTMLElement | null) {
  const measuredWidth = element?.getBoundingClientRect().width ?? 0;

  return measuredWidth > 0 ? measuredWidth : getInitialMainAreaWidthPx();
}

export function panelSizesAreEqual(left: number, right: number) {
  return Math.abs(left - right) < LEFT_SIDEBAR_PANEL_SIZE_EPSILON;
}

export function resizeLeftSidebarPanel(
  panel: ImperativePanelHandle | null,
  size: number,
) {
  if (!panel) {
    return;
  }

  try {
    panel.resize(size);
  } catch {
    window.requestAnimationFrame(() => {
      try {
        panel.resize(size);
      } catch {
        // The panel can be layoutless while hidden; the CSS variables still restore visual width on reopen.
      }
    });
  }
}

function getInitialMainAreaWidthPx() {
  if (typeof window === "undefined") {
    return LEFT_SIDEBAR_FALLBACK_CONTAINER_WIDTH_PX;
  }

  return (
    window.innerWidth ||
    document.documentElement.clientWidth ||
    LEFT_SIDEBAR_FALLBACK_CONTAINER_WIDTH_PX
  );
}

function percentageFromPixels(widthPx: number, containerWidthPx: number) {
  return Math.min((widthPx / containerWidthPx) * 100, 100);
}
