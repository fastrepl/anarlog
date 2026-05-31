import { AnimatePresence, motion } from "motion/react";
import type { CSSProperties } from "react";

import { cn } from "@hypr/utils";

import { useCaretPosition } from "../caret-position-context";
import { ListenButton } from "./listen";

import { useAITask } from "~/ai/contexts";
import { type LLMConnectionStatus, useLLMConnectionStatus } from "~/ai/hooks";
import {
  useCurrentNoteTab,
  useHasTranscript,
} from "~/session/components/shared";
import { ChatCTA } from "~/shared/chat-cta";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
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
    showAction &&
    (isCaretNearBottom || (shouldShowChat && hidden));

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

function useShouldShowListeningFab(tab: Extract<Tab, { type: "sessions" }>) {
  const currentTab = useCurrentNoteTab(tab);
  const hasTranscript = useHasTranscript(tab.id);

  return currentTab.type === "raw" && !hasTranscript;
}

function useShouldShowChatFab(tab: Extract<Tab, { type: "sessions" }>) {
  const hasTranscript = useHasTranscript(tab.id);
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const currentTab = useCurrentNoteTab(tab);
  const enhancedNoteId = currentTab.type === "enhanced" ? currentTab.id : null;
  const taskId = enhancedNoteId
    ? createTaskId(enhancedNoteId, "enhance")
    : null;
  const taskStatus = useAITask((state) =>
    taskId ? state.tasks[taskId]?.status : undefined,
  );
  const llmStatus = useLLMConnectionStatus();
  const content = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId ?? "",
    "content",
    main.STORE_ID,
  );
  const visibleTaskStatus = taskStatus ?? "idle";
  const hasContent = typeof content === "string" && content.trim().length > 0;
  const hasVisibleIssue =
    currentTab.type === "enhanced" &&
    (visibleTaskStatus === "error" ||
      (visibleTaskStatus === "idle" &&
        !hasContent &&
        isBlockingLLMStatus(llmStatus)));

  return hasTranscript && sessionMode === "inactive" && !hasVisibleIssue;
}

function isBlockingLLMStatus(status: LLMConnectionStatus) {
  if (status.status === "pending") {
    return true;
  }

  return (
    status.status === "error" &&
    (status.reason === "missing_config" ||
      status.reason === "not_pro" ||
      status.reason === "unauthenticated")
  );
}
