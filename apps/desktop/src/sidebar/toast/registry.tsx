import type { ServerStatus } from "@hypr/plugin-local-stt";

import type { DownloadProgress, ToastCondition, ToastType } from "./types";

type ToastRegistryEntry = {
  toast: ToastType;
  condition: ToastCondition;
};

type ToastRegistryParams = {
  isAuthenticated: boolean;
  hasLLMConfigured: boolean;
  hasSttConfigured: boolean;
  hasProSttConfigured: boolean;
  hasProLlmConfigured: boolean;
  isAiTranscriptionTabActive: boolean;
  isAiIntelligenceTabActive: boolean;
  hasActiveDownload: boolean;
  downloadProgress: number | null;
  downloadingModel: string | null;
  activeDownloads: DownloadProgress[];
  localSttStatus: ServerStatus | null;
  isLocalSttModel: boolean;
  onSignIn: () => void | Promise<void>;
  onOpenLLMSettings: () => void;
  onOpenSTTSettings: () => void;
};

export function createToastRegistry({
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
  onSignIn,
  onOpenLLMSettings,
  onOpenSTTSettings,
}: ToastRegistryParams): ToastRegistryEntry[] {
  const downloadTitle =
    activeDownloads.length === 1
      ? `Downloading ${downloadingModel}`
      : `Downloading ${activeDownloads.length} models`;

  // order matters
  return [
    {
      toast: {
        id: "downloading-model",
        title: downloadTitle,
        description: "This may take a few minutes",
        dismissible: false,
        progress:
          activeDownloads.length === 1 ? (downloadProgress ?? 0) : undefined,
        downloads: activeDownloads.length > 1 ? activeDownloads : undefined,
      },
      condition: () => hasActiveDownload,
    },
    {
      toast: {
        id: "local-stt-loading",
        description: (
          <>
            <strong className="font-mono">Local transcription</strong> is
            starting up...
          </>
        ),
        dismissible: false,
      },
      condition: () =>
        isLocalSttModel && localSttStatus === "loading" && !hasActiveDownload,
    },
    {
      toast: {
        id: "local-stt-unreachable",
        description: (
          <>
            <strong className="text-red-600">Could not connect</strong> to the
            local speech-to-text model. Please check your settings.
          </>
        ),
        primaryAction: {
          label: "Check settings",
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
        description: (
          <>
            <strong className="font-mono">Transcription model</strong> is needed
            to make Char listen to your conversations.
          </>
        ),
        primaryAction: {
          label: "Configure transcription",
          onClick: onOpenSTTSettings,
        },
        dismissible: false,
      },
      condition: () => !hasSttConfigured && !isAiTranscriptionTabActive,
    },
    {
      toast: {
        id: "missing-llm",
        description: (
          <>
            <strong className="font-mono">Language model</strong> is needed to
            make Char summarize and chat about your conversations.
          </>
        ),
        primaryAction: {
          label: "Add intelligence",
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
            src="/assets/hyprnote-pro.png"
            alt="Char Pro"
            className="size-5"
          />
        ),
        title: "Sign in required",
        description:
          "You have Char Pro models configured. Please sign in to use them.",
        primaryAction: {
          label: "Sign in",
          onClick: onSignIn,
        },
        dismissible: true,
      },
      condition: () =>
        !isAuthenticated && (hasProSttConfigured || hasProLlmConfigured),
    },
    {
      toast: {
        id: "upgrade-to-pro",
        icon: (
          <img
            src="/assets/hyprnote-pro.png"
            alt="Char Pro"
            className="size-5"
          />
        ),
        title: "Keep the magic going",
        description:
          "Transcription stays free. Pro unlocks other magic you'll love.",
        primaryAction: {
          label: "Upgrade to Pro",
          onClick: onSignIn,
        },
        dismissible: true,
      },
      condition: () =>
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
