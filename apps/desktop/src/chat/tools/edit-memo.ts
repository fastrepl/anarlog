import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import {
  applySessionProposal,
  declineSessionProposal,
  persistChatSessionProposal,
} from "~/session/queries";

export const buildEditMemoTool = (
  deps: Pick<ToolDependencies, "getSessionId" | "openEditTab">,
) =>
  tool({
    description:
      "Propose a complete replacement for a session memo and open a diff review where the user can apply or cancel it. Use this to create or revise meeting preparation, agendas, talking points, and notes before or during a meeting. Preserve relevant existing memo content in the replacement markdown.",
    inputSchema: z.object({
      sessionId: z
        .string()
        .optional()
        .describe("The session ID to edit. Defaults to the current session."),
      content: z
        .string()
        .describe("The complete replacement memo in markdown format"),
    }),
    execute: async (
      params: { sessionId?: string; content: string },
      { toolCallId },
    ) => {
      const sessionId = params.sessionId ?? deps.getSessionId();

      if (!sessionId) {
        return {
          status: "error",
          message:
            "No active session selected. Provide sessionId explicitly when calling edit_memo.",
        };
      }

      const snapshot = await loadSessionContentSnapshot(sessionId);
      if (!snapshot) {
        return { status: "error", message: "Session not found." };
      }

      try {
        await persistChatSessionProposal({
          id: toolCallId,
          sessionId,
          kind: "memo_replace",
          targetId: snapshot.rawNoteId || sessionId,
          currentMarkdown: snapshot.rawMarkdown,
          proposedMarkdown: params.content,
        });
      } catch {
        return {
          status: "error",
          message: "Failed to save the proposed memo edit.",
        };
      }

      const approved = await new Promise<boolean>((resolve) => {
        usePendingEditStore.getState().addEdit({
          requestId: toolCallId,
          sessionId,
          target: { kind: "memo" },
          currentContent: snapshot.rawMarkdown,
          proposedContent: params.content,
          source: "chat",
          resolve,
        });
        deps.openEditTab(toolCallId);
      });

      if (!approved) {
        await declineSessionProposal(toolCallId);
        return { status: "declined" };
      }

      try {
        await applySessionProposal(toolCallId);
      } catch {
        return {
          status: "error",
          message: "Failed to apply the memo edit.",
        };
      }

      return { status: "applied" };
    },
  });
