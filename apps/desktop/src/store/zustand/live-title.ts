import { create } from "zustand";

const LIVE_TITLE_ACK_WINDOW_MS = 10_000;

interface LiveTitleState {
  titles: Record<
    string,
    {
      value: string;
      persisted: boolean;
      persistedAt: number | null;
      previousTitle: string | undefined;
    }
  >;
  setTitle: (sessionId: string, title: string) => void;
  markTitlePersisted: (
    sessionId: string,
    title: string,
    previousTitle: string | undefined,
  ) => void;
  clearTitle: (sessionId: string) => void;
}

export const useLiveTitle = create<LiveTitleState>((set) => ({
  titles: {},
  setTitle: (sessionId, title) =>
    set((state) => ({
      titles: {
        ...state.titles,
        [sessionId]: {
          value: title,
          persisted: false,
          persistedAt: null,
          previousTitle: undefined,
        },
      },
    })),
  markTitlePersisted: (sessionId, title, previousTitle) =>
    set((state) => {
      const current = state.titles[sessionId];
      if (current?.value !== title) return state;

      return {
        titles: {
          ...state.titles,
          [sessionId]: {
            value: title,
            persisted: true,
            persistedAt: Date.now(),
            previousTitle,
          },
        },
      };
    }),
  clearTitle: (sessionId) =>
    set((state) => {
      const { [sessionId]: _, ...rest } = state.titles;
      return { titles: rest };
    }),
}));

export function hasLiveSessionTitleDraft(sessionId: string): boolean {
  const title = useLiveTitle.getState().titles[sessionId];
  return Boolean(title && !title.persisted);
}

export function resolveLiveSessionTitle(
  liveTitle:
    | {
        value: string;
        persisted: boolean;
        persistedAt: number | null;
        previousTitle: string | undefined;
      }
    | undefined,
  storeTitle: string | undefined,
): string {
  if (!liveTitle) return storeTitle ?? "Untitled";
  if (!liveTitle.persisted) return liveTitle.value;
  if (
    storeTitle === liveTitle.previousTitle &&
    liveTitle.persistedAt !== null &&
    Date.now() - liveTitle.persistedAt <= LIVE_TITLE_ACK_WINDOW_MS
  ) {
    return liveTitle.value;
  }
  return storeTitle ?? liveTitle.value;
}

export function useSessionTitle(
  sessionId: string,
  storeTitle: string | undefined,
): string {
  const liveTitle = useLiveTitle((s) => s.titles[sessionId]);
  return resolveLiveSessionTitle(liveTitle, storeTitle);
}
