import { md2json, parseJsonContent } from "@anlg/editor/markdown";

import { applyGeneratedSessionTitle } from "./content-mutations";
import { loadSessionContentSnapshot } from "./content-queries";
import { extractFirstLineTitle } from "./title-content";

import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

export async function repairMissingSessionTitle(
  sessionId: string,
  enhancedNoteId: string,
) {
  for (let attempt = 0; attempt < 2; attempt++) {
    if (hasLiveSessionTitleDraft(sessionId)) return;

    const snapshot = await loadSessionContentSnapshot(sessionId, {
      includeTranscriptWords: false,
    });
    if (!snapshot || snapshot.title.trim()) return;

    const note = snapshot.enhancedNotes.find(
      (note) => note.id === enhancedNoteId,
    );
    if (!note?.content.trim()) return;

    const content =
      note.contentFormat === "markdown"
        ? md2json(note.content)
        : parseJsonContent(note.content);
    const firstBlock = content.content?.[0];
    if (firstBlock?.type !== "heading" || firstBlock.attrs?.level !== 1) return;

    const title = extractFirstLineTitle(content);
    const event = snapshot.event;
    // Legacy summaries also use H1 for sections; only a calendar match proves
    // this heading is the missing meeting title.
    if (
      !title ||
      !event ||
      typeof event !== "object" ||
      !("title" in event) ||
      typeof event.title !== "string" ||
      event.title.trim() !== title ||
      hasLiveSessionTitleDraft(sessionId)
    )
      return;

    try {
      await applyGeneratedSessionTitle({
        sessionId,
        currentTitle: snapshot.title,
        nextTitle: title,
        documents: [
          {
            id: note.id,
            currentContent: note.content,
            currentContentFormat: note.contentFormat,
            nextContent:
              note.contentFormat === "markdown"
                ? JSON.stringify(content)
                : note.content,
          },
        ],
      });
      return;
    } catch (error) {
      if (
        attempt === 1 ||
        !(error instanceof Error) ||
        !/^transaction statement \d+ affected 0 rows; expected 1$/.test(
          error.message,
        )
      ) {
        throw error;
      }
    }
  }
}
