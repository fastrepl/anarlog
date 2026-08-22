import { t } from "@lingui/core/macro";

import type { ServerStatus } from "@anlg/plugin-local-stt";

import type { DownloadProgress, ToastCondition, ToastType } from "./types";

import type { DesktopUpdateControl } from "~/main/update-banner";
import type { DevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";

const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";
const DESKTOP_UPDATE_TOAST_PREFIX = "desktop-update:";

type ToastRegistryEntry = {
  toast: ToastType;
  condition: ToastCondition;
};

type ToastRegistryParams = {
  isAuthenticated: boolean;
  isAuthLoading: boolean;
  hasLLMConfigured: boolean;
  hasSttConfigured: boolean;
  hasProSttConfigured: boolean;
  hasProLlmConfigured: boolean;
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
  onSignIn: () => void | Promise<void>;
  onOpenLLMSettings: () => void;
  onOpenSTTSettings: () => void;
};

type DevtoolsToastPreviewParams = {
  preview: DevtoolsToastPreview;
  onSignIn: () => void | Promise<void>;
  onOpenLLMSettings: () => void;
  onOpenSTTSettings: () => void;
};

export function createToastRegistry({
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
  onSignIn,
  onOpenLLMSettings,
  onOpenSTTSettings,
}: ToastRegistryParams): ToastRegistryEntry[] {
  const downloadTitle =
    activeDownloads.length === 1 && downloadingModel
      ? t`Downloading ${downloadingModel}`
      : t`Downloading ${activeDownloads.length} models`;
  const hasUsableSttConfigured =
    hasSttConfigured &&
    (isAuthLoading || isAuthenticated || !hasProSttConfigured);
  const hasUsableLlmConfigured =
    hasLLMConfigured &&
    (isAuthLoading || isAuthenticated || !hasProLlmConfigured);
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
        id: "sign-in-benefits",
        icon: (
          <img
            src={ANARLOG_ICON_SRC}
            alt="Anarlog"
            className="size-5 object-contain object-center"
          />
        ),
        description: t`Sign in to get the most out of Anarlog`,
        primaryAction: {
          label: t`Sign in`,
          onClick: onSignIn,
        },
        lifecycle: {
          type: "persistent",
          dismissal: "permanent",
          dismissalId: "auth-promotion",
        },
      },
      condition: () => !isAuthLoading && !isAuthenticated,
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
    {
      toast: {
        id: "upgrade-to-pro",
        description: t`Pro features available`,
        primaryAction: {
          label: t`Upgrade`,
          onClick: onSignIn,
        },
        lifecycle: {
          type: "persistent",
          dismissal: "permanent",
          dismissalId: "auth-promotion",
        },
      },
      // suppress until auth resolves to avoid flash on startup
      condition: () =>
        !isAuthLoading &&
        !isAuthenticated &&
        hasLLMConfigured &&
        hasSttConfigured &&
        !hasProSttConfigured &&
        !hasProLlmConfigured,
    },
  ];
}

export function createDesktopUpdateToast(
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
      description: t`Anarlog ${update.version} is ready to install`,
      primaryAction: busy
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
      description: t`Downloading Anarlog ${update.version}${progress}`,
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
    description: t`Anarlog ${update.version} is available`,
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
  onSignIn,
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
    case "pro":
      return {
        id: "devtools-upgrade-to-pro",
        description: t`Pro features available`,
        primaryAction: {
          label: t`Upgrade`,
          onClick: onSignIn,
        },
        lifecycle: { type: "persistent", dismissal: "session" },
      };
  }
}
