import { cn } from "@hypr/utils";

import { ListenButton } from "./listen";

import {
  useCurrentNoteTab,
  useHasTranscript,
} from "~/session/components/shared";
import { ChatCTA } from "~/shared/chat-cta";
import type { Tab } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function FloatingActionButton({
  hidden = false,
  tab,
}: {
  hidden?: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const shouldShowListen = useShouldShowListeningFab(tab);
  const shouldShowChat = useShouldShowChatFab(tab);

  if (!shouldShowListen && !shouldShowChat) {
    return null;
  }

  return (
    <div
      className={cn([
        "absolute bottom-0 left-1/2 z-20 h-14 w-96 max-w-[calc(100%-2rem)] -translate-x-1/2",
        hidden ? "group pointer-events-auto" : "pointer-events-none",
      ])}
    >
      <div
        aria-hidden={hidden}
        className={cn([
          "absolute bottom-4 left-1/2 -translate-x-1/2 transition-[opacity,visibility,transform] duration-150",
          hidden
            ? "pointer-events-none visible translate-y-[calc(100%+0.5rem)] opacity-100 group-hover:pointer-events-auto group-hover:translate-y-0"
            : "pointer-events-auto visible translate-y-0 opacity-100",
        ])}
      >
        {shouldShowListen ? <ListenButton tab={tab} /> : <ChatCTA />}
      </div>
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
