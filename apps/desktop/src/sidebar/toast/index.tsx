import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { Toast } from "./component";
import { createToastRegistry, getToastToShow } from "./registry";
import { useDismissedToasts } from "./useDismissedToasts";

import { useAuth } from "~/auth";
import { useNotifications } from "~/contexts/notifications";
import { useConfigValues } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";
import { useToastAction } from "~/store/zustand/toast-action";

export function ToastArea({
  isProfileExpanded,
}: {
  isProfileExpanded: boolean;
}) {
  const auth = useAuth();
  const { dismissToast, isDismissed } = useDismissedToasts();
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
    currentTab?.type === "ai" && currentTab.state?.tab === "transcription";
  const isAiIntelligenceTabActive =
    currentTab?.type === "ai" && currentTab.state?.tab === "intelligence";

  const openNew = useTabs((state) => state.openNew);
  const updateAiTabState = useTabs((state) => state.updateAiTabState);
  const setToastActionTarget = useToastAction((state) => state.setTarget);

  const handleSignIn = useCallback(async () => {
    await auth?.signIn();
  }, [auth]);

  const openAiTab = useCallback(
    (tab: "intelligence" | "transcription") => {
      if (currentTab?.type === "ai") {
        updateAiTabState(currentTab, { tab });
      } else {
        openNew({ type: "ai", state: { tab } });
      }
    },
    [currentTab, openNew, updateAiTabState],
  );

  const handleOpenLLMSettings = useCallback(() => {
    setToastActionTarget("llm");
    openAiTab("intelligence");
  }, [openAiTab, setToastActionTarget]);

  const handleOpenSTTSettings = useCallback(() => {
    setToastActionTarget("stt");
    openAiTab("transcription");
  }, [openAiTab, setToastActionTarget]);

  const registry = useMemo(
    () =>
      createToastRegistry({
        isAuthenticated,
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
        onSignIn: handleSignIn,
        onOpenLLMSettings: handleOpenLLMSettings,
        onOpenSTTSettings: handleOpenSTTSettings,
      }),
    [
      isAuthenticated,
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
      handleSignIn,
      handleOpenLLMSettings,
      handleOpenSTTSettings,
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

  return (
    <AnimatePresence mode="wait">
      {shouldShowToast && currentToast ? (
        <motion.div
          key={currentToast.id}
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
              toast={currentToast}
              onDismiss={currentToast.dismissible ? handleDismiss : undefined}
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
