import { useCallback } from "react";

import { useMainStoreInternal } from "~/session/hooks/internal";

export function useUpdateDailyNoteContent(
  date: string,
): (contentJson: string) => void {
  const store = useMainStoreInternal();
  return useCallback(
    (contentJson) => {
      if (!store) return;
      store.setPartialRow("daily_notes", date, {
        content: contentJson,
        date,
      });
    },
    [store, date],
  );
}
