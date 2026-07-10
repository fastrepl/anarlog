import { md2json, parseJsonContent } from "@hypr/editor/markdown";

import type { TaskConfig } from ".";

import {
  applyGeneratedSessionTitle,
  type SessionDocumentContentUpdate,
} from "~/session/content-mutations";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import { ensureFirstLineTitle } from "~/session/title-content";
import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

const onSuccess: NonNullable<TaskConfig<"title">["onSuccess"]> = async ({
  text,
  args,
}) => {
  if (args.skipPersist) {
    return;
  }

  await persistGeneratedTitle({
    text,
    args,
  });
};

export async function persistGeneratedTitle({
  text,
  args,
}: {
  text: string;
  args: { sessionId: string };
}): Promise<boolean> {
  if (!text) {
    return false;
  }

  const trimmed = getPersistableGeneratedTitle(text);
  if (!trimmed) {
    return false;
  }

  if (hasLiveSessionTitleDraft(args.sessionId)) {
    return false;
  }

  const snapshot = await loadSessionContentSnapshot(args.sessionId);
  if (!snapshot || snapshot.title.trim()) {
    return false;
  }

  if (hasLiveSessionTitleDraft(args.sessionId)) {
    return false;
  }

  const documents: SessionDocumentContentUpdate[] = [];
  if (snapshot.rawNoteId && snapshot.rawContent.trim()) {
    documents.push(
      createTitledDocumentUpdate(
        snapshot.rawNoteId,
        snapshot.rawContent,
        snapshot.rawContentFormat,
        trimmed,
      ),
    );
  }
  documents.push(
    ...snapshot.enhancedNotes
      .filter((note) => note.content.trim())
      .map((note) =>
        createTitledDocumentUpdate(
          note.id,
          note.content,
          note.contentFormat,
          trimmed,
        ),
      ),
  );

  await applyGeneratedSessionTitle({
    sessionId: args.sessionId,
    currentTitle: snapshot.title,
    nextTitle: trimmed,
    documents,
  });
  return true;
}

function createTitledDocumentUpdate(
  id: string,
  content: string,
  contentFormat: string,
  title: string,
): SessionDocumentContentUpdate {
  const parsed =
    contentFormat === "markdown" ? md2json(content) : parseJsonContent(content);
  return {
    id,
    currentContent: content,
    currentContentFormat: contentFormat,
    nextContent: JSON.stringify(ensureFirstLineTitle(parsed, title)),
  };
}

export function getPersistableGeneratedTitle(text: string): string {
  const trimmed = text.trim();
  return trimmed && trimmed !== "<EMPTY>" ? trimmed : "";
}

export const titleSuccess: Pick<TaskConfig<"title">, "onSuccess"> = {
  onSuccess,
};
