import { useCallback } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { useTabs } from "~/store/zustand/tabs";

export function useSettings() {
  const openNew = useTabs((state) => state.openNew);

  const openSettings = useCallback(() => {
    openNew({ type: "settings" });
  }, [openNew]);

  useHotkeys(
    "mod+,",
    openSettings,
    {
      preventDefault: true,
      splitKey: "|",
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openSettings],
  );

  return { openSettings };
}
