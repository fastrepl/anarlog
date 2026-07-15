import type { ServerStatus } from "@hypr/plugin-local-stt";

import type { DownloadProgress, ToastCondition, ToastType } from "./types";

import type { DevtoolsToastPreview } from "~/store/zustand/devtools-toast-preview";

const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

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
  hasActiveDownload: boolean;
  downloadingModel: string | null;
  activeDownloads: DownloadProgress[];
  localSttStatus: ServerStatus | null;
  isLocalSttModel: boolean;
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
  hasActiveDownload,
  downloadingModel,
  activeDownloads,
  localSttStatus,
  isLocalSttModel,
  onSignIn,
  onOpenLLMSettings,
  onOpenSTTSettings,
}: ToastRegistryParams): ToastRegistryEntry[] {
  const downloadTitle =
    activeDownloads.length === 1 && downloadingModel
      ? `Downloading ${downloadingModel}`
      : `Downloading ${activeDownloads.length} models`;

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
        id: "missing-stt",
        description: "Transcription model needed",
        primaryAction: {
          label: "Add",
          onClick: onOpenSTTSettings,
        },
        dismissible: false,
      },
      condition: () => !hasSttConfigured && !isAiTranscriptionTabActive,
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
        hasSttConfigured && !hasLLMConfigured && !isAiIntelligenceTabActive,
    },
    {
      toast: {
        id: "pro-requires-login",
        icon: (
          <img
            src={ANARLOG_ICON_SRC}
            alt="Anarlog Pro"
            className="size-5 object-contain object-center"
          />
        ),
        description: "Sign in required",
        primaryAction: {
          label: "Sign in",
          onClick: onSignIn,
        },
        dismissible: true,
      },
      // suppress until auth resolves to avoid flash on startup
      condition: () =>
        !isAuthLoading &&
        !isAuthenticated &&
        (hasProSttConfigured || hasProLlmConfigured),
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
        description: "Transcription model needed",
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
