import type { SearchFilters, SearchHit } from "~/search/contexts/engine/types";
import type { SummaryEditCandidate } from "~/session/hooks/enhanced-notes";

export type ContactSearchResult = {
  id: string;
  name: string;
  email: string | null;
  jobTitle: string | null;
  organization: string | null;
  memo: string | null;
};

export type CalendarEventSearchResult = {
  id: string;
  title: string;
  startedAt: string | null;
  endedAt: string | null;
  location: string | null;
  meetingLink: string | null;
  description: string | null;
  participantCount: number;
  linkedSessionId: string | null;
};

export interface ToolDependencies {
  search: (
    query: string,
    filters?: SearchFilters | null,
  ) => Promise<SearchHit[]>;
  getContactSearchResults: (
    query: string,
    limit: number,
  ) => Promise<ContactSearchResult[]>;
  getCalendarEventSearchResults: (
    query: string,
    limit: number,
  ) => Promise<CalendarEventSearchResult[]>;
  getSessionSearchTimestamp: (sessionId: string) => number | undefined;
  getSummaryCandidates: (sessionId: string) => SummaryEditCandidate[];
  getSummaryMarkdown: (enhancedNoteId: string) => string;
  applySummaryMarkdown: (enhancedNoteId: string, markdown: string) => void;
  getSessionId: () => string | undefined;
  getEnhancedNoteId: () => string | undefined;
  openEditTab: (requestId: string) => void;
}
