import { create } from "zustand";

import type { LiveAssistKind } from "~/session/insights/live-assist";

export type LiveAssistCardStatus = "generating" | "ready" | "error";

export type LiveAssistCard = {
  id: string;
  kind: LiveAssistKind;
  createdAtMs: number;
  status: LiveAssistCardStatus;
  items?: string[];
  errorMessage?: string;
};

// A very long meeting should not let this grow unbounded in memory; cards
// are not persisted, so trimming the oldest ones is a safe, simple cap.
const MAX_CARDS_PER_SESSION = 20;

const EMPTY_CARDS: LiveAssistCard[] = [];

type LiveAssistState = {
  cardsBySession: Record<string, LiveAssistCard[]>;
};

type LiveAssistActions = {
  addGeneratingCard: (
    sessionId: string,
    card: Pick<LiveAssistCard, "id" | "kind" | "createdAtMs">,
  ) => void;
  resolveCard: (
    sessionId: string,
    cardId: string,
    result:
      | { status: "ready"; items: string[] }
      | { status: "error"; errorMessage: string },
  ) => void;
  removeCard: (sessionId: string, cardId: string) => void;
  clearSession: (sessionId: string) => void;
};

export const useLiveAssistStore = create<LiveAssistState & LiveAssistActions>(
  (set) => ({
    cardsBySession: {},
    addGeneratingCard: (sessionId, card) =>
      set((state) => {
        const existing = state.cardsBySession[sessionId] ?? EMPTY_CARDS;
        const newCard: LiveAssistCard = { ...card, status: "generating" };
        const next: LiveAssistCard[] = [newCard, ...existing].slice(
          0,
          MAX_CARDS_PER_SESSION,
        );
        return {
          cardsBySession: { ...state.cardsBySession, [sessionId]: next },
        };
      }),
    resolveCard: (sessionId, cardId, result) =>
      set((state) => {
        const existing = state.cardsBySession[sessionId];
        if (!existing) {
          return state;
        }
        return {
          cardsBySession: {
            ...state.cardsBySession,
            [sessionId]: existing.map((card) =>
              card.id === cardId ? { ...card, ...result } : card,
            ),
          },
        };
      }),
    removeCard: (sessionId, cardId) =>
      set((state) => {
        const existing = state.cardsBySession[sessionId];
        if (!existing) {
          return state;
        }
        return {
          cardsBySession: {
            ...state.cardsBySession,
            [sessionId]: existing.filter((card) => card.id !== cardId),
          },
        };
      }),
    clearSession: (sessionId) =>
      set((state) => {
        if (!(sessionId in state.cardsBySession)) {
          return state;
        }
        const { [sessionId]: _removed, ...rest } = state.cardsBySession;
        return { cardsBySession: rest };
      }),
  }),
);

export function useLiveAssistCards(sessionId: string): LiveAssistCard[] {
  return useLiveAssistStore(
    (state) => state.cardsBySession[sessionId] ?? EMPTY_CARDS,
  );
}
