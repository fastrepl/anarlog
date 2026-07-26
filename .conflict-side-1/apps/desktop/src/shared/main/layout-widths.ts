import type { Tab } from "~/store/zustand/tabs";

export const NOTE_SURFACE_MIN_WIDTH_PX = 500;

export function usesNoteSurfaceMinWidth(tab: Pick<Tab, "type"> | null) {
  return (
    tab?.type === "sessions" ||
    tab?.type === "shared_sessions" ||
    tab?.type === "shared_note_preview" ||
    tab?.type === "empty"
  );
}
