import { create } from "zustand";

type PendingEdit = {
  requestId: string;
  sessionId: string;
  enhancedNoteId: string;
  currentContent: string;
  proposedContent: string;
  resolve: (approved: boolean) => void;
};

type PendingEditStore = {
  edits: Map<string, PendingEdit>;
  addEdit: (edit: PendingEdit) => void;
  resolveEdit: (requestId: string, approved: boolean) => void;
  removeEdit: (requestId: string) => void;
};

export const usePendingEditStore = create<PendingEditStore>((set, get) => ({
  edits: new Map(),
  addEdit: (edit) => {
    set((state) => {
      const next = new Map(state.edits);
      next.set(edit.requestId, edit);
      return { edits: next };
    });
  },
  resolveEdit: (requestId, approved) => {
    const edit = get().edits.get(requestId);
    if (edit) {
      edit.resolve(approved);
      get().removeEdit(requestId);
    }
  },
  removeEdit: (requestId) => {
    set((state) => {
      const next = new Map(state.edits);
      next.delete(requestId);
      return { edits: next };
    });
  },
}));
