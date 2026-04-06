import { useMainTabsShortcuts } from "~/shared/useTabsShortcuts";
import { useTabs } from "~/store/zustand/tabs";

export function useMain2TabsShortcuts() {
  const clearSelection = useTabs((state) => state.clearSelection);

  return useMainTabsShortcuts({ onModT: clearSelection });
}
