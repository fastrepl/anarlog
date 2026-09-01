import { useQuery } from "@tanstack/react-query";

import { commands as detectCommands } from "@anlg/plugin-detect";

import { inspectionShowsActiveMeeting } from "~/stt/meeting-accessibility";

const MEETING_ACCESSIBILITY_POLL_INTERVAL_MS = 5_000;

export function useMeetingAccessibilityActive(enabled: boolean): boolean {
  const { data = false } = useQuery({
    queryKey: ["meeting-accessibility-active"],
    queryFn: async () => {
      const result = await detectCommands.inspectMeetingAccessibility();
      if (result.status === "error") return false;
      return result.data.some(inspectionShowsActiveMeeting);
    },
    enabled,
    refetchInterval: enabled ? MEETING_ACCESSIBILITY_POLL_INTERVAL_MS : false,
    refetchIntervalInBackground: false,
    retry: false,
    staleTime: MEETING_ACCESSIBILITY_POLL_INTERVAL_MS - 500,
  });

  return enabled && data;
}
