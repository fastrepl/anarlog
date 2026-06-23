import { parseJsonContent } from "@hypr/editor/markdown";

import type { TaskConfig } from ".";

import { ensureFirstLineTitle } from "~/session/title-content";
import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

const onSuccess: NonNullable<TaskConfig<"title">["onSuccess"]> = ({
  text,
  args,
  store,
}) => {
  if (!text) {
    return;
  }

  const trimmed = text.trim();
  if (!trimmed || trimmed === "<EMPTY>") {
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
};

export const titleSuccess: Pick<TaskConfig<"title">, "onSuccess"> = {
  onSuccess,
};
