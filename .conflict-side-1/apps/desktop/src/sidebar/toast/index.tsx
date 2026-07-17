import { useCallback, useMemo, useState } from "react";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  createDevtoolsToastPreview,
  createToastRegistry,
  getToastToShow,
} from "./registry";
import type { ToastType } from "./types";
import { useDismissedToasts } from "./useDismissedToasts";

import { useAuth } from "~/auth";
import { useCloudsyncInitialSyncProgress } from "~/auth/cloudsync-progress";
import { useNotifications } from "~/contexts/notifications";
import { useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useDevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";
import { useTabs } from "~/store/zustand/tabs";
import { useToastAction } from "~/store/zustand/toast-action";
import {
  isConfiguredSttModel,
  isHyprnoteCloudSttModel,
} from "~/stt/capabilities";
import { useListener } from "~/stt/contexts";

export function ToastNotifications() {
  const auth = useAuth();
  const cloudsyncProgress = useCloudsyncInitialSyncProgress();
  const { dismissToast, isDismissed } = useDismissedToasts();
  const shouldShowToast = useShouldShowToast();
  const {
    hasActiveDownload,
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
  const hasSttConfigured = isConfiguredSttModel(
    current_stt_provider,
    current_stt_model,
  );
  const hasProSttConfigured = isHyprnoteCloudSttModel(
    current_stt_provider,
    current_stt_model,
  );
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
  const activeTranscriptSessionId =
    currentTab?.type === "sessions" &&
    currentTab.state.view?.type === "transcript"
      ? currentTab.id
      : null;
  const isBatchTranscribingInActiveTranscriptTab = useListener((state) =>
    activeTranscriptSessionId
      ? state.getSessionMode(activeTranscriptSessionId) === "running_batch"
      : false,
  );

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
        isBatchTranscribingInActiveTranscriptTab,
        cloudsyncInitialSyncToastId:
          cloudsyncProgress.state === "syncing"
            ? cloudsyncProgress.toastId
            : null,
        hasActiveDownload,
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
      isBatchTranscribingInActiveTranscriptTab,
      cloudsyncProgress,
      hasActiveDownload,
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

  const registryPriorityToast =
    currentToast?.id === "downloading-model" ? currentToast : null;
  const displayToast = registryPriorityToast ?? devtoolsToast ?? currentToast;

  const handleDismiss = useCallback(() => {
    if (devtoolsToast) {
      clearDevtoolsPreview();
      return;
    }

    if (currentToast) {
      dismissToast(currentToast.id);
    }
  }, [clearDevtoolsPreview, currentToast, devtoolsToast, dismissToast]);

  if (!shouldShowToast || !displayToast) {
    return null;
  }

  const descriptionKey =
    typeof displayToast.description === "string"
      ? displayToast.description
      : displayToast.id;
  const previewKey =
    devtoolsPreview && devtoolsToast
      ? `${devtoolsToast.id}:${devtoolsPreview.key}`
      : `${displayToast.id}:${descriptionKey}`;

  return (
    <SonnerNotification
      key={previewKey}
      toast={displayToast}
      onDismiss={displayToast.dismissible ? handleDismiss : undefined}
    />
  );
}

function SonnerNotification({
  toast,
  onDismiss,
}: {
  toast: ToastType;
  onDismiss?: () => void;
}) {
  const toastRef = useLatestRef(toast);
  const onDismissRef = useLatestRef(onDismiss);

  useMountEffect(() => {
    let shouldPersistDismissal = true;
    const options = {
      id: toast.id,
      duration: Infinity,
      closeButton: toast.dismissible,
      icon: toast.icon,
      action: toast.primaryAction
        ? {
            label: toast.primaryAction.label,
            onClick: () => {
              shouldPersistDismissal = false;
              void toastRef.current.primaryAction?.onClick();
            },
          }
        : undefined,
      onDismiss: () => {
        if (shouldPersistDismissal) {
          onDismissRef.current?.();
        }
      },
    };

    if (toast.loading) {
      sonnerToast.loading(toast.description, options);
    } else if (toast.variant === "error") {
      sonnerToast.error(toast.description, options);
    } else if (toast.variant === "warning") {
      sonnerToast.warning(toast.description, options);
    } else {
      sonnerToast.message(toast.description, options);
    }

    return () => {
      shouldPersistDismissal = false;
      sonnerToast.dismiss(toast.id);
    };
  });

  return null;
}

function useShouldShowToast() {
  const [showToast, setShowToast] = useState(false);

  useMountEffect(() => {
    const timer = setTimeout(() => {
      setShowToast(true);
    }, 500);

    return () => clearTimeout(timer);
  });

  return showToast;
}
