import { parseJsonContent } from "@hypr/editor/markdown";

import type { TaskConfig } from ".";

import { ensureFirstLineTitle } from "~/session/title-content";
import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

const onSuccess: NonNullable<TaskConfig<"title">["onSuccess"]> = ({
  text,
  args,
  store,
}) => {
  if (args.skipPersist) {
    return;
  }

  persistGeneratedTitle({
    text,
    args,
    store,
  });
};

export function persistGeneratedTitle({
  text,
  args,
  store,
}: {
  text: string;
  args: { sessionId: string };
  store: Parameters<NonNullable<TaskConfig<"title">["onSuccess"]>>[0]["store"];
}) {
  if (!text) {
    return;
  }

  const trimmed = getPersistableGeneratedTitle(text);
  if (!trimmed) {
    return;
  }

  const currentTitle = store.getCell("sessions", args.sessionId, "title");
  if (typeof currentTitle === "string" && currentTitle.trim()) {
    return;
  }

  if (hasLiveSessionTitleDraft(args.sessionId)) {
    return;
  }

  const row: { title: string; raw_md?: string } = { title: trimmed };
  const rawMd = store.getCell("sessions", args.sessionId, "raw_md");
  if (typeof rawMd === "string" && rawMd.trim()) {
    row.raw_md = JSON.stringify(
      ensureFirstLineTitle(parseJsonContent(rawMd), trimmed),
    );
  }

  store.setPartialRow("sessions", args.sessionId, row);
  store.forEachRow("enhanced_notes", (enhancedNoteId, _forEachCell) => {
    const sessionId = store.getCell(
      "enhanced_notes",
      enhancedNoteId,
      "session_id",
    );
    if (sessionId !== args.sessionId) {
      return;
    }

    const content = store.getCell("enhanced_notes", enhancedNoteId, "content");
    if (typeof content !== "string" || !content.trim()) {
      return;
    }

    store.setPartialRow("enhanced_notes", enhancedNoteId, {
      content: JSON.stringify(
        ensureFirstLineTitle(parseJsonContent(content), trimmed),
      ),
    });
  });
}

export function getPersistableGeneratedTitle(text: string): string {
  const trimmed = text.trim();
  return trimmed && trimmed !== "<EMPTY>" ? trimmed : "";
}

export const titleSuccess: Pick<TaskConfig<"title">, "onSuccess"> = {
  onSuccess,
};
