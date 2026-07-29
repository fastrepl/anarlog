import { useEffect, useRef } from "react";

import type { PermissionStatus } from "@anlg/plugin-permissions";

import { trackAnalyticsEvent } from "~/analytics";

export function usePermissionAnalytics(
  permission: string,
  status: PermissionStatus | undefined,
  entryPoint: "onboarding" | "settings",
) {
  const previousStatus = useRef(status);

  useEffect(() => {
    const previous = previousStatus.current;
    previousStatus.current = status;

    if (
      previous === undefined ||
      previous === status ||
      (status !== "authorized" && status !== "denied")
    ) {
      return;
    }

    trackAnalyticsEvent("permission_resolved", {
      permission,
      status,
      entry_point: entryPoint,
    });
  }, [entryPoint, permission, status]);
}

export function trackPermissionRequested(
  permission: string,
  status: PermissionStatus | undefined,
  entryPoint: "onboarding" | "settings",
  action: "request" | "open_settings",
) {
  trackAnalyticsEvent("permission_requested", {
    permission,
    previous_status: status ?? "unknown",
    entry_point: entryPoint,
    action,
  });
}
