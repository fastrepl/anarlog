import { create } from "zustand";

interface LiveTitleState {
  titles: Record<string, string>;
  setTitle: (sessionId: string, title: string) => void;
  clearTitle: (sessionId: string) => void;
}

export const useLiveTitle = create<LiveTitleState>((set) => ({
  titles: {},
  setTitle: (sessionId, title) =>
    set((state) => ({ titles: { ...state.titles, [sessionId]: title } })),
  clearTitle: (sessionId) =>
    set((state) => {
      const { [sessionId]: _, ...rest } = state.titles;
      return { titles: rest };
    }),
}));

export function hasLiveSessionTitleDraft(sessionId: string): boolean {
  return Object.prototype.hasOwnProperty.call(
    useLiveTitle.getState().titles,
    sessionId,
  );
}

export function useSessionTitle(
  sessionId: string,
  storeTitle: string | undefined,
): string {
  const liveTitle = useLiveTitle((s) => s.titles[sessionId]);
  return liveTitle ?? (storeTitle as string) ?? "Untitled";
}
