import { useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { useScheduleTaskRunCallback } from "tinytick/ui-react";

import {
  type DeepLink,
  commands as deeplink2Commands,
  events as deeplink2Events,
} from "@hypr/plugin-deeplink2";
import { dismissInstruction } from "@hypr/plugin-windows";

import { useAuth } from "~/auth";
import {
  CALENDAR_SYNC_TASK_ID,
  removeDisconnectedCalendarConnection,
} from "~/services/calendar";
import {
  createShareOpenProcessor,
  subscribeThenDrainShareOpens,
} from "~/shared-notes/deeplink";
import { subscribeThenDrainDeepLinks } from "~/shared/deeplink";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
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
  const authRef = useLatestRef(auth);
  const queryClientRef = useLatestRef(queryClient);
  const openNewRef = useLatestRef(openNew);
  const scheduleCalendarSyncRef = useLatestRef(scheduleCalendarSync);

  useMountEffect(() => {
    if (!isTauri()) {
      return;
    }

    const timeoutIds = new Set<number>();
    const invalidateIntegrationState = () => {
      void queryClientRef.current.invalidateQueries({
        predicate: (query) => query.queryKey[0] === "integration-status",
      });
    };
    const refreshIntegrationState = () => {
      invalidateIntegrationState();
      scheduleCalendarSyncRef.current();
    };
    const handleDeepLink = (payload: DeepLink) => {
      if (payload.to === "/auth/callback") {
        const { access_token, refresh_token } = payload.search;
        if (access_token && refresh_token) {
          void authRef.current.setSessionFromTokens(
            access_token,
            refresh_token,
          );
        }
      } else if (payload.to === "/billing/refresh") {
        void authRef.current.refreshSession();
        void dismissInstruction();
      } else if (payload.to === "/integration/callback") {
        const {
          disconnected_connection_id,
          integration_id,
          status,
          return_to,
        } = payload.search;
        if (status === "success") {
          console.log(`[deeplink] integration updated: ${integration_id}`);
          if (disconnected_connection_id) {
            invalidateIntegrationState();
            void removeDisconnectedCalendarConnection(
              integration_id,
              disconnected_connection_id,
            ).catch((error) => {
              console.error(
                "[calendar] failed to remove disconnected calendar data",
                error,
              );
            });
          } else {
            refreshIntegrationState();
            for (const delay of [1000, 3000]) {
              const timeoutId = window.setTimeout(() => {
                timeoutIds.delete(timeoutId);
                refreshIntegrationState();
              }, delay);
              timeoutIds.add(timeoutId);
            }
          }

          void dismissInstruction().then(() => {
            if (return_to === "calendar" || return_to === "settings-calendar") {
              openNewRef.current({ type: "calendar" });
            } else if (return_to === "todo") {
              openNewRef.current({
                type: "settings",
                state: { tab: "todo" },
              });
            }
          });
        }
      }
    };
    const deepLinkSubscription = subscribeThenDrainDeepLinks({
      listen: (handler) =>
        deeplink2Events.deepLinkEvent.listen(({ payload }) => {
          handler(payload);
        }),
      takePendingDeepLinks: deeplink2Commands.takePendingDeepLinks,
      handle: handleDeepLink,
    });
    const shareOpenProcessor = createShareOpenProcessor({
      takePendingShareOpen: deeplink2Commands.takePendingShareOpen,
      getAuth: () => authRef.current,
      openNew: (tab) => openNewRef.current(tab),
    });
    const shareOpenSubscription = subscribeThenDrainShareOpens({
      listen: (handler) =>
        deeplink2Events.shareOpenPendingEvent.listen(({ payload }) => {
          handler(payload.pending_id);
        }),
      listPendingShareOpens: deeplink2Commands.listPendingShareOpens,
      handle: shareOpenProcessor.handle,
    });

    return () => {
      shareOpenProcessor.dispose();
      for (const timeoutId of timeoutIds) {
        window.clearTimeout(timeoutId);
      }
      void deepLinkSubscription.then((fn) => fn()).catch(() => {});
      void shareOpenSubscription.then((fn) => fn()).catch(() => {});
    };
  });
}
