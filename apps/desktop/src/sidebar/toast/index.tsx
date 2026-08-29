import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { sonnerToast, TOAST_DURATIONS } from "@anlg/ui/components/ui/toast";

import {
  createDevtoolsToastPreview,
  createToastRegistry,
  getToastToShow,
} from "./registry";
import type { ToastType } from "./types";
import { useDismissedToasts } from "./useDismissedToasts";

import { useAuth } from "~/auth";
import { useNotifications } from "~/contexts/notifications";
import { useDesktopUpdateControl } from "~/main/update-banner";
import { useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useDevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";
import { useTabs } from "~/store/zustand/tabs";
import { useToastAction } from "~/store/zustand/toast-action";
import {
  isConfiguredSttModel,
  isAnarlogCloudSttModel,
} from "~/stt/capabilities";
import { useListener } from "~/stt/contexts";

export function ToastNotifications() {
  const auth = useAuth();
  const { dismissToast, isDismissed } = useDismissedToasts();
  const [sessionDismissedToastIds, setSessionDismissedToastIds] = useState(
    () => new Set<string>(),
  );
  const shouldShowToast = useShouldShowToast();
  const {
    hasActiveDownload,
    downloadingModel,
    activeDownloads,
    localSttStatus,
    isLocalSttModel,
  } = useNotifications();
  const update = useDesktopUpdateControl();
  const [observedUpdateStatus, setObservedUpdateStatus] = useState(
    update.status,
  );
  if (observedUpdateStatus !== update.status) {
    setObservedUpdateStatus(update.status);
    if (observedUpdateStatus === "failed") {
      setSessionDismissedToastIds(
        (current) =>
          new Set([...current].filter((id) => !id.endsWith(":failed"))),
      );
    }
  }

  useEffect(() => {
    if (hasActiveDownload) {
      return;
    }

    setSessionDismissedToastIds((current) => {
      if (!current.has("downloading-model")) {
        return current;
      }

      const next = new Set(current);
      next.delete("downloading-model");
      return next;
    });
  }, [hasActiveDownload]);

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
  const hasProSttConfigured = isAnarlogCloudSttModel(
    current_stt_provider,
    current_stt_model,
  );
  const hasProLlmConfigured = current_llm_provider === "anarlog";

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
  const activeLiveSessionId = useListener((state) =>
    state.live.status === "active" || state.live.status === "finalizing"
      ? state.live.sessionId
      : null,
  );
  const isLiveMeetingActive = activeLiveSessionId !== null;

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
        isLiveMeetingActive,
        hasActiveDownload,
        downloadingModel,
        activeDownloads,
        localSttStatus,
        isLocalSttModel,
        update,
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
      isLiveMeetingActive,
      hasActiveDownload,
      downloadingModel,
      activeDownloads,
      localSttStatus,
      isLocalSttModel,
      update,
      handleSignIn,
      handleOpenLLMSettings,
      handleOpenSTTSettings,
    ],
  );

  const isToastDismissed = useCallback(
    (toast: ToastType) => {
      if (toast.lifecycle.type === "condition-bound") {
        return false;
      }

      const dismissalId = toast.lifecycle.dismissalId ?? toast.id;
      if (toast.lifecycle.dismissal === "permanent") {
        return isDismissed(dismissalId);
      }
      if (sessionDismissedToastIds.has(dismissalId)) {
        return true;
      }
      return (
        toast.lifecycle.dismissal === "day" &&
        hasActiveDayToastSnooze(dismissalId)
      );
    },
    [isDismissed, sessionDismissedToastIds],
  );

  const currentToast = useMemo(
    () => getToastToShow(registry, isToastDismissed),
    [registry, isToastDismissed],
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
      if (currentToast.lifecycle.type === "condition-bound") {
        return;
      }

      const dismissalId = currentToast.lifecycle.dismissalId ?? currentToast.id;
      if (currentToast.lifecycle.dismissal === "permanent") {
        dismissToast(dismissalId);
        return;
      }
      if (currentToast.lifecycle.dismissal === "day") {
        saveDayToastSnooze(dismissalId);
        return;
      }
      setSessionDismissedToastIds((current) =>
        new Set(current).add(dismissalId),
      );
    }
  }, [clearDevtoolsPreview, currentToast, devtoolsToast, dismissToast]);

  if (!shouldShowToast || !displayToast) {
    return null;
  }

  const previewKey =
    devtoolsPreview && devtoolsToast
      ? `${devtoolsToast.id}:${devtoolsPreview.key}`
      : displayToast.id;

  return (
    <SonnerNotification
      key={previewKey}
      toast={displayToast}
      onDismiss={
        displayToast.lifecycle.type === "persistent" ? handleDismiss : undefined
      }
    />
  );
}

function toastPresentation(toast: ToastType) {
  const description =
    typeof toast.description === "string" ? toast.description : toast.id;
  return [
    toast.id,
    description,
    toast.variant ?? "default",
    toast.loading ? "loading" : "idle",
    toast.primaryAction?.label ?? "",
  ].join(":");
}

function showSonnerNotification(
  toast: ToastType,
  toastRef: { current: ToastType },
  onDismissRef: { current?: () => void },
  dismissal: { persist: boolean },
) {
  const dismissible = toast.lifecycle.type === "persistent";
  const options = {
    id: toast.id,
    duration: toast.variant === "error" ? TOAST_DURATIONS.error : Infinity,
    closeButton: dismissible,
    dismissible,
    icon: toast.icon,
    action: toast.primaryAction
      ? {
          label: toast.primaryAction.label,
          onClick: () => {
            dismissal.persist = false;
            void toastRef.current.primaryAction?.onClick();
          },
        }
      : undefined,
    onDismiss: () => {
      if (dismissal.persist) {
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
  const dismissalRef = useRef({ persist: true });
  const shownPresentationRef = useRef<string | null>(null);
  const presentation = toastPresentation(toast);

  if (
    shownPresentationRef.current !== null &&
    shownPresentationRef.current !== presentation
  ) {
    shownPresentationRef.current = presentation;
    showSonnerNotification(toast, toastRef, onDismissRef, dismissalRef.current);
  }

  useMountEffect(() => {
    dismissalRef.current.persist = true;
    shownPresentationRef.current = toastPresentation(toast);
    showSonnerNotification(toast, toastRef, onDismissRef, dismissalRef.current);

    return () => {
      dismissalRef.current.persist = false;
      sonnerToast.dismiss(toast.id);
    };
  });

  return null;
}

const DAY_TOAST_SNOOZE_MS = 24 * 60 * 60 * 1_000;
const DAY_TOAST_SNOOZE_KEY_PREFIX = "anarlog:toast:snoozed-until:";

function hasActiveDayToastSnooze(id: string): boolean {
  try {
    const storageKey = `${DAY_TOAST_SNOOZE_KEY_PREFIX}${id}`;
    const snoozedUntil = Number(localStorage.getItem(storageKey));
    if (Number.isFinite(snoozedUntil) && snoozedUntil > Date.now()) {
      return true;
    }
    localStorage.removeItem(storageKey);
  } catch {
    return false;
  }
  return false;
}

function saveDayToastSnooze(id: string) {
  try {
    localStorage.setItem(
      `${DAY_TOAST_SNOOZE_KEY_PREFIX}${id}`,
      String(Date.now() + DAY_TOAST_SNOOZE_MS),
    );
  } catch {
    return;
  }
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
