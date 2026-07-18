import { useCallback } from "react";

import { type Tab, useTabs } from "~/store/zustand/tabs";

export function useOpenTemplatesTab() {
  const openNew = useTabs((state) => state.openNew);
  const selectTab = useTabs((state) => state.select);
  const updateTemplatesTabState = useTabs(
    (state) => state.updateTemplatesTabState,
  );

  return useCallback(
    (state: Extract<Tab, { type: "templates" }>["state"]) => {
      const existingTemplatesTab = useTabs
        .getState()
        .tabs.find(
          (tab): tab is Extract<Tab, { type: "templates" }> =>
            tab.type === "templates",
        );

      if (!existingTemplatesTab) {
        openNew({ type: "templates", state });
        return;
      }

      updateTemplatesTabState(existingTemplatesTab, state);
      selectTab(existingTemplatesTab);
    },
    [openNew, selectTab, updateTemplatesTabState],
  );
}
