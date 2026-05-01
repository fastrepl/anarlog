import { useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { useScheduleTaskRunCallback } from "tinytick/ui-react";

import { events as deeplink2Events } from "@hypr/plugin-deeplink2";
import { dismissInstruction } from "@hypr/plugin-windows";

import { useAuth } from "~/auth";
import { CALENDAR_SYNC_TASK_ID } from "~/services/calendar";
import { useTabs } from "~/store/zustand/tabs";

export function useDeeplinkHandler() {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const openNew = useTabs((state) => state.openNew);
  const scheduleCalendarSync = useScheduleTaskRunCallback(
    CALENDAR_SYNC_TASK_ID,
    undefined,
    0,
  );

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    const timeoutIds = new Set<number>();
    const refreshIntegrationState = () => {
      void queryClient.invalidateQueries({
        predicate: (query) => query.queryKey[0] === "integration-status",
      });
      scheduleCalendarSync();
    };

    const unlisten = deeplink2Events.deepLinkEvent.listen(({ payload }) => {
      if (payload.to === "/auth/callback") {
        const { access_token, refresh_token } = payload.search;
        if (access_token && refresh_token && auth) {
          void auth.setSessionFromTokens(access_token, refresh_token);
        }
      } else if (payload.to === "/billing/refresh") {
        if (auth) {
          void auth.refreshSession();
        }
        void dismissInstruction();
      } else if (payload.to === "/integration/callback") {
        const { integration_id, status, return_to } = payload.search;
        if (status === "success") {
          console.log(`[deeplink] integration updated: ${integration_id}`);
          refreshIntegrationState();
          for (const delay of [1000, 3000]) {
            const timeoutId = window.setTimeout(() => {
              timeoutIds.delete(timeoutId);
              refreshIntegrationState();
            }, delay);
            timeoutIds.add(timeoutId);
          }

          void dismissInstruction().then(() => {
            if (return_to === "calendar" || return_to === "settings-calendar") {
              openNew({ type: "calendar" });
            }
          });
        }
      }
    });

    return () => {
      for (const timeoutId of timeoutIds) {
        window.clearTimeout(timeoutId);
      }
      void unlisten.then((fn) => fn());
    };
  }, [auth, openNew, queryClient, scheduleCalendarSync]);
}
