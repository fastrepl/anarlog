import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

import { Toast } from "./component";
import {
  createDevtoolsToastPreview,
  createToastRegistry,
  getToastToShow,
} from "./registry";
import { useTransientToast } from "./transient";
import { useDismissedToasts } from "./useDismissedToasts";

import { useAuth } from "~/auth";
import { useNotifications } from "~/contexts/notifications";
import { useConfigValues } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useDevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";
import { useTabs } from "~/store/zustand/tabs";
import { useToastAction } from "~/store/zustand/toast-action";

type ToastAreaPlacement = "default" | "left-sidebar";
type ToastAreaPosition = {
  left: number | string;
  top: number;
};

const DEFAULT_TOP_OFFSET_PX = 56;
const LEFT_SIDEBAR_TOP_OFFSET_PX = 36;
const MAIN_SURFACE_SELECTOR = "[data-chat-floating-anchor]";

export function ToastArea({
  placement = "default",
}: {
  placement?: ToastAreaPlacement;
}) {
  const auth = useAuth();
  const { dismissToast, isDismissed } = useDismissedToasts();
  const shouldShowToast = useShouldShowToast();
  const contentOffset = useMainContentCenterOffset();
  const mainSurfacePosition = useMainSurfaceToastPosition(
    placement === "left-sidebar",
  );
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
  const transientToast = useTransientToast((state) => state.toast);
  const clearTransientToast = useTransientToast((state) => state.clearToast);
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
    if (transientToast) {
      clearTransientToast(transientToast.key);
      return;
    }

    if (devtoolsToast) {
      clearDevtoolsPreview();
      return;
    }

    if (currentToast) {
      dismissToast(currentToast.id);
    }
  }, [
    clearDevtoolsPreview,
    clearTransientToast,
    currentToast,
    devtoolsToast,
    dismissToast,
    transientToast,
  ]);

  const displayToast = transientToast ?? devtoolsToast ?? currentToast;
  const displayToastKey =
    transientToast?.key ??
    (devtoolsPreview && devtoolsToast
      ? `${devtoolsToast.id}:${devtoolsPreview.key}`
      : displayToast?.id);

  const dismissAction = displayToast?.dismissible ? handleDismiss : undefined;
  const position =
    mainSurfacePosition ??
    getFallbackPosition({
      contentOffset,
      placement,
    });

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
          left: position.left,
          top: position.top,
        }}
        className={cn(["fixed z-40 -translate-x-1/2", "pointer-events-none"])}
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

function useMainSurfaceToastPosition(enabled: boolean) {
  const [position, setPosition] = useState<ToastAreaPosition | null>(null);

  useEffect(() => {
    if (!enabled) {
      setPosition(null);
      return;
    }

    const computePosition = () => {
      const mainSurface = document.querySelector(MAIN_SURFACE_SELECTOR);
      if (!mainSurface) {
        setPosition(null);
        return;
      }

      const rect = mainSurface.getBoundingClientRect();
      setPosition({
        left: rect.left + rect.width / 2,
        top: rect.top + LEFT_SIDEBAR_TOP_OFFSET_PX,
      });
    };

    computePosition();
    window.addEventListener("resize", computePosition);
    window.addEventListener("scroll", computePosition, true);

    const resizeObserver =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(computePosition)
        : null;

    const mainSurface = document.querySelector(MAIN_SURFACE_SELECTOR);
    if (mainSurface) {
      resizeObserver?.observe(mainSurface);
    }

    return () => {
      window.removeEventListener("resize", computePosition);
      window.removeEventListener("scroll", computePosition, true);
      resizeObserver?.disconnect();
    };
  }, [enabled]);

  return enabled ? position : null;
}

function getFallbackPosition({
  contentOffset,
  placement,
}: {
  contentOffset: number;
  placement: ToastAreaPlacement;
}): ToastAreaPosition {
  return {
    left: `calc(50% + ${contentOffset}px)`,
    top:
      placement === "left-sidebar"
        ? LEFT_SIDEBAR_TOP_OFFSET_PX
        : DEFAULT_TOP_OFFSET_PX,
  };
}
