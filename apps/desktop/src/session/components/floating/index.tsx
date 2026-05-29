import { AnimatePresence, motion } from "motion/react";
import type { CSSProperties } from "react";

import { cn } from "@hypr/utils";

import { useCaretPosition } from "../caret-position-context";
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
  skipReason = null,
  tab,
}: {
  hidden?: boolean;
  skipReason?: string | null;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const shouldShowListen = useShouldShowListeningFab(tab);
  const shouldShowChat = useShouldShowChatFab(tab);
  const isCaretNearBottom = useCaretPosition()?.isCaretNearBottom ?? false;
  const showSkipReason = !!skipReason;
  const showAction = shouldShowListen || shouldShowChat;
  const tuckAction =
    !showSkipReason &&
    ((shouldShowListen && isCaretNearBottom) || (shouldShowChat && hidden));

  if (!showSkipReason && !showAction) {
    return null;
  }

  return (
    <div
      className={cn([
        "absolute bottom-0 left-1/2 z-20 flex h-14 max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center pb-4",
        tuckAction ? "group pointer-events-auto" : "pointer-events-none",
      ])}
    >
      <AnimatePresence mode="wait" initial={false}>
        {showSkipReason ? (
          <motion.div
            key={skipReason}
            role="status"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="max-w-full translate-y-0 text-center text-sm whitespace-nowrap text-red-400"
          >
            {skipReason}
          </motion.div>
        ) : (
          <motion.div
            key={shouldShowListen ? "listen" : "chat"}
            aria-hidden={tuckAction}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            style={
              {
                "--floating-fab-tuck-offset": tuckAction
                  ? "calc(100% - 0.5rem + 18px)"
                  : "0px",
              } as CSSProperties
            }
            className={cn([
              "max-w-full translate-y-[var(--floating-fab-tuck-offset)] transition-transform duration-200 ease-out",
              tuckAction
                ? "pointer-events-none visible group-hover:pointer-events-auto group-hover:translate-y-0"
                : "pointer-events-auto visible",
            ])}
          >
            {shouldShowListen ? <ListenButton tab={tab} /> : <ChatCTA />}
          </motion.div>
        )}
      </AnimatePresence>
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
