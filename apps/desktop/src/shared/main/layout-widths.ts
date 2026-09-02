import type { Tab } from "~/store/zustand/tabs";

export const NOTE_SURFACE_MIN_WIDTH_PX = 500;
export const AUTOMATIONS_SURFACE_MIN_WIDTH_PX = 600;
export const SETTINGS_SURFACE_MIN_WIDTH_PX = 700;

export function usesNoteSurfaceMinWidth(tab: Pick<Tab, "type"> | null) {
  return (
    tab?.type === "sessions" ||
    tab?.type === "shared_sessions" ||
    tab?.type === "shared_note_preview" ||
    tab?.type === "empty"
  );
}

export function getMainContentMinWidth(tab: Pick<Tab, "type"> | null) {
  if (tab?.type === "automations") {
    return AUTOMATIONS_SURFACE_MIN_WIDTH_PX;
  }
  if (tab?.type === "settings") {
    return SETTINGS_SURFACE_MIN_WIDTH_PX;
  }
  return usesNoteSurfaceMinWidth(tab) ? NOTE_SURFACE_MIN_WIDTH_PX : undefined;
}

export function boundedMinWidthPx(px: number): string {
  return `min(${px}px, 100%)`;
}
