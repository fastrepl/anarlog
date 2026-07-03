import type { SearchFilters, SearchHit } from "~/search/contexts/engine/types";
import type * as main from "~/store/tinybase/store/main";
import type * as settings from "~/store/tinybase/store/settings";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
type Indexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;
type SettingsStore = NonNullable<ReturnType<typeof settings.UI.useStore>>;

export type ContactSearchResult = {
  id: string;
  name: string;
  email: string | null;
  phone: string | null;
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

export type WebSearchResult = {
  title: string;
  url: string;
  snippet: string;
  publishedDate?: string | null;
  author?: string | null;
};

export type WebSearchResponse = {
  status: "ok" | "error";
  message?: string;
  query: string;
  results: WebSearchResult[];
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
  getStore: () => Store | undefined;
  getSettingsStore: () => SettingsStore | undefined;
  getIndexes: () => Indexes | undefined;
  getSessionId: () => string | undefined;
  getEnhancedNoteId: () => string | undefined;
  openEditTab: (requestId: string) => void;
  getAuthHeaders: () => Record<string, string> | null | undefined;
  fetch?: typeof fetch;
}
