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
  cloudsyncInitialSyncToastId: string | null;
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
  cloudsyncInitialSyncToastId,
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
      ? `Downloading ${downloadingModel}`
      : `Downloading ${activeDownloads.length} models`;
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
        dismissible: false,
        loading: true,
      },
      condition: () => hasActiveDownload,
    },
    {
      toast: {
        id: cloudsyncInitialSyncToastId ?? "cloudsync-initial-sync",
        description: "Syncing your data in the background...",
        dismissible: true,
        loading: true,
      },
      condition: () => cloudsyncInitialSyncToastId !== null,
    },
    ...(updateToast
      ? [
          {
            toast: updateToast,
            condition: () => true,
          },
        ]
      : []),
    {
      toast: {
        id: "local-stt-loading",
        description: "Starting transcription...",
        dismissible: false,
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
        description: "Transcription unavailable",
        primaryAction: {
          label: "Settings",
          onClick: onOpenSTTSettings,
        },
        dismissible: true,
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
        description: "Sign in to get the most out of Anarlog",
        primaryAction: {
          label: "Sign in",
          onClick: onSignIn,
        },
        dismissible: true,
      },
      condition: () => !isAuthLoading && !isAuthenticated,
    },
    {
      toast: {
        id: "missing-stt",
        description: "Transcription provider needed",
        primaryAction: {
          label: "Add",
          onClick: onOpenSTTSettings,
        },
        dismissible: false,
      },
      condition: () => !hasUsableSttConfigured && !isAiTranscriptionTabActive,
    },
    {
      toast: {
        id: "missing-llm",
        description: "Language model needed",
        primaryAction: {
          label: "Add",
          onClick: onOpenLLMSettings,
        },
        dismissible: true,
      },
      condition: () =>
        hasUsableSttConfigured &&
        !hasUsableLlmConfigured &&
        !isAiIntelligenceTabActive,
    },
    {
      toast: {
        id: "upgrade-to-pro",
        description: "Pro features available",
        primaryAction: {
          label: "Upgrade",
          onClick: onSignIn,
        },
        dismissible: true,
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

export function isDesktopUpdateToastId(id: string): boolean {
  return id.startsWith(DESKTOP_UPDATE_TOAST_PREFIX);
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
      id,
      description: `Anarlog ${update.version} is ready to install`,
      primaryAction: busy
        ? undefined
        : { label: "Restart", onClick: update.installUpdate },
      dismissible: true,
      loading: update.installing,
    };
  }

  if (update.status === "downloading" || update.downloadStarting) {
    const progress =
      update.progress === null
        ? ""
        : ` (${Math.round(update.progress * 100)}%)`;
    return {
      id,
      description: `Downloading Anarlog ${update.version}${progress}`,
      dismissible: true,
      loading: true,
    };
  }

  if (update.status === "failed") {
    return {
      id,
      description: update.errorMessage || "The update download failed",
      primaryAction: busy
        ? undefined
        : { label: "Retry", onClick: update.downloadUpdate },
      dismissible: true,
      variant: "error",
    };
  }

  return {
    id,
    description: `Anarlog ${update.version} is available`,
    primaryAction: busy
      ? undefined
      : { label: "Download", onClick: update.downloadUpdate },
    dismissible: true,
    loading: update.downloadStarting,
  };
}

export function getToastToShow(
  registry: ToastRegistryEntry[],
  isDismissed: (id: string) => boolean,
): ToastType | null {
  for (const entry of registry) {
    if (entry.condition() && !isDismissed(entry.toast.id)) {
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
        description: "Language model needed",
        primaryAction: {
          label: "Add",
          onClick: onOpenLLMSettings,
        },
        dismissible: true,
      };
    case "transcription-model":
      return {
        id: "devtools-missing-stt",
        description: "Transcription provider needed",
        primaryAction: {
          label: "Add",
          onClick: onOpenSTTSettings,
        },
        dismissible: false,
      };
    case "transcription-error":
      return {
        id: "devtools-local-stt-unreachable",
        description: "Transcription unavailable",
        primaryAction: {
          label: "Settings",
          onClick: onOpenSTTSettings,
        },
        dismissible: true,
        variant: "error",
      };
    case "download":
      return {
        id: "devtools-downloading-model",
        description: "Downloading model",
        dismissible: false,
        loading: true,
      };
    case "pro":
      return {
        id: "devtools-upgrade-to-pro",
        description: "Pro features available",
        primaryAction: {
          label: "Upgrade",
          onClick: onSignIn,
        },
        dismissible: true,
      };
  }
}
