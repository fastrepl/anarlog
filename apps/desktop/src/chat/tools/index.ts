import { CONTEXT_TEXT_FIELD } from "./context-text";
import { buildEditSummaryTool } from "./edit-summary";
import {
  buildGrepNotesTool,
  buildListRelatedNotesTool,
  buildReadCurrentNoteTool,
  buildReadNoteTool,
} from "./note-files";
import { buildSearchCalendarEventsTool } from "./search-calendar-events";
import { buildSearchContactsTool } from "./search-contacts";
import { buildSearchSessionsTool } from "./search-sessions";
import type {
  CalendarEventSearchResult,
  ContactSearchResult,
  ToolDependencies,
} from "./types";

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
      console.log(`[chat/tool:start] ${name}`, ...args);

      try {
        const result = await toolDef.execute!(...args);
        console.log(`[chat/tool:result] ${name}`, result);
        return result;
      } catch (error) {
        console.error(`[chat/tool:error] ${name}`, error);
        throw error;
      }
    },
  } as T;
}

export const buildChatTools = (deps: ToolDependencies) => ({
  read_current_note: withToolLogging(
    "read_current_note",
    buildReadCurrentNoteTool(deps),
  ),
  read_note: withToolLogging("read_note", buildReadNoteTool(deps)),
  grep_notes: withToolLogging("grep_notes", buildGrepNotesTool(deps)),
  list_related_notes: withToolLogging(
    "list_related_notes",
    buildListRelatedNotesTool(deps),
  ),
  search_sessions: withToolLogging(
    "search_sessions",
    buildSearchSessionsTool(deps),
  ),
  search_contacts: withToolLogging(
    "search_contacts",
    buildSearchContactsTool(deps),
  ),
  search_calendar_events: withToolLogging(
    "search_calendar_events",
    buildSearchCalendarEventsTool(deps),
  ),
  edit_summary: withToolLogging("edit_summary", buildEditSummaryTool(deps)),
});

type LocalTools = {
  read_current_note: {
    input: { maxChars?: number };
    output: {
      status: "ok" | "error";
      message?: string;
      sessionId?: string;
      title?: string;
      date?: string | null;
      event?: string | null;
      participants?: string[];
      sections?: Array<{ title: string; characters: number }>;
      truncated?: boolean;
      contextText?: string | null;
    };
  };
  read_note: {
    input: { sessionId: string; maxChars?: number };
    output: {
      status: "ok" | "error";
      message?: string;
      sessionId: string;
      title?: string;
      date?: string | null;
      event?: string | null;
      participants?: string[];
      sections?: Array<{ title: string; characters: number }>;
      truncated?: boolean;
      contextText?: string | null;
    };
  };
  grep_notes: {
    input: { query: string; sessionIds?: string[]; limit?: number };
    output: {
      query: string;
      scanned?: number;
      message?: string;
      results: Array<{
        sessionId: string;
        title: string;
        date: string | null;
        score: number;
        snippets: Array<{ section: string; text: string }>;
      }>;
    };
  };
  list_related_notes: {
    input: { sessionId?: string; limit?: number };
    output: {
      status: "ok" | "error";
      message?: string;
      sessionId?: string;
      title?: string;
      results: Array<{
        sessionId: string;
        title: string;
        date: string | null;
        score: number;
        reasons: string[];
      }>;
    };
  };
  search_sessions: {
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
};

export type Tools = LocalTools;

export type ToolPartType = `tool-${keyof Tools}`;
