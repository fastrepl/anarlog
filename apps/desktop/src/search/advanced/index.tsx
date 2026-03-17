import { SearchIcon } from "lucide-react";
import { useCallback } from "react";

import { AdvancedSearchView } from "./view";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export const TabItemSearch: TabItem<Extract<Tab, { type: "search" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<SearchIcon className="h-4 w-4" />}
      title="Advanced Search"
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

export function TabContentSearch({
  tab,
}: {
  tab: Extract<Tab, { type: "search" }>;
}) {
  return (
    <StandardTabWrapper>
      <SearchView tab={tab} />
    </StandardTabWrapper>
  );
}

function SearchView({ tab }: { tab: Extract<Tab, { type: "search" }> }) {
  const updateSearchTabState = useTabs((state) => state.updateSearchTabState);
  const openNew = useTabs((state) => state.openNew);

  const { selectedTypes } = tab.state;

  const setSelectedTypes = useCallback(
    (types: string[] | null) => {
      updateSearchTabState(tab, {
        ...tab.state,
        selectedTypes: types,
      });
    },
    [updateSearchTabState, tab],
  );

  const handleResultClick = useCallback(
    (type: string, id: string) => {
      if (type === "session") {
        openNew({ type: "sessions", id });
      } else if (type === "human") {
        openNew({
          type: "contacts",
          state: { selected: { type: "person", id } },
        });
      } else if (type === "organization") {
        openNew({
          type: "contacts",
          state: { selected: { type: "organization", id } },
        });
      }
    },
    [openNew],
  );

  return (
    <AdvancedSearchView
      initialQuery={tab.state.initialQuery ?? undefined}
      selectedTypes={selectedTypes}
      setSelectedTypes={setSelectedTypes}
      onResultClick={handleResultClick}
    />
  );
}
