import { tool } from "ai";
import { z } from "zod";

import {
  getMeeting,
  getMeetingTranscript,
  getRecurringMeetingHistory,
  listMeetings,
} from "@hypr/plugin-db";

const listLimitSchema = z
  .number()
  .int()
  .min(1)
  .max(200)
  .optional()
  .describe("Maximum results; defaults to 20 and is capped at 200");

const offsetSchema = z
  .number()
  .int()
  .min(0)
  .optional()
  .describe("Number of results to skip; defaults to 0");

const historyLimitSchema = z
  .number()
  .int()
  .min(1)
  .max(200)
  .optional()
  .describe("Maximum meetings; defaults to 20 and is capped at 200");

const historyOffsetSchema = z
  .number()
  .int()
  .min(0)
  .optional()
  .describe("Number of meetings to skip; defaults to 0");

export const buildListMeetingsTool = () =>
  tool({
    description:
      "List recent Anarlog meetings with pagination metadata. Use query to narrow by title or meeting id, then pass next_offset as offset to continue.",
    inputSchema: z.object({
      query: z
        .string()
        .optional()
        .describe("Case-insensitive title or meeting id substring"),
      series_id: z.string().optional().describe("Exact recurring series id"),
      limit: listLimitSchema,
      offset: offsetSchema,
    }),
    execute: listMeetings,
  });

export const buildGetMeetingTool = () =>
  tool({
    description:
      "Get one Anarlog meeting with its canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for transcript words.",
    inputSchema: z.object({
      meeting_id: z.string().describe("Anarlog meeting id"),
    }),
    execute: getMeeting,
  });

export const buildGetMeetingTranscriptTool = () =>
  tool({
    description:
      "Get a bounded page of transcript words and readable text for an Anarlog meeting. Pass pagination.next_offset as offset to continue.",
    inputSchema: z.object({
      meeting_id: z.string().describe("Anarlog meeting id"),
      offset: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe("Word offset; defaults to 0"),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .describe("Maximum words; defaults to 200 and is capped at 500"),
    }),
    execute: getMeetingTranscript,
  });

export const buildGetRecurringMeetingHistoryTool = () =>
  tool({
    description:
      "List meetings in the same recurring series as the supplied meeting, newest first, with pagination metadata.",
    inputSchema: z.object({
      meeting_id: z
        .string()
        .describe("A meeting id used to resolve its recurring series"),
      limit: historyLimitSchema,
      offset: historyOffsetSchema,
    }),
    execute: getRecurringMeetingHistory,
  });
