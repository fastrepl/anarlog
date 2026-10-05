import type { SessionDetail } from "../data/session-detail.ts";

export type NoteDraft = {
  title?: string;
  body?: string;
  bodyFormat?: SessionDetail["bodyFormat"];
};

export function createNoteDraft({
  getNote,
  saveNote,
  saveTitle,
  onError,
}: {
  getNote: () => SessionDetail | null;
  saveNote: (note: {
    title: string;
    bodyText: string;
    bodyFormat: SessionDetail["bodyFormat"];
  }) => Promise<void>;
  saveTitle: (title: string) => Promise<void>;
  onError: (error: unknown, draft: NoteDraft) => void;
}) {
  let pending: NoteDraft = {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  let savedTitle: string | null = null;
  let inFlight: Promise<void> | undefined;
  let generation = 0;

  function cancelTimer() {
    clearTimeout(timer);
    timer = undefined;
  }

  function scheduleSave() {
    cancelTimer();
    timer = setTimeout(flush, 500);
  }

  async function waitForSave(throwOnError = false) {
    while (inFlight) {
      try {
        await inFlight;
      } catch (error) {
        if (throwOnError) throw error;
      }
    }
  }

  function discard() {
    generation++;
    cancelTimer();
    pending = {};
  }

  async function flush(throwOnError = false) {
    cancelTimer();
    if (inFlight) await waitForSave(throwOnError);
    const note = getNote();
    if (!note) return;
    const draft = pending;
    pending = {};
    const currentGeneration = generation;
    let write: Promise<void>;
    if (draft.body !== undefined) {
      // Live-query results can lag our writes; a body save must keep the latest title.
      const title = draft.title ?? savedTitle ?? note.title;
      savedTitle = title;
      write = saveNote({
        title,
        bodyText: draft.body,
        bodyFormat: draft.bodyFormat ?? note.bodyFormat,
      });
    } else if (draft.title !== undefined) {
      savedTitle = draft.title;
      write = saveTitle(draft.title);
    } else {
      return;
    }
    inFlight = write;
    try {
      await write;
    } catch (error) {
      if (generation === currentGeneration) {
        pending = { ...draft, ...pending };
        onError(error, draft);
      }
      if (throwOnError) throw error;
    } finally {
      if (inFlight === write) inFlight = undefined;
    }
  }

  return {
    edit(patch: NoteDraft) {
      pending = { ...pending, ...patch };
      scheduleSave();
    },
    flush,
    discard,
    async remove(deleteNote: () => Promise<void>) {
      cancelTimer();
      await waitForSave();
      try {
        await deleteNote();
      } catch (error) {
        if (Object.keys(pending).length) scheduleSave();
        throw error;
      }
      discard();
    },
    restore(title: string) {
      generation++;
      cancelTimer();
      pending = {};
      savedTitle = title;
    },
    observeTitle(title: string | undefined) {
      if (title === savedTitle) savedTitle = null;
    },
    snapshot: () => ({ ...pending }),
  };
}
