import { t } from "@lingui/core/macro";

import type { ServerStatus } from "@anlg/plugin-local-stt";

import type { DownloadProgress, ToastCondition, ToastType } from "./types";

import type { DesktopUpdateControl } from "~/main/update-banner";
import type { DevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";

const DESKTOP_UPDATE_TOAST_PREFIX = "desktop-update:";

type ToastRegistryEntry = {
  toast: ToastType;
  condition: ToastCondition;
};

type ToastRegistryParams = {
  hasLLMConfigured: boolean;
  hasSttConfigured: boolean;
  isAiTranscriptionTabActive: boolean;
  isAiIntelligenceTabActive: boolean;
  isBatchTranscribingInActiveTranscriptTab: boolean;
  isLiveMeetingActive: boolean;
  hasActiveDownload: boolean;
  downloadingModel: string | null;
  activeDownloads: DownloadProgress[];
  localSttStatus: ServerStatus | null;
  isLocalSttModel: boolean;
  update: DesktopUpdateControl;
  onOpenLLMSettings: () => void;
  onOpenSTTSettings: () => void;
};

type DevtoolsToastPreviewParams = {
  preview: DevtoolsToastPreview;
  onOpenLLMSettings: () => void;
  onOpenSTTSettings: () => void;
};

export function createToastRegistry({
  hasLLMConfigured,
  hasSttConfigured,
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
  onOpenLLMSettings,
  onOpenSTTSettings,
}: ToastRegistryParams): ToastRegistryEntry[] {
  const downloadTitle =
    activeDownloads.length === 1 && downloadingModel
      ? t`Downloading ${downloadingModel}`
      : t`Downloading ${activeDownloads.length} models`;
  const hasUsableSttConfigured = hasSttConfigured;
  const hasUsableLlmConfigured = hasLLMConfigured;
  const updateToast = createDesktopUpdateToast(update);

  // order matters
  return [
    {
      toast: {
        id: "downloading-model",
        description: downloadTitle,
        lifecycle: { type: "persistent", dismissal: "session" },
        loading: true,
      },
      condition: () => hasActiveDownload,
    },
    ...(updateToast
      ? [
          {
            toast: updateToast,
            // Never show update prompts mid-meeting; they resurface after.
            condition: () => !isLiveMeetingActive,
          },
        ]
      : []),
    {
      toast: {
        id: "local-stt-loading",
        description: t`Starting transcription...`,
        lifecycle: { type: "condition-bound" },
        loading: true,
      },
      condition: () =>
        isLocalSttModel &&
        localSttStatus === "loading" &&
        !hasActiveDownload &&
        !isBatchTranscribingInActiveTranscriptTab,
    },
    {
      toast: {
        id: "local-stt-unreachable",
        description: t`Transcription unavailable`,
        primaryAction: {
          label: t`Settings`,
          onClick: onOpenSTTSettings,
        },
        lifecycle: { type: "condition-bound" },
        variant: "error",
      },
      condition: () =>
        isLocalSttModel &&
        localSttStatus === "unreachable" &&
        !hasActiveDownload &&
        !isAiTranscriptionTabActive,
    },
    {
      toast: {
        id: "missing-stt",
        description: t`Transcription provider needed`,
        primaryAction: {
          label: t`Add`,
          onClick: onOpenSTTSettings,
        },
        lifecycle: { type: "condition-bound" },
      },
      condition: () => !hasUsableSttConfigured && !isAiTranscriptionTabActive,
    },
    {
      toast: {
        id: "missing-llm",
        description: t`Language model needed`,
        primaryAction: {
          label: t`Add`,
          onClick: onOpenLLMSettings,
        },
        lifecycle: { type: "condition-bound" },
      },
      condition: () =>
        hasUsableSttConfigured &&
        !hasUsableLlmConfigured &&
        !isAiIntelligenceTabActive,
    },
  ];
}

function createDesktopUpdateToast(
  update: DesktopUpdateControl,
): ToastType | null {
  if (!update.status || !update.version) {
    return null;
  }

  const id = `${DESKTOP_UPDATE_TOAST_PREFIX}${update.version}`;
  const busy =
    update.status === "downloading" ||
    update.downloadStarting ||
    update.installing;

  if (update.status === "ready") {
    return {
      // A new ID prevents Sonner from retaining the loading state used while
      // this update was downloading.
      id: `${id}:ready`,
      description: t`BlackMushi ${update.version} is ready to install`,
      primaryAction: update.installing
        ? undefined
        : { label: t`Restart`, onClick: update.installUpdate },
      lifecycle: { type: "persistent", dismissal: "session" },
    };
  }

  if (update.status === "downloading" || update.downloadStarting) {
    const progress =
      update.progress === null
        ? ""
        : ` (${Math.round(update.progress * 100)}%)`;
    return {
      id: `${id}:downloading`,
      description: t`Downloading BlackMushi ${update.version}${progress}`,
      lifecycle: { type: "persistent", dismissal: "session" },
      loading: true,
    };
  }

  if (update.status === "failed") {
    return {
      id: `${id}:failed`,
      description: update.errorMessage || t`The update download failed`,
      primaryAction: busy
        ? undefined
        : { label: t`Retry`, onClick: update.downloadUpdate },
      lifecycle: { type: "persistent", dismissal: "session" },
      variant: "error",
    };
  }

  return {
    id: `${id}:available`,
    description: t`BlackMushi ${update.version} is available`,
    primaryAction: busy
      ? undefined
      : { label: t`Download`, onClick: update.downloadUpdate },
    lifecycle: { type: "persistent", dismissal: "day" },
  };
}

export function getToastToShow(
  registry: ToastRegistryEntry[],
  isDismissed: (toast: ToastType) => boolean,
): ToastType | null {
  for (const entry of registry) {
    if (
      entry.condition() &&
      (entry.toast.lifecycle.type === "condition-bound" ||
        !isDismissed(entry.toast))
    ) {
      return entry.toast;
    }
  }
  return null;
}

export function createDevtoolsToastPreview({
  preview,
  onOpenLLMSettings,
  onOpenSTTSettings,
}: DevtoolsToastPreviewParams): ToastType {
  switch (preview) {
    case "language-model":
      return {
        id: "devtools-missing-llm",
        description: t`Language model needed`,
        primaryAction: {
          label: t`Add`,
          onClick: onOpenLLMSettings,
        },
        lifecycle: { type: "condition-bound" },
      };
    case "transcription-model":
      return {
        id: "devtools-missing-stt",
        description: t`Transcription provider needed`,
        primaryAction: {
          label: t`Add`,
          onClick: onOpenSTTSettings,
        },
        lifecycle: { type: "condition-bound" },
      };
    case "transcription-error":
      return {
        id: "devtools-local-stt-unreachable",
        description: t`Transcription unavailable`,
        primaryAction: {
          label: t`Settings`,
          onClick: onOpenSTTSettings,
        },
        lifecycle: { type: "condition-bound" },
        variant: "error",
      };
    case "download":
      return {
        id: "devtools-downloading-model",
        description: t`Downloading model`,
        lifecycle: { type: "persistent", dismissal: "session" },
        loading: true,
      };
  }
}
