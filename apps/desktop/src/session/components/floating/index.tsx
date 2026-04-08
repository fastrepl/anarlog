import { ListenButton } from "./listen";

import {
  useCurrentNoteTab,
  useHasTranscript,
} from "~/session/components/shared";
import { ChatCTA } from "~/shared/chat-cta";
import type { Tab } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function FloatingActionButton({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const shouldShowListen = useShouldShowListeningFab(tab);
  const shouldShowChat = useShouldShowChatFab(tab);

  if (!shouldShowListen && !shouldShowChat) {
    return null;
  }

  return (
    <div className="absolute bottom-4 left-1/2 z-20 -translate-x-1/2">
      {shouldShowListen ? <ListenButton tab={tab} /> : <ChatCTA />}
    </div>
  );
}

export function useShouldShowListeningFab(
  tab: Extract<Tab, { type: "sessions" }>,
) {
  const currentTab = useCurrentNoteTab(tab);
  const hasTranscript = useHasTranscript(tab.id);

  return currentTab.type === "raw" && !hasTranscript;
}

function useShouldShowChatFab(tab: Extract<Tab, { type: "sessions" }>) {
  const hasTranscript = useHasTranscript(tab.id);
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));

  return hasTranscript && sessionMode === "inactive";
}
