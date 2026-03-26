import { useEffect } from "react";

import type { Store } from "./main";

import { DEFAULT_USER_ID } from "~/shared/utils";

export function useInitializeStore(
  store: Store,
  persisters: { session: unknown; human: unknown; values: unknown },
): void {
  const { session, human, values } = persisters;

  useEffect(() => {
    if (!store || !session || !human || !values) {
      return;
    }

    initializeStore(store);
  }, [store, session, human, values]);
}
function initializeStore(store: Store): void {
  store.transaction(() => {
    if (!store.hasValue("user_id")) {
      store.setValue("user_id", DEFAULT_USER_ID);
    }

    const userId = store.getValue("user_id") as string;
    if (!store.hasRow("humans", userId)) {
      store.setRow("humans", userId, {
        user_id: userId,
        name: "",
        email: "",
        org_id: "",
      });
    }

    if (
      !store.getTableIds().includes("sessions") ||
      store.getRowIds("sessions").length === 0
    ) {
      const sessionId = crypto.randomUUID();
      const now = new Date().toISOString();

      store.setRow("sessions", sessionId, {
        user_id: DEFAULT_USER_ID,
        created_at: now,
        title: "Welcome to Char",
        raw_md: "",
      });
    }
  });
}
