import { useCallback } from "react";

import { useMainTabsShortcuts } from "~/shared/useTabsShortcuts";
import { useTabs } from "~/store/zustand/tabs";

export function useClassicMainTabsShortcuts() {
  const newEmptyTab = useNewEmptyTab();

  return useMainTabsShortcuts({ onModT: newEmptyTab });
}

function useNewEmptyTab() {
  const openNew = useTabs((state) => state.openNew);

  const handler = useCallback(() => {
    openNew({ type: "empty" });
  }, [openNew]);

  return handler;
}
