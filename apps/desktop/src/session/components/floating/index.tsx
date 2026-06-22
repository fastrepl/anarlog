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
  skipReason = null,
  tab,
}: {
  skipReason?: string | null;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const shouldShowListen = useShouldShowListeningFab(tab, sessionMode);
  const shouldShowChat = useShouldShowChatFab(tab, sessionMode);
  const isCaretNearBottom = useCaretPosition()?.isCaretNearBottom ?? false;
  const showSkipReason = !!skipReason;
  const useChatHoverArea = !showSkipReason && shouldShowChat;
  const tuckListenAction =
    !showSkipReason && shouldShowListen && isCaretNearBottom;

  if (!showSkipReason && !shouldShowListen && !shouldShowChat) {
    return null;
  }

  return (
    <div
      className={cn([
        "absolute left-1/2 z-30 flex max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center",
        tuckListenAction
          ? "group pointer-events-auto bottom-0 h-32 pb-4"
          : cn([
              "pointer-events-none",
              useChatHoverArea
                ? "bottom-3 h-10 w-40 pb-0"
                : "bottom-0 h-14 pb-4",
            ]),
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
            aria-hidden={tuckListenAction}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            style={
              {
                "--floating-fab-tuck-offset": tuckListenAction
                  ? "calc(100% - 0.5rem + 18px)"
                  : "0px",
              } as CSSProperties
            }
            className={cn([
              "relative max-w-full translate-y-[var(--floating-fab-tuck-offset)] transition-transform duration-200 ease-out",
              tuckListenAction
                ? "pointer-events-none visible group-hover:pointer-events-auto group-hover:translate-y-0 before:pointer-events-none before:absolute before:-inset-x-8 before:-inset-y-8 before:content-[''] hover:pointer-events-auto hover:translate-y-0"
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

function useShouldShowListeningFab(
  tab: Extract<Tab, { type: "sessions" }>,
  sessionMode: string,
) {
  const currentTab = useCurrentNoteTab(tab);
  const hasTranscript = useHasTranscript(tab.id);

  return (
    sessionMode === "inactive" && currentTab.type === "raw" && !hasTranscript
  );
}

function useShouldShowChatFab(
  tab: Extract<Tab, { type: "sessions" }>,
  sessionMode: string,
) {
  const hasTranscript = useHasTranscript(tab.id);
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

  const canShowForSessionMode =
    sessionMode === "inactive" || sessionMode === "active";

  return (
    canShowForSessionMode &&
    (hasTranscript || sessionMode === "active") &&
    !hasVisibleIssue
  );
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
