import { tool } from "ai";
import { z } from "zod";

import { json2md, md2json, parseJsonContent } from "@hypr/tiptap/shared";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
type Indexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;

type SummaryCandidate = {
  enhancedNoteId: string;
  title: string;
  templateId?: string;
  position?: number;
};

function listSummaryCandidates(
  store: Store,
  noteIds: string[],
): SummaryCandidate[] {
  return noteIds.map((enhancedNoteId) => {
    const title = store.getCell("enhanced_notes", enhancedNoteId, "title");
    const templateId = store.getCell(
      "enhanced_notes",
      enhancedNoteId,
      "template_id",
    );
    const position = store.getCell(
      "enhanced_notes",
      enhancedNoteId,
      "position",
    );

    return {
      enhancedNoteId,
      title: typeof title === "string" && title.trim() ? title : "Summary",
      templateId:
        typeof templateId === "string" && templateId ? templateId : undefined,
      position: typeof position === "number" ? position : undefined,
    };
  });
}

export const buildEditSummaryTool = (deps: {
  getStore: () => Store | undefined;
  getIndexes: () => Indexes | undefined;
  getSessionId: () => string | undefined;
  getEnhancedNoteId: () => string | undefined;
  openEditTab: (requestId: string) => void;
}) =>
  tool({
    description:
      "Propose an edit to a session summary. This opens a review tab where the user can approve or decline the changes.",
    inputSchema: z.object({
      sessionId: z
        .string()
        .optional()
        .describe("The session ID to edit. Defaults to the current session."),
      enhancedNoteId: z
        .string()
        .optional()
        .describe(
          "The specific summary ID (enhanced note ID) to edit. Defaults to the active summary in the session tab when possible.",
        ),
      content: z
        .string()
        .describe("The proposed summary content in markdown format"),
    }),
    execute: async (params: {
      sessionId?: string;
      enhancedNoteId?: string;
      content: string;
    }) => {
      const store = deps.getStore();
      const indexes = deps.getIndexes();
      const activeSessionId = deps.getSessionId();
      const sessionId = params.sessionId ?? activeSessionId;

      if (!store || !indexes || !sessionId) {
        return {
          status: "error",
          message:
            "No active session selected. Provide sessionId explicitly when calling edit_summary.",
        };
      }

      const noteIds = indexes.getSliceRowIds(
        main.INDEXES.enhancedNotesBySession,
        sessionId,
      );

      if (noteIds.length === 0) {
        return {
          status: "error",
          message: "No summaries found for this session",
        };
      }

      const noteIdSet = new Set(noteIds);

      const requestedEnhancedNoteId = params.enhancedNoteId;
      const activeEnhancedNoteId = deps.getEnhancedNoteId();
      const candidates = listSummaryCandidates(store, noteIds);

      if (requestedEnhancedNoteId && !noteIdSet.has(requestedEnhancedNoteId)) {
        return {
          status: "error",
          message: "That summary does not belong to the target session.",
          candidates,
        };
      }

      const defaultEnhancedNoteId =
        noteIds.find((id) => {
          const templateId = store.getCell(
            "enhanced_notes",
            id,
            "template_id",
          ) as string | undefined;
          return !templateId;
        }) ?? null;

      const enhancedNoteId =
        (requestedEnhancedNoteId && noteIdSet.has(requestedEnhancedNoteId)
          ? requestedEnhancedNoteId
          : null) ??
        (activeEnhancedNoteId && noteIdSet.has(activeEnhancedNoteId)
          ? activeEnhancedNoteId
          : null) ??
        defaultEnhancedNoteId ??
        (noteIds.length === 1 ? noteIds[0] : null);

      if (!enhancedNoteId) {
        return {
          status: "error",
          message:
            "Multiple summaries exist for this session. Specify enhancedNoteId explicitly.",
          candidates,
        };
      }

      const raw = store.getCell("enhanced_notes", enhancedNoteId, "content") as
        | string
        | undefined;
      const currentContent = json2md(parseJsonContent(raw));

      const requestId = id();
      const approved = await new Promise<boolean>((resolve) => {
        usePendingEditStore.getState().addEdit({
          requestId,
          sessionId,
          enhancedNoteId,
          currentContent,
          proposedContent: params.content,
          resolve,
        });
        deps.openEditTab(requestId);
      });

      if (!approved) {
        return { status: "declined" };
      }

      try {
        const json = md2json(params.content);
        store.setPartialRow("enhanced_notes", enhancedNoteId, {
          content: JSON.stringify(json),
        });
      } catch {
        return {
          status: "error",
          message: "Failed to apply the summary edit.",
        };
      }

      return { status: "applied" };
    },
  });
