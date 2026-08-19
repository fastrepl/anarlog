import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

import { moveSessionContents } from "~/session/move-contents";

export const buildMoveMeetingContentsTool = (
  deps: Pick<ToolDependencies, "getSessionId">,
) =>
  tool({
    description:
      "Move a finished recording, transcript, generated summaries, notes, and action items from one meeting onto another existing meeting. Use this when the user says a recording or notes landed on the wrong meeting. Resolve both meeting IDs with list_meetings or search_meetings first and never guess IDs. The target meeting must not already have a recording or transcript.",
    inputSchema: z.object({
      sourceMeetingId: z
        .string()
        .optional()
        .describe(
          "Meeting that currently has the recording or notes. Defaults to the current meeting.",
        ),
      targetMeetingId: z
        .string()
        .describe(
          "Existing meeting that should receive the recording and notes.",
        ),
    }),
    execute: async (params: {
      sourceMeetingId?: string;
      targetMeetingId: string;
    }) => {
      const sourceMeetingId = params.sourceMeetingId ?? deps.getSessionId();
      const targetMeetingId = params.targetMeetingId;

      if (!sourceMeetingId) {
        return {
          status: "error",
          message:
            "No source meeting selected. Provide sourceMeetingId explicitly when calling move_meeting_contents.",
        };
      }

      return moveSessionContents({
        sourceSessionId: sourceMeetingId,
        targetSessionId: targetMeetingId,
      });
    },
  });
