import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { id } from "~/shared/utils";

export const buildEditSummaryTool = (
  deps: Pick<
    ToolDependencies,
    | "getSummaryCandidates"
    | "getSummaryMarkdown"
    | "applySummaryMarkdown"
    | "getSessionId"
    | "getEnhancedNoteId"
    | "openEditTab"
  >,
) =>
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
      const activeSessionId = deps.getSessionId();
      const sessionId = params.sessionId ?? activeSessionId;

      if (!sessionId) {
        return {
          status: "error",
          message:
            "No active session selected. Provide sessionId explicitly when calling edit_summary.",
        };
      }

      const candidates = deps.getSummaryCandidates(sessionId);
      const noteIds = candidates.map((candidate) => candidate.enhancedNoteId);

      if (noteIds.length === 0) {
        return {
          status: "error",
          message: "No summaries found for this session",
        };
      }

      const noteIdSet = new Set(noteIds);

      const requestedEnhancedNoteId = params.enhancedNoteId;
      const activeEnhancedNoteId = deps.getEnhancedNoteId();

      if (requestedEnhancedNoteId && !noteIdSet.has(requestedEnhancedNoteId)) {
        return {
          status: "error",
          message: "That summary does not belong to the target session.",
          candidates,
        };
      }

      const defaultEnhancedNoteId =
        candidates.find((candidate) => !candidate.templateId)?.enhancedNoteId ??
        null;

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

      const currentContent = deps.getSummaryMarkdown(enhancedNoteId);

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
        deps.applySummaryMarkdown(enhancedNoteId, params.content);
      } catch {
        return {
          status: "error",
          message: "Failed to apply the summary edit.",
        };
      }

      return { status: "applied" };
    },
  });
