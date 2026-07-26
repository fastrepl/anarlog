import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import { commands } from "~/types/tauri.gen";

export function useDismissedToasts(): {
  dismissedToasts: string[];
  dismissToast: (id: string) => void;
  isDismissed: (id: string) => boolean;
} {
  const queryClient = useQueryClient();

  const { data: dismissedToasts = [] } = useQuery({
    queryKey: ["dismissed_toasts"],
    queryFn: async () => {
      const result = await commands.getDismissedToasts();
      if (result.status === "ok") {
        return result.data;
      }
      return [];
    },
  });

  const dismissedSet = useMemo(
    () => new Set(dismissedToasts),
    [dismissedToasts],
  );

  const dismissToast = useCallback(
    (id: string) => {
      if (dismissedSet.has(id)) {
        return;
      }

      const updated = [...dismissedToasts, id];
      commands.setDismissedToasts(updated).then(() => {
        queryClient.invalidateQueries({ queryKey: ["dismissed_toasts"] });
      });
    },
    [dismissedToasts, dismissedSet, queryClient],
  );

  const isDismissed = useCallback(
    (id: string) => dismissedSet.has(id),
    [dismissedSet],
  );

  return {
    dismissedToasts,
    dismissToast,
    isDismissed,
  };
}
