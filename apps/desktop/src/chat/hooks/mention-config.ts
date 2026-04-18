import { useMemo } from "react";

import type { MentionConfig } from "~/editor/widgets";
import { useSearchEngine } from "~/search/contexts/engine";
import { useMentionableEntities } from "~/session/hooks/queries";

export function useMentionConfig(): MentionConfig {
  const entities = useMentionableEntities();
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
          results.push(...entities);
        }

        return results.slice(0, 5);
      },
    }),
    [entities, search],
  );
}
