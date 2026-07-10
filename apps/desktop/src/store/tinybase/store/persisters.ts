import { useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { useInitializeStore } from "./initialize";
import { isLegacyDataPersistenceDisabled } from "./legacy-persistence";
import { type Store } from "./main";
import { registerSaveHandler } from "./save";

import { useCalendarPersister } from "~/store/tinybase/persister/calendar";
import { useChatPersister } from "~/store/tinybase/persister/chat";
import { useDailyNotePersister } from "~/store/tinybase/persister/daily-note";
import { useEventsPersister } from "~/store/tinybase/persister/events";
import { useHumanPersister } from "~/store/tinybase/persister/human";
import { useOrganizationPersister } from "~/store/tinybase/persister/organization";
import { useSessionPersister } from "~/store/tinybase/persister/session";
import { useTaskPersister } from "~/store/tinybase/persister/tasks";
import { useValuesPersister } from "~/store/tinybase/persister/values";

export function useMainPersisters(store: Store) {
  const valuesPersister = useValuesPersister(store);
  const sessionPersister = useSessionPersister(store);
  const organizationPersister = useOrganizationPersister(store);
  const humanPersister = useHumanPersister(store);
  const eventPersister = useEventsPersister(store);
  const chatPersister = useChatPersister(store);
  const calendarPersister = useCalendarPersister(store);
  const dailyNotePersister = useDailyNotePersister(store);
  const taskPersister = useTaskPersister(store);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    const persisters = [
      { id: "values", persister: valuesPersister },
      { id: "session", persister: sessionPersister },
      { id: "organization", persister: organizationPersister },
      { id: "human", persister: humanPersister },
      { id: "event", persister: eventPersister },
      { id: "chat", persister: chatPersister },
      { id: "calendar", persister: calendarPersister },
      { id: "dailyNote", persister: dailyNotePersister },
      { id: "task", persister: taskPersister },
    ];

    const unsubscribes = persisters
      .filter(({ persister }) => persister)
      .map(({ id, persister }) =>
        registerSaveHandler(id, async () => {
          if (id !== "values" && isLegacyDataPersistenceDisabled()) {
            return;
          }
          await persister!.save();
        }),
      );

    return () => {
      unsubscribes.forEach((unsub) => unsub());
    };
  }, [
    valuesPersister,
    sessionPersister,
    organizationPersister,
    humanPersister,
    eventPersister,
    chatPersister,
    calendarPersister,
    dailyNotePersister,
    taskPersister,
  ]);

  useInitializeStore(store, {
    session: sessionPersister,
    human: humanPersister,
    values: valuesPersister,
  });

  return {
    valuesPersister,
    sessionPersister,
    organizationPersister,
    humanPersister,
    eventPersister,
    chatPersister,
    calendarPersister,
    dailyNotePersister,
    taskPersister,
  };
}
