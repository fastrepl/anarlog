import type { WheelEvent as ReactWheelEvent } from "react";

export function scrollElementByWheel(
  element: HTMLElement | null,
  event: ReactWheelEvent<Element>,
): boolean {
  if (!element || event.defaultPrevented) {
    return false;
  }

  const deltaY = getWheelDeltaY(event, element.clientHeight);
  if (deltaY === 0) {
    return false;
  }

  const maxScrollTop = Math.max(0, element.scrollHeight - element.clientHeight);
  const nextScrollTop = clamp(element.scrollTop + deltaY, 0, maxScrollTop);

  if (nextScrollTop === element.scrollTop) {
    return false;
  }

  event.preventDefault();
  element.scrollTop = nextScrollTop;
  element.dispatchEvent(new Event("scroll"));

  return true;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function getWheelDeltaY(
  event: ReactWheelEvent<Element>,
  pageScrollHeight: number,
): number {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
    return event.deltaY * 16;
  }

  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return event.deltaY * pageScrollHeight;
  }

  return event.deltaY;
}
