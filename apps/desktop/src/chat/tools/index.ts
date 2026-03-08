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

export const buildChatTools = (deps: ToolDependencies) => ({
  search_sessions: buildSearchSessionsTool(deps),
  search_contacts: buildSearchContactsTool(deps),
  search_calendar_events: buildSearchCalendarEventsTool(deps),
});

type LocalTools = {
  search_sessions: {
    input: { query: string; filters?: SearchFilters; limit?: number };
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
