import { useQuery } from "@tanstack/react-query";

import { commands as detectCommands } from "@anlg/plugin-detect";

import { getIgnorableApps, type MicApp } from "~/stt/meeting-apps";

const MEETING_MIC_POLL_INTERVAL_MS = 5_000;

export function micAppsShowInUse(apps: MicApp[]): boolean {
  return getIgnorableApps(apps).some((app) => {
    const key = app.id.toLowerCase();
    return !key.includes("anarlog") && !key.includes("hyprnote");
  });
}

export function useMeetingMicInUse(enabled: boolean): boolean {
  const { data = false } = useQuery({
    queryKey: ["meeting-mic-in-use"],
    queryFn: async () => {
      const result = await detectCommands.listMicUsingApplications();
      if (result.status === "error") {
        return false;
      }
      return micAppsShowInUse(result.data);
    },
    enabled,
    refetchInterval: enabled ? MEETING_MIC_POLL_INTERVAL_MS : false,
    refetchIntervalInBackground: false,
    retry: false,
    staleTime: MEETING_MIC_POLL_INTERVAL_MS - 500,
  });

  return enabled && data;
}
