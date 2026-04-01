import { buildEditSummaryTool } from "./edit-summary";
import { buildSearchCalendarEventsTool } from "./search-calendar-events";
import { buildSearchContactsTool } from "./search-contacts";
import { buildSearchSessionsTool } from "./search-sessions";
import type {
  CalendarEventSearchResult,
  ContactSearchResult,
  ToolDependencies,
} from "./types";

import type { SupportMcpTools } from "~/chat/mcp/support-mcp-tools";
import type { SearchFilters } from "~/search/contexts/engine/types";

export type { ToolDependencies };

// Ephemeral field: injected by transport during hydration, stripped before persistence.
export const CONTEXT_TEXT_FIELD = "contextText" as const;

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

export type Tools = LocalTools & SupportMcpTools;

export type ToolPartType = `tool-${keyof Tools}`;
