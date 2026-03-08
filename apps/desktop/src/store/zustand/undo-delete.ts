import { create } from "zustand";

type SessionRow = {
  id: string;
  user_id: string;
  created_at: string;
  folder_id: string;
  event_json: string;
  title: string;
  raw_md: string;
};

type TranscriptRow = {
  id: string;
  user_id: string;
  created_at: string;
  session_id: string;
  started_at: number;
  ended_at: number;
  words: string;
  speaker_hints: string;
  memo_md: string;
};

type ParticipantRow = {
  id: string;
  user_id: string;
  session_id: string;
  human_id: string;
  source: string;
};

type TagSessionRow = {
  id: string;
  user_id: string;
  tag_id: string;
  session_id: string;
};

type EnhancedNoteRow = {
  id: string;
  user_id: string;
  session_id: string;
  content: string;
  template_id: string;
  position: number;
  title: string;
};

export type DeletedSessionData = {
  session: SessionRow;
  transcripts: TranscriptRow[];
  participants: ParticipantRow[];
  tagSessions: TagSessionRow[];
  enhancedNotes: EnhancedNoteRow[];
  deletedAt: number;
};

export const UNDO_TIMEOUT_MS = 5000;

export type PendingDeletion = {
  data: DeletedSessionData;
  timeoutId: ReturnType<typeof setTimeout> | null;
  onDeleteConfirm: (() => void) | null;
  addedAt: number;
  batchId: string | null;
  paused: boolean;
  pausedAt: number | null;
};

interface UndoDeleteState {
  pendingDeletions: Record<string, PendingDeletion>;
  addDeletion: (
    data: DeletedSessionData,
    onConfirm?: () => void,
    batchId?: string,
  ) => void;
  clearDeletion: (sessionId: string) => void;
  confirmDeletion: (sessionId: string) => void;
  clearBatch: (batchId: string) => void;
  confirmBatch: (batchId: string) => void;
  pauseSession: (sessionId: string) => void;
  resumeSession: (sessionId: string) => void;
  pauseGroup: (sessionIds: string[]) => void;
  resumeGroup: (sessionIds: string[]) => void;
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
          paused: false,
          pausedAt: null,
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

    if (pending.onDeleteConfirm) {
      pending.onDeleteConfirm();
    }
    get().clearDeletion(sessionId);
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

  pauseSession: (sessionId) => {
    const pending = get().pendingDeletions[sessionId];
    if (!pending || pending.paused) return;

    if (pending.timeoutId) {
      clearTimeout(pending.timeoutId);
    }

    set((state) => {
      const current = state.pendingDeletions[sessionId];
      if (!current) return state;
      return {
        pendingDeletions: {
          ...state.pendingDeletions,
          [sessionId]: {
            ...current,
            timeoutId: null,
            paused: true,
            pausedAt: Date.now(),
          },
        },
      };
    });
  },

  resumeSession: (sessionId) => {
    const pending = get().pendingDeletions[sessionId];
    if (!pending || !pending.paused || !pending.pausedAt) return;

    const pauseDuration = Date.now() - pending.pausedAt;
    const newDeletedAt = pending.data.deletedAt + pauseDuration;
    const elapsed = Date.now() - newDeletedAt;
    const remaining = Math.max(0, UNDO_TIMEOUT_MS - elapsed);

    const timeoutId = setTimeout(() => {
      get().confirmDeletion(sessionId);
    }, remaining);

    set((state) => {
      const current = state.pendingDeletions[sessionId];
      if (!current) return state;
      return {
        pendingDeletions: {
          ...state.pendingDeletions,
          [sessionId]: {
            ...current,
            timeoutId,
            paused: false,
            pausedAt: null,
            data: { ...current.data, deletedAt: newDeletedAt },
          },
        },
      };
    });
  },

  pauseGroup: (sessionIds) => {
    for (const id of sessionIds) {
      get().pauseSession(id);
    }
  },

  resumeGroup: (sessionIds) => {
    for (const id of sessionIds) {
      get().resumeSession(id);
    }
  },
}));
