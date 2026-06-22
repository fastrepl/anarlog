import { SparklesIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import type { CSSProperties } from "react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { useCaretPosition } from "../caret-position-context";
import { ListenButton } from "./listen";
import { FloatingButton } from "./shared";

import { useAITask } from "~/ai/contexts";
import { type LLMConnectionStatus, useLLMConnectionStatus } from "~/ai/hooks";
import { getEnhancerService } from "~/services/enhancer";
import {
  useCurrentNoteTab,
  hasStoredNoteContent,
  useHasTranscript,
} from "~/session/components/shared";
import { ChatCTA } from "~/shared/chat-cta";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { useTabs } from "~/store/zustand/tabs";
import type { Tab } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function FloatingActionButton({
  allowListening = true,
  skipReason = null,
  tab,
}: {
  allowListening?: boolean;
  skipReason?: string | null;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const canShowListen = useShouldShowListeningFab(tab, sessionMode);
  const shouldShowListen = allowListening && canShowListen;
  const shouldShowChat = useShouldShowChatFab(tab, sessionMode);
  const generateSummaryNoteId = useGenerateSummaryNoteId(tab, sessionMode);
  const shouldShowGenerateSummary = generateSummaryNoteId !== null;
  const isCaretNearBottom = useCaretPosition()?.isCaretNearBottom ?? false;
  const showSkipReason = !!skipReason;
  const useChatHoverArea =
    !showSkipReason && !shouldShowGenerateSummary && shouldShowChat;
  const tuckListenAction =
    !showSkipReason && shouldShowListen && isCaretNearBottom;

  if (
    !showSkipReason &&
    !shouldShowListen &&
    !shouldShowGenerateSummary &&
    !shouldShowChat
  ) {
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
            key={
              shouldShowListen
                ? "listen"
                : shouldShowGenerateSummary
                  ? "generate-summary"
                  : "chat"
            }
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
            {shouldShowListen ? (
              <ListenButton tab={tab} />
            ) : shouldShowGenerateSummary ? (
              <GenerateSummaryButton
                tab={tab}
                enhancedNoteId={generateSummaryNoteId}
              />
            ) : (
              <ChatCTA />
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function GenerateSummaryButton({
  tab,
  enhancedNoteId,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  enhancedNoteId: string;
}) {
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const templateId = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "template_id",
    main.STORE_ID,
  ) as string | undefined;

  const handleGenerateSummary = useCallback(() => {
    const result = getEnhancerService()?.enhance(tab.id, {
      templateId: templateId || undefined,
    });

    if (
      (result?.type === "started" || result?.type === "already_active") &&
      result.noteId
    ) {
      updateSessionTabState(tab, {
        ...tab.state,
        view: { type: "enhanced", id: result.noteId },
      });
    }
  }, [tab, templateId, updateSessionTabState]);

  return (
    <FloatingButton
      onClick={handleGenerateSummary}
      className="w-[164px] justify-start gap-2 px-4"
    >
      <span className="flex items-center gap-1.5">
        <SparklesIcon className="size-3.5" /> Generate summary
      </span>
    </FloatingButton>
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

function useGenerateSummaryNoteId(
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
  const canStartEnhance =
    taskStatus === undefined ||
    taskStatus === "idle" ||
    taskStatus === "success";

  if (
    sessionMode === "inactive" &&
    hasTranscript &&
    enhancedNoteId &&
    canStartEnhance &&
    !hasStoredNoteContent(content) &&
    !isBlockingLLMStatus(llmStatus)
  ) {
    return enhancedNoteId;
  }

  return null;
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
  const hasContent = hasStoredNoteContent(content);
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
