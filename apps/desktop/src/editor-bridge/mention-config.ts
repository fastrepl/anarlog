import { useMemo } from "react";

import type { MentionConfig } from "@hypr/editor/widgets";

import { useSearchEngine } from "~/search/contexts/engine";
import * as main from "~/store/tinybase/store/main";

export function useMentionConfig(): MentionConfig {
  const sessions = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );
  const humans = main.UI.useResultTable(
    main.QUERIES.visibleHumans,
    main.STORE_ID,
  );
  const organizations = main.UI.useResultTable(
    main.QUERIES.visibleOrganizations,
    main.STORE_ID,
  );
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
          Object.entries(sessions).forEach(([rowId, row]) => {
            const title = row.title as string | undefined;
            if (title) {
              results.push({ id: rowId, type: "session", label: title });
            }
          });
          Object.entries(humans).forEach(([rowId, row]) => {
            const name = row.name as string | undefined;
            if (name) {
              results.push({ id: rowId, type: "human", label: name });
            }
          });
          Object.entries(organizations).forEach(([rowId, row]) => {
            const name = row.name as string | undefined;
            if (name) {
              results.push({ id: rowId, type: "organization", label: name });
            }
          });
        }

        return results.slice(0, 5);
      },
    }),
    [sessions, humans, organizations, search],
  );
}
