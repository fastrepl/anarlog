import { useCallback, useMemo, useRef } from "react";

import { buildEditSummaryTool } from "~/chat/tools/edit-summary";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function useEditSummaryTools(
  getSessionId: () => string | undefined,
  getEnhancedNoteId: () => string | undefined,
) {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);

  const storeRef = useRef(store);
  storeRef.current = store;
  const indexesRef = useRef(indexes);
  indexesRef.current = indexes;

  const openEditTab = useCallback((requestId: string) => {
    useTabs.getState().openNew({ type: "edit", requestId });
  }, []);

  const extraTools = useMemo(
    () => ({
      edit_summary: buildEditSummaryTool({
        getStore: () => storeRef.current ?? undefined,
        getIndexes: () => indexesRef.current ?? undefined,
        getSessionId,
        getEnhancedNoteId,
        openEditTab,
      }),
    }),
    [getSessionId, getEnhancedNoteId, openEditTab],
  );

  return { extraTools };
}
