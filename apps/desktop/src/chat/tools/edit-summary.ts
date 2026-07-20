import { tool } from "ai";
import { z } from "zod";

import { md2json } from "@hypr/editor/markdown";

import type { ToolDependencies } from "./types";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import { updateEnhancedNoteContent } from "~/session/queries";
import { id } from "~/shared/utils";

type SummaryCandidate = {
  enhancedNoteId: string;
  title: string;
  templateId?: string;
  position?: number;
};

function listSummaryCandidates(
  notes: NonNullable<
    Awaited<ReturnType<typeof loadSessionContentSnapshot>>
  >["enhancedNotes"],
): SummaryCandidate[] {
  return notes.map((note) => ({
    enhancedNoteId: note.id,
    title: note.title.trim() || "Summary",
    templateId: note.templateId || undefined,
    position: note.position,
  }));
}

export const buildEditSummaryTool = (
  deps: Pick<
    ToolDependencies,
    "getSessionId" | "getEnhancedNoteId" | "openEditTab"
  >,
) =>
  tool({
    description:
      "Propose a complete replacement for an existing session summary and open a diff review where the user can apply or cancel it. Use this for broad rewrites such as refocusing, shortening, or restructuring a summary. The content must be the full replacement markdown, not instructions or a partial patch.",
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
        .describe("The complete replacement summary in markdown format"),
    }),
    execute: async (params: {
      sessionId?: string;
      enhancedNoteId?: string;
      content: string;
    }) => {
      const activeSessionId = deps.getSessionId();
      const sessionId = params.sessionId ?? activeSessionId;

      if (!sessionId) {
        return {
          status: "error",
          message:
            "No active session selected. Provide sessionId explicitly when calling edit_summary.",
        };
      }

      const snapshot = await loadSessionContentSnapshot(sessionId);
      const notes = snapshot?.enhancedNotes ?? [];
      const noteIds = notes.map((note) => note.id);

      if (noteIds.length === 0) {
        return {
          status: "error",
          message: "No summaries found for this session",
        };
      }

      const noteIdSet = new Set(noteIds);

      const requestedEnhancedNoteId = params.enhancedNoteId;
      const activeEnhancedNoteId = deps.getEnhancedNoteId();
      const candidates = listSummaryCandidates(notes);

      if (requestedEnhancedNoteId && !noteIdSet.has(requestedEnhancedNoteId)) {
        return {
          status: "error",
          message: "That summary does not belong to the target session.",
          candidates,
        };
      }

      const defaultEnhancedNoteId =
        notes.find((note) => !note.templateId)?.id ?? null;

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

      const currentContent =
        notes.find((note) => note.id === enhancedNoteId)?.markdown ?? "";

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
        await updateEnhancedNoteContent(
          enhancedNoteId,
          sessionId,
          JSON.stringify(json),
        );
      } catch {
        return {
          status: "error",
          message: "Failed to apply the summary edit.",
        };
      }

      return { status: "applied" };
    },
  });
