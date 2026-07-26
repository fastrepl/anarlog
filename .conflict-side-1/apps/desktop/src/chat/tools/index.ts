import type {
  GetMeetingInput,
  GetMeetingTranscriptInput,
  GetRecurringMeetingHistoryInput,
  ListMeetingsInput,
  Meeting,
  MeetingPage,
  TranscriptPage,
} from "@hypr/plugin-db";

import { CONTEXT_TEXT_FIELD } from "./context-text";
import { buildEditSummaryTool } from "./edit-summary";
import {
  buildGetMeetingTool,
  buildGetMeetingTranscriptTool,
  buildGetRecurringMeetingHistoryTool,
  buildListMeetingsTool,
} from "./meetings";
import {
  buildFindRelatedMeetingsTool,
  buildSearchMeetingContentTool,
} from "./note-files";
import { buildSearchCalendarEventsTool } from "./search-calendar-events";
import { buildSearchContactsTool } from "./search-contacts";
import { buildSearchMeetingsTool } from "./search-meetings";
import { buildApplySessionCorrectionTool } from "./session-correction";
import type {
  CalendarEventSearchResult,
  ContactSearchResult,
  WebSearchResponse,
  ToolDependencies,
} from "./types";
import { buildWebSearchTool } from "./web-search";

import type { SearchFilters } from "~/search/contexts/engine/types";

export type { ToolDependencies };
export { CONTEXT_TEXT_FIELD };

function withToolLogging<T extends { execute?: (...args: any[]) => any }>(
  name: string,
  toolDef: T,
): T {
  if (typeof toolDef.execute !== "function") {
    return toolDef;
  }

  return {
    ...toolDef,
    execute: async (...args: Parameters<NonNullable<T["execute"]>>) => {
      if (import.meta.env.DEV) {
        console.log(`[chat/tool:start] ${name}`);
      }

      try {
        const result = await toolDef.execute!(...args);
        if (import.meta.env.DEV) {
          console.log(`[chat/tool:result] ${name}`);
        }
        return result;
      } catch (error) {
        if (import.meta.env.DEV) {
          console.error(`[chat/tool:error] ${name}`);
        }
        throw error;
      }
    },
  } as T;
}

export const buildChatTools = (deps: ToolDependencies) => ({
  list_meetings: withToolLogging("list_meetings", buildListMeetingsTool()),
  get_meeting: withToolLogging("get_meeting", buildGetMeetingTool()),
  get_meeting_transcript: withToolLogging(
    "get_meeting_transcript",
    buildGetMeetingTranscriptTool(),
  ),
  get_recurring_meeting_history: withToolLogging(
    "get_recurring_meeting_history",
    buildGetRecurringMeetingHistoryTool(),
  ),
  search_meeting_content: withToolLogging(
    "search_meeting_content",
    buildSearchMeetingContentTool(deps),
  ),
  find_related_meetings: withToolLogging(
    "find_related_meetings",
    buildFindRelatedMeetingsTool(deps),
  ),
  search_meetings: withToolLogging(
    "search_meetings",
    buildSearchMeetingsTool(deps),
  ),
  search_contacts: withToolLogging(
    "search_contacts",
    buildSearchContactsTool(deps),
  ),
  search_calendar_events: withToolLogging(
    "search_calendar_events",
    buildSearchCalendarEventsTool(deps),
  ),
  web_search: withToolLogging("web_search", buildWebSearchTool(deps)),
  edit_summary: withToolLogging("edit_summary", buildEditSummaryTool(deps)),
  apply_session_correction: withToolLogging(
    "apply_session_correction",
    buildApplySessionCorrectionTool(deps),
  ),
});

type LocalTools = {
  list_meetings: {
    input: ListMeetingsInput;
    output: MeetingPage;
  };
  get_meeting: {
    input: GetMeetingInput;
    output: Meeting;
  };
  get_meeting_transcript: {
    input: GetMeetingTranscriptInput;
    output: TranscriptPage;
  };
  get_recurring_meeting_history: {
    input: GetRecurringMeetingHistoryInput;
    output: MeetingPage;
  };
  search_meeting_content: {
    input: { query: string; meeting_ids?: string[]; limit?: number };
    output: {
      query: string;
      scanned?: number;
      message?: string;
      results: Array<{
        meeting_id: string;
        title: string;
        date: string | null;
        score: number;
        snippets: Array<{ section: string; text: string }>;
      }>;
    };
  };
  find_related_meetings: {
    input: { meeting_id?: string; limit?: number };
    output: {
      status: "ok" | "error";
      message?: string;
      meeting_id?: string;
      title?: string;
      results: Array<{
        meeting_id: string;
        title: string;
        date: string | null;
        score: number;
        reasons: string[];
      }>;
    };
  };
  search_meetings: {
    input: {
      query?: string;
      filters?: {
        created_at?:
          | ({
              kind: "absolute";
            } & NonNullable<SearchFilters["created_at"]>)
          | {
              kind: "relative";
              recent_days: number;
            };
      };
      limit?: number;
    };
    output: {
      results: Array<{
        id: string;
        title: string;
        excerpt: string;
        score: number;
        created_at: number;
      }>;
      contextText?: string | null;
    };
  };
  search_contacts: {
    input: { query: string; limit?: number };
    output: {
      query: string;
      results: ContactSearchResult[];
    };
  };
  search_calendar_events: {
    input: { query: string; limit?: number };
    output: {
      query: string;
      results: CalendarEventSearchResult[];
    };
  };
  web_search: {
    input: {
      query: string;
      includeDomains?: string[];
      excludeDomains?: string[];
      limit?: number;
    };
    output: WebSearchResponse;
  };
  edit_summary: {
    input: { sessionId?: string; enhancedNoteId?: string; content: string };
    output: {
      status: string;
      message?: string;
      candidates?: Array<{
        enhancedNoteId: string;
        title: string;
        templateId?: string;
        position?: number;
      }>;
    };
  };
  apply_session_correction: {
    input: {
      sessionId?: string;
      target?: "summary" | "transcript" | "summary_and_transcript";
      enhancedNoteId?: string;
      oldText: string;
      newText: string;
      dictionaryTerms?: string[];
    };
    output: {
      status: string;
      message?: string;
      sessionId?: string;
      summaryChanges?: Array<{
        enhancedNoteId: string;
        title: string;
        replacements: number;
      }>;
      transcriptChanges?: Array<{
        transcriptId: string;
        wordReplacements: number;
        memoReplacements: number;
      }>;
      dictionaryChanges?: {
        addedTerms: string[];
      };
    };
  };
};

export type Tools = LocalTools;

export type ToolPartType = `tool-${keyof Tools}`;
