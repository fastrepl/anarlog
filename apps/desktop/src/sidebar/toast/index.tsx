import { AnimatePresence, motion } from "motion/react";
import { useCallback, useMemo, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

import { Toast } from "./component";
import {
  createDevtoolsToastPreview,
  createToastRegistry,
  getToastToShow,
} from "./registry";
import { useDismissedToasts } from "./useDismissedToasts";

import { useAuth } from "~/auth";
import { useNotifications } from "~/contexts/notifications";
import { useConfigValues } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useDevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";
import { useTabs } from "~/store/zustand/tabs";
import { useToastAction } from "~/store/zustand/toast-action";

export function ToastArea() {
  const auth = useAuth();
  const { dismissToast, isDismissed } = useDismissedToasts();
  const shouldShowToast = useShouldShowToast();
  const contentOffset = useMainContentCenterOffset();
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
  const devtoolsPreview = useDevtoolsToastPreview((state) => state.preview);
  const clearDevtoolsPreview = useDevtoolsToastPreview(
    (state) => state.clearPreview,
  );
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
    openAiTab("intelligence");
  }, [openAiTab]);

  const handleOpenSTTSettings = useCallback(() => {
    setToastActionTarget("stt");
    openAiTab("transcription");
  }, [openAiTab, setToastActionTarget]);

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
        onSignIn: handleSignIn,
        onOpenLLMSettings: handleOpenLLMSettings,
        onOpenSTTSettings: handleOpenSTTSettings,
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
      handleSignIn,
      handleOpenLLMSettings,
      handleOpenSTTSettings,
    ],
  );

  const currentToast = useMemo(
    () => getToastToShow(registry, isDismissed),
    [registry, isDismissed],
  );

  const devtoolsToast = useMemo(
    () =>
      devtoolsPreview
        ? createDevtoolsToastPreview({
            preview: devtoolsPreview.type,
            onSignIn: handleSignIn,
            onOpenLLMSettings: handleOpenLLMSettings,
            onOpenSTTSettings: handleOpenSTTSettings,
          })
        : null,
    [
      devtoolsPreview,
      handleSignIn,
      handleOpenLLMSettings,
      handleOpenSTTSettings,
    ],
  );

  const handleDismiss = useCallback(() => {
    if (devtoolsToast) {
      clearDevtoolsPreview();
      return;
    }

    if (currentToast) {
      dismissToast(currentToast.id);
    }
  }, [clearDevtoolsPreview, currentToast, devtoolsToast, dismissToast]);

  const displayToast = devtoolsToast ?? currentToast;
  const displayToastKey =
    devtoolsPreview && devtoolsToast
      ? `${devtoolsToast.id}:${devtoolsPreview.key}`
      : displayToast?.id;

  const dismissAction = displayToast?.dismissible ? handleDismiss : undefined;

  if (!shouldShowToast || !displayToast) {
    return null;
  }

  return createPortal(
    <AnimatePresence mode="wait">
      <motion.div
        key={displayToastKey}
        initial={{ opacity: 0, y: -20, scale: 0.95 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: -20, scale: 0.95 }}
        transition={{ duration: 0.2, ease: "easeOut" }}
        style={{
          left: `calc(50% + ${contentOffset}px)`,
        }}
        className={cn([
          "fixed top-14 z-40 -translate-x-1/2",
          "pointer-events-none",
        ])}
      >
        <div className="pointer-events-auto">
          <Toast toast={displayToast} onDismiss={dismissAction} />
        </div>
      </motion.div>
    </AnimatePresence>,
    document.body,
  );
}

function useShouldShowToast() {
  const TOAST_CHECK_DELAY_MS = 500;

  const [showToast, setShowToast] = useState(false);

  useMountEffect(() => {
    const timer = setTimeout(() => {
      setShowToast(true);
    }, TOAST_CHECK_DELAY_MS);

    return () => clearTimeout(timer);
  });

  return showToast;
}

function useMainContentCenterOffset() {
  const [contentOffset, setContentOffset] = useState(0);

  useMountEffect(() => {
    const computeOffset = () => {
      const bodyPanel = document.querySelector("[data-panel-id]");
      if (!bodyPanel) {
        setContentOffset(0);
        return;
      }

      const bodyRect = bodyPanel.getBoundingClientRect();
      const bodyCenter = bodyRect.left + bodyRect.width / 2;
      const windowCenter = window.innerWidth / 2;
      setContentOffset(bodyCenter - windowCenter);
    };

    computeOffset();
    window.addEventListener("resize", computeOffset);

    const resizeObserver =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(computeOffset)
        : null;

    const panels = document.querySelectorAll("[data-panel-id]");
    for (const panel of panels) {
      resizeObserver?.observe(panel);
    }

    return () => {
      window.removeEventListener("resize", computeOffset);
      resizeObserver?.disconnect();
    };
  });

  return contentOffset;
}
