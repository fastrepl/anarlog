import { type KeyboardEvent, useMemo } from "react";

import { useSearch } from "~/search/contexts/ui";
import { useTabs } from "~/store/zustand/tabs";

export function useSearchKeyboard() {
  const { query, setQuery, selectedIndex, setSelectedIndex, results } =
    useSearch();
  const openNew = useTabs((state) => state.openNew);

  const flatResults = useMemo(() => {
    if (!results) return [];
    return results.groups.flatMap((g) => g.results);
  }, [results]);

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      if (query.trim()) {
        setQuery("");
        setSelectedIndex(-1);
      } else {
        e.currentTarget.blur();
      }
    }
    if (e.key === "ArrowDown" && flatResults.length > 0) {
      e.preventDefault();
      setSelectedIndex(Math.min(selectedIndex + 1, flatResults.length - 1));
    }
    if (e.key === "ArrowUp" && flatResults.length > 0) {
      e.preventDefault();
      setSelectedIndex(Math.max(selectedIndex - 1, -1));
    }
    if (
      e.key === "Enter" &&
      !e.metaKey &&
      !e.ctrlKey &&
      selectedIndex >= 0 &&
      selectedIndex < flatResults.length
    ) {
      e.preventDefault();
      const item = flatResults[selectedIndex];
      if (item.type === "session") {
        openNew({ type: "sessions", id: item.id });
      } else if (item.type === "human") {
        openNew({
          type: "contacts",
          state: {
            selected: { type: "person", id: item.id },
          },
        });
      } else if (item.type === "organization") {
        openNew({
          type: "contacts",
          state: {
            selected: { type: "organization", id: item.id },
          },
        });
      }
      e.currentTarget.blur();
    }
  };

  return { onKeyDown };
}
