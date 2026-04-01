import { useQueryClient } from "@tanstack/react-query";
import { Copy } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { cn } from "@hypr/utils";

import { Toast } from "./component";
import { createToastRegistry, getToastToShow } from "./registry";
import { useDismissedToasts } from "./useDismissedToasts";

import { useAuth } from "~/auth";
import { useNotifications } from "~/contexts/notifications";
import { useConfigValues } from "~/shared/config";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useToastAction } from "~/store/zustand/toast-action";
import { commands } from "~/types/tauri.gen";

const SHARE_SNOOZE_PREFIX = "share-char-snoozed:";

function parseShareSnoozeCount(dismissedToasts: string[]): number | null {
  for (const id of dismissedToasts) {
    if (id.startsWith(SHARE_SNOOZE_PREFIX)) {
      return parseInt(id.slice(SHARE_SNOOZE_PREFIX.length), 10);
    }
  }
  return null;
}

export function ToastArea({
  isProfileExpanded,
}: {
  isProfileExpanded: boolean;
}) {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const { dismissToast, dismissedToasts, isDismissed } = useDismissedToasts();
  const shouldShowToast = useShouldShowToast(isProfileExpanded);
  const {
    hasActiveDownload,
    downloadProgress,
    downloadingModel,
    activeDownloads,
    localSttStatus,
    isLocalSttModel,
  } = useNotifications();

  const isAuthenticated = !!auth?.session;
  const isAuthLoading = auth.session === undefined;
  const {
    current_llm_provider,
    current_llm_model,
    current_stt_provider,
    current_stt_model,
  } = useConfigValues([
    "current_llm_provider",
    "current_llm_model",
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const hasLLMConfigured = !!(current_llm_provider && current_llm_model);
  const hasSttConfigured = !!(current_stt_provider && current_stt_model);
  const hasProSttConfigured =
    current_stt_provider === "hyprnote" && current_stt_model === "cloud";
  const hasProLlmConfigured = current_llm_provider === "hyprnote";

  const currentTab = useTabs((state) => state.currentTab);
  const isAiTranscriptionTabActive =
    currentTab?.type === "settings" &&
    currentTab.state?.tab === "transcription";
  const isAiIntelligenceTabActive =
    currentTab?.type === "settings" && currentTab.state?.tab === "intelligence";

  const openNew = useTabs((state) => state.openNew);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const setToastActionTarget = useToastAction((state) => state.setTarget);

  const handleSignIn = useCallback(async () => {
    await auth?.signIn();
  }, [auth]);

  const openAiTab = useCallback(
    (tab: "intelligence" | "transcription") => {
      if (currentTab?.type === "settings") {
        updateSettingsTabState(currentTab, { tab });
      } else {
        openNew({ type: "settings", state: { tab } });
      }
    },
    [currentTab, openNew, updateSettingsTabState],
  );

  const handleOpenLLMSettings = useCallback(() => {
    setToastActionTarget("llm");
    openAiTab("intelligence");
  }, [openAiTab, setToastActionTarget]);

  const handleOpenSTTSettings = useCallback(() => {
    setToastActionTarget("stt");
    openAiTab("transcription");
  }, [openAiTab, setToastActionTarget]);

  const sessionIds = main.UI.useRowIds("sessions", main.STORE_ID);
  const [shareExpanded, setShareExpanded] = useState(false);

  const shareSnoozedAtCount = useMemo(
    () => parseShareSnoozeCount(dismissedToasts),
    [dismissedToasts],
  );

  const handleShareExpand = useCallback(() => {
    void analyticsCommands.event({ event: "share_cta_opened" });
    setShareExpanded(true);
  }, []);

  const handleShareSnooze = useCallback(async () => {
    void analyticsCommands.event({ event: "share_cta_snoozed" });
    const filtered = dismissedToasts.filter(
      (id) => !id.startsWith(SHARE_SNOOZE_PREFIX),
    );
    filtered.push(`${SHARE_SNOOZE_PREFIX}${sessionIds.length}`);
    await commands.setDismissedToasts(filtered);
    queryClient.invalidateQueries({ queryKey: ["dismissed_toasts"] });
    setShareExpanded(false);
  }, [dismissedToasts, sessionIds.length, queryClient]);

  const handleShareDone = useCallback(() => {
    dismissToast("share-char");
    setShareExpanded(false);
  }, [dismissToast]);

  const handleShareCollapse = useCallback(() => {
    setShareExpanded(false);
  }, []);

  const handleShareSocial = useCallback(
    (platform: "x" | "linkedin" | "reddit") => {
      void analyticsCommands.event({
        event: "share_cta_shared",
        platform,
      });
      const text = encodeURIComponent(
        "I use Char AI notetaker and love it! Try it as well: char.com",
      );
      const url = encodeURIComponent("https://char.com");
      const urls: Record<string, string> = {
        x: `https://x.com/intent/tweet?text=${text}`,
        linkedin: `https://www.linkedin.com/sharing/share-offsite/?url=${url}&summary=${text}`,
        reddit: `https://www.reddit.com/submit?title=${text}&url=${url}`,
      };
      void openerCommands.openUrl(urls[platform], null);
      handleShareDone();
    },
    [handleShareDone],
  );

  const registry = useMemo(
    () =>
      createToastRegistry({
        isAuthenticated,
        isAuthLoading,
        hasLLMConfigured,
        hasSttConfigured,
        hasProSttConfigured,
        hasProLlmConfigured,
        isAiTranscriptionTabActive,
        isAiIntelligenceTabActive,
        hasActiveDownload,
        downloadProgress,
        downloadingModel,
        activeDownloads,
        localSttStatus,
        isLocalSttModel,
        sessionCount: sessionIds.length,
        shareSnoozedAtCount,
        onSignIn: handleSignIn,
        onOpenLLMSettings: handleOpenLLMSettings,
        onOpenSTTSettings: handleOpenSTTSettings,
        onShareExpand: handleShareExpand,
      }),
    [
      isAuthenticated,
      isAuthLoading,
      hasLLMConfigured,
      hasSttConfigured,
      hasProSttConfigured,
      hasProLlmConfigured,
      isAiTranscriptionTabActive,
      isAiIntelligenceTabActive,
      hasActiveDownload,
      downloadProgress,
      downloadingModel,
      activeDownloads,
      localSttStatus,
      isLocalSttModel,
      sessionIds.length,
      shareSnoozedAtCount,
      handleSignIn,
      handleOpenLLMSettings,
      handleOpenSTTSettings,
      handleShareExpand,
    ],
  );

  const currentToast = useMemo(
    () => getToastToShow(registry, isDismissed),
    [registry, isDismissed],
  );

  const handleDismiss = useCallback(() => {
    if (currentToast) {
      dismissToast(currentToast.id);
    }
  }, [currentToast, dismissToast]);

  const displayToast = useMemo(() => {
    if (!currentToast) return null;
    if (currentToast.id === "share-char" && shareExpanded) {
      return {
        ...currentToast,
        title: "Choose a platform",
        description: "Share your experience with others.",
        primaryAction: undefined,
        secondaryAction: undefined,
        dismissible: false,
        actions: [
          {
            label: "X (Twitter)",
            icon: <img src="/assets/X logo.svg" alt="X" className="size-5" />,
            onClick: () => handleShareSocial("x"),
          },
          {
            label: "LinkedIn",
            icon: (
              <img
                src="/assets/linkedin logo.svg"
                alt="LinkedIn"
                className="size-5"
              />
            ),
            onClick: () => handleShareSocial("linkedin"),
          },
          {
            label: "Reddit",
            icon: (
              <img
                src="/assets/reddit logo.svg"
                alt="Reddit"
                className="size-5"
              />
            ),
            onClick: () => handleShareSocial("reddit"),
          },
          {
            label: "Copy text",
            icon: <Copy className="size-4" />,
            onClick: () => {
              void analyticsCommands.event({
                event: "share_cta_shared",
                platform: "copy",
              });
              void navigator.clipboard.writeText(
                "I use Char AI notetaker and love it! Try it as well: char.com",
              );
              handleShareDone();
            },
          },
        ],
      };
    }
    return currentToast;
  }, [currentToast, shareExpanded, handleShareSocial, handleShareDone]);

  const dismissAction =
    displayToast?.id === "share-char"
      ? shareExpanded
        ? handleShareCollapse
        : handleShareSnooze
      : displayToast?.dismissible
        ? handleDismiss
        : undefined;

  return (
    <AnimatePresence mode="wait">
      {shouldShowToast && displayToast ? (
        <motion.div
          key={displayToast.id}
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 16 }}
          transition={{ duration: 0.3, ease: "easeInOut" }}
          className={cn([
            "absolute right-0 bottom-0 left-0 z-20",
            "pointer-events-none",
          ])}
        >
          <div className="pointer-events-auto">
            <Toast
              toast={displayToast}
              onDismiss={dismissAction}
              alwaysShowDismissButton={displayToast.id === "share-char"}
            />
          </div>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

function useShouldShowToast(isProfileExpanded: boolean) {
  const TOAST_CHECK_DELAY_MS = 500;

  const [showToast, setShowToast] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => {
      setShowToast(true);
    }, TOAST_CHECK_DELAY_MS);

    return () => clearTimeout(timer);
  }, []);

  return !isProfileExpanded && showToast;
}
