import { useCallback, useMemo, useSyncExternalStore } from "react";
import { AppState } from "react-native";

import { deriveBillingInfo, type SupabaseJwtPayload } from "./billing";

export function useBillingInfo(payload: SupabaseJwtPayload | null) {
  const subscribe = useCallback(
    (notify: () => void) => {
      const remaining = (payload?.trial_end ?? 0) * 1000 - Date.now();
      if (payload?.subscription_status !== "trialing" || remaining <= 0)
        return () => {};
      const interval = setInterval(notify, 60_000);
      const expiry = setTimeout(notify, Math.min(remaining, 2_147_483_647));
      const foreground = AppState.addEventListener("change", (state) => {
        if (state === "active") notify();
      });
      return () => {
        clearInterval(interval);
        clearTimeout(expiry);
        foreground.remove();
      };
    },
    [payload],
  );
  const days = useSyncExternalStore(
    subscribe,
    () => deriveBillingInfo(payload).trialDaysRemaining,
  );
  return useMemo(() => deriveBillingInfo(payload), [payload, days]);
}
