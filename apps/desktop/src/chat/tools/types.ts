import type { SearchFilters, SearchHit } from "~/search/contexts/engine/types";
import type * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
type Indexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;

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
  getStore: () => Store | undefined;
  getIndexes: () => Indexes | undefined;
  getSessionId: () => string | undefined;
  getEnhancedNoteId: () => string | undefined;
  openEditTab: (requestId: string) => void;
}
