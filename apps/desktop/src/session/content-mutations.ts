import { commands as templateCommands } from "@anlg/plugin-template";

import { enqueueDatabaseWrite } from "~/db/write-queue";

export type SummaryContentCorrection = {
  id: string;
  currentContent: string;
  currentContentFormat: string;
  nextContent: string;
};

export type TranscriptContentCorrection = {
  id: string;
  currentWordsJson: string;
  currentMemo: string;
  nextWordsJson: string;
  nextMemo: string;
};

export type SessionDocumentContentUpdate = {
  id: string;
  currentContent: string;
  currentContentFormat: string;
  nextContent: string;
};

export type SessionTitleCorrection = {
  currentTitle: string;
  nextTitle: string;
};

export function applySessionContentCorrections({
  sessionId,
  summaries,
  transcripts,
  title,
}: {
  sessionId: string;
  summaries: SummaryContentCorrection[];
  transcripts: TranscriptContentCorrection[];
  title?: SessionTitleCorrection;
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await templateCommands.applySessionContentCorrections({
      session_id: sessionId,
      summaries: summaries.map((summary) => ({
        id: summary.id,
        current_body: summary.currentContent,
        current_body_format: summary.currentContentFormat,
        next_body: summary.nextContent,
      })),
      transcripts: transcripts.map((transcript) => ({
        id: transcript.id,
        current_words_json: transcript.currentWordsJson,
        current_memo: transcript.currentMemo,
        next_words_json: transcript.nextWordsJson,
        next_memo: transcript.nextMemo,
      })),
      title: title
        ? {
            current_title: title.currentTitle,
            next_title: title.nextTitle,
          }
        : null,
    });
    if (result.status === "error") throw new Error(result.error);
  });
}

export function persistGeneratedEnhancedNote({
  sessionId,
  ownerUserId,
  note,
  tagNames,
  pendingAutoEnhance,
}: {
  sessionId: string;
  ownerUserId: string;
  note: SessionDocumentContentUpdate;
  tagNames: string[];
  pendingAutoEnhance?: {
    generation: string;
    expectedBody: string;
    expectedContentFormat: string;
  };
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await templateCommands.saveGeneratedSummary({
      session_id: sessionId,
      owner_user_id: ownerUserId,
      note_id: note.id,
      current_body: note.currentContent,
      current_body_format: note.currentContentFormat,
      next_body: note.nextContent,
      tag_names: tagNames,
      pending_auto_enhance: pendingAutoEnhance
        ? {
            generation: pendingAutoEnhance.generation,
            expected_body: pendingAutoEnhance.expectedBody,
            expected_body_format: pendingAutoEnhance.expectedContentFormat,
          }
        : null,
    });
    if (result.status === "error") throw new Error(result.error);
  });
}

export function applyGeneratedSessionTitle({
  sessionId,
  currentTitle,
  nextTitle,
  documents,
}: {
  sessionId: string;
  currentTitle: string;
  nextTitle: string;
  documents: SessionDocumentContentUpdate[];
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await templateCommands.saveGeneratedTitle({
      session_id: sessionId,
      current_title: currentTitle,
      next_title: nextTitle,
      documents: documents.map((document) => ({
        id: document.id,
        current_body: document.currentContent,
        current_body_format: document.currentContentFormat,
        next_body: document.nextContent,
      })),
    });
    if (result.status === "error") throw new Error(result.error);
  });
}
