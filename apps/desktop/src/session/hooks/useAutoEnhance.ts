import { useEffect, useState } from "react";

import { getEnhancerService } from "~/services/enhancer";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function useAutoEnhance(tab: Extract<Tab, { type: "sessions" }>) {
  const sessionId = tab.id;
  const [skipReason, setSkipReason] = useState<string | null>(null);

  useEffect(() => {
    const service = getEnhancerService();
    if (!service) return;
    return service.on((event) => {
      if (event.sessionId !== sessionId) return;
      if (event.type === "auto-enhance-skipped") {
        setSkipReason(event.reason);
      }
      if (event.type === "auto-enhance-started") {
        const tabsState = useTabs.getState();
        const sessionTab = tabsState.tabs.find(
          (t): t is Extract<Tab, { type: "sessions" }> =>
            t.type === "sessions" && t.id === sessionId,
        );
        if (sessionTab) {
          tabsState.updateSessionTabState(sessionTab, {
            ...sessionTab.state,
            view: { type: "enhanced", id: event.noteId },
          });
        }
      }
      if (event.type === "auto-enhance-no-model") {
        setSkipReason("No AI model configured");
      }
    });
  }, [sessionId]);

  useEffect(() => {
    if (skipReason) {
      const timer = setTimeout(() => setSkipReason(null), 5000);
      return () => clearTimeout(timer);
    }
  }, [skipReason]);

  return { skipReason };
}
