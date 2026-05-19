const FLOATING_BUTTON_SCROLL_TOP_THRESHOLD = 8;
const FLOATING_BUTTON_SCROLL_BOTTOM_THRESHOLD = 64;
const FLOATING_BUTTON_SCROLL_DIRECTION_THRESHOLD = 2;

export function getNextFloatingButtonHidden({
  currentHidden,
  delta,
  scrollTop,
  scrollHeight,
  clientHeight,
}: {
  currentHidden: boolean;
  delta: number;
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}) {
  const distanceToBottom = scrollHeight - scrollTop - clientHeight;
  const isNearBottom =
    distanceToBottom <= FLOATING_BUTTON_SCROLL_BOTTOM_THRESHOLD;

  if (scrollTop <= FLOATING_BUTTON_SCROLL_TOP_THRESHOLD) {
    return false;
  }

  if (delta > FLOATING_BUTTON_SCROLL_DIRECTION_THRESHOLD) {
    return true;
  }

  if (delta < -FLOATING_BUTTON_SCROLL_DIRECTION_THRESHOLD && !isNearBottom) {
    return false;
  }

  return currentHidden;
}
