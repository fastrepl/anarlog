import { useMemo } from "react";

import type { MentionConfig } from "@hypr/editor/widgets";

import { useTimelineSessionsTable } from "~/calendar/queries";
import { useHumans, useOrganizations } from "~/contacts/queries";
import { useSearchEngine } from "~/search/contexts/engine";

export function useMentionConfig(): MentionConfig {
  const sessions = useTimelineSessionsTable();
  const humans = useHumans();
  const organizations = useOrganizations();
  const { search } = useSearchEngine();

  return useMemo(
    () => ({
      trigger: "@",
      handleSearch: async (query: string) => {
        const results: {
          id: string;
          type: string;
          label: string;
          content?: string;
        }[] = [];

        if (query.trim()) {
          const searchResults = await search(query);
          for (const hit of searchResults) {
            results.push({
              id: hit.document.id,
              type: hit.document.type,
              label: hit.document.title,
            });
          }
        } else {
          Object.entries(sessions ?? {}).forEach(([rowId, row]) => {
            const title = row.title as string | undefined;
            if (title) {
              results.push({ id: rowId, type: "session", label: title });
            }
          });
          humans.forEach((human) => {
            if (human.name) {
              results.push({
                id: human.id,
                type: "human",
                label: human.name,
              });
            }
          });
          organizations.forEach((organization) => {
            if (organization.name) {
              results.push({
                id: organization.id,
                type: "organization",
                label: organization.name,
              });
            }
          });
        }

        return results.slice(0, 5);
      },
    }),
    [sessions, humans, organizations, search],
  );
}
