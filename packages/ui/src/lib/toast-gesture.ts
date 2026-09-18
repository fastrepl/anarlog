export type AppToastSwipeDismissDirection = -1 | 1;

const APP_TOAST_SWIPE_DISTANCE_PX = 72;
const APP_TOAST_SWIPE_VELOCITY_PX = 600;

/** Returns the side a released toast should exit through, or keeps it in place. */
export function appToastSwipeDismissDirection(
  offsetX: number,
  velocityX: number,
): AppToastSwipeDismissDirection | undefined {
  if (
    Math.abs(offsetX) < APP_TOAST_SWIPE_DISTANCE_PX &&
    Math.abs(velocityX) < APP_TOAST_SWIPE_VELOCITY_PX
  ) {
    return undefined;
  }

  return offsetX + velocityX * 0.12 < 0 ? -1 : 1;
}
