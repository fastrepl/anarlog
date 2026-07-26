import { create } from "zustand";

type DeletedSession = {
  id: string;
  title: string;
};

export type DeletedSessionData = {
  session: DeletedSession;
  tombstone: string;
  deletedAt: number;
};

export const UNDO_TIMEOUT_MS = 5000;

export type PendingDeletion = {
  data: DeletedSessionData;
  timeoutId: ReturnType<typeof setTimeout> | null;
  onDeleteConfirm: (() => void | Promise<unknown>) | null;
  addedAt: number;
  batchId: string | null;
};

interface UndoDeleteState {
  pendingDeletions: Record<string, PendingDeletion>;
  addDeletion: (
    data: DeletedSessionData,
    onConfirm?: () => void | Promise<unknown>,
    batchId?: string,
  ) => void;
  clearDeletion: (sessionId: string) => void;
  confirmDeletion: (sessionId: string) => void | Promise<unknown>;
  clearBatch: (batchId: string) => void;
  confirmBatch: (batchId: string) => void;
}

export const useUndoDelete = create<UndoDeleteState>((set, get) => ({
  pendingDeletions: {},

  addDeletion: (data, onConfirm, batchId) => {
    const sessionId = data.session.id;

    const existing = get().pendingDeletions[sessionId];
    if (existing?.timeoutId) {
      clearTimeout(existing.timeoutId);
    }

    const timeoutId = setTimeout(() => {
      get().confirmDeletion(sessionId);
    }, UNDO_TIMEOUT_MS);

    set((state) => ({
      pendingDeletions: {
        ...state.pendingDeletions,
        [sessionId]: {
          data,
          timeoutId,
          onDeleteConfirm: onConfirm ?? null,
          addedAt: Date.now(),
          batchId: batchId ?? null,
        },
      },
    }));
  },

  clearDeletion: (sessionId) => {
    const pending = get().pendingDeletions[sessionId];
    if (!pending) return;

    if (pending.timeoutId) {
      clearTimeout(pending.timeoutId);
    }

    set((state) => {
      const { [sessionId]: _, ...rest } = state.pendingDeletions;
      return { pendingDeletions: rest };
    });
  },

  confirmDeletion: (sessionId) => {
    const pending = get().pendingDeletions[sessionId];
    if (!pending) return;

    const result = pending.onDeleteConfirm?.();
    get().clearDeletion(sessionId);
    return result;
  },

  clearBatch: (batchId) => {
    const entries = Object.entries(get().pendingDeletions).filter(
      ([_, p]) => p.batchId === batchId,
    );
    for (const [sessionId] of entries) {
      get().clearDeletion(sessionId);
    }
  },

  confirmBatch: (batchId) => {
    const entries = Object.entries(get().pendingDeletions).filter(
      ([_, p]) => p.batchId === batchId,
    );
    for (const [sessionId] of entries) {
      get().confirmDeletion(sessionId);
    }
  },
}));

// App exit must not strand notes soft-deleted with live shared links: confirm
// every pending deletion now and let the caller await the finalize work.
export function confirmAllPendingDeletions(): Promise<void> {
  const { pendingDeletions, confirmDeletion } = useUndoDelete.getState();
  return Promise.allSettled(
    Object.keys(pendingDeletions).map((sessionId) =>
      confirmDeletion(sessionId),
    ),
  ).then(() => undefined);
}
