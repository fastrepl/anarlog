import { useQuery } from "@tanstack/react-query";
import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import {
  commands as localSttCommands,
  events as localSttEvents,
  type ServerStatus,
  type LocalModel,
} from "@hypr/plugin-local-stt";

import { useConfigValues } from "~/shared/config";
import type { DownloadProgress } from "~/sidebar/toast/types";
import { useTabs } from "~/store/zustand/tabs";

interface NotificationState {
  hasActiveBanner: boolean;
  hasActiveEnhancement: boolean;
  hasActiveDownload: boolean;
  downloadProgress: number | null;
  downloadingModel: string | null;
  activeDownloads: DownloadProgress[];
  notificationCount: number;
  shouldShowBadge: boolean;
  localSttStatus: ServerStatus | null;
  isLocalSttModel: boolean;
}

const NotificationContext = createContext<NotificationState | null>(null);

const MODEL_DISPLAY_NAMES: Partial<Record<LocalModel, string>> = {
  "am-parakeet-v2": "Parakeet v2",
  "am-parakeet-v3": "Parakeet v3",
  "am-whisper-large-v3": "Whisper Large v3",
  QuantizedTinyEn: "Whisper Tiny (English)",
  QuantizedSmallEn: "Whisper Small (English)",
};

export function NotificationProvider({ children }: { children: ReactNode }) {
  const {
    current_stt_provider,
    current_stt_model,
    current_llm_provider,
    current_llm_model,
  } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
    "current_llm_provider",
    "current_llm_model",
  ] as const);

  const hasConfigBanner =
    !current_stt_provider ||
    !current_stt_model ||
    !current_llm_provider ||
    !current_llm_model;

  const sttModel = current_stt_model as string | undefined;
  const isLocalSttModel =
    current_stt_provider === "hyprnote" && !!sttModel && sttModel !== "cloud";

  const localSttQuery = useQuery({
    enabled: isLocalSttModel,
    queryKey: ["local-stt-status", sttModel],
    refetchInterval: 1000,
    queryFn: async () => {
      if (!sttModel) return null;

      const serverResult = await localSttCommands.getServerForModel(
        sttModel as LocalModel,
      );
      if (serverResult.status !== "ok") return null;

      return serverResult.data?.status ?? null;
    },
  });

  const localSttStatus = isLocalSttModel ? (localSttQuery.data ?? null) : null;

  const [activeDownloads, setActiveDownloads] = useState<
    Map<LocalModel, number>
  >(new Map());

  useEffect(() => {
    const unlisten = localSttEvents.downloadProgressPayload.listen((event) => {
      const { model: eventModel, progress: eventProgress } = event.payload;

      setActiveDownloads((prev) => {
        const next = new Map(prev);
        if (eventProgress < 0 || eventProgress >= 100) {
          next.delete(eventModel);
        } else {
          next.set(eventModel, Math.max(0, Math.min(100, eventProgress)));
        }
        return next;
      });
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const hasActiveEnhancement = false;

  const currentTab = useTabs(
    (state: {
      currentTab: ReturnType<typeof useTabs.getState>["currentTab"];
    }) => state.currentTab,
  );
  const isAiTab = currentTab?.type === "ai";

  const value = useMemo<NotificationState>(() => {
    const hasActiveBanner = hasConfigBanner && !isAiTab;
    const hasActiveDownload = activeDownloads.size > 0;

    const downloadsArray: DownloadProgress[] = Array.from(
      activeDownloads.entries(),
    ).map(([model, progress]) => ({
      model,
      displayName: MODEL_DISPLAY_NAMES[model] ?? model,
      progress,
    }));

    const firstDownload = downloadsArray[0];
    const downloadProgress = firstDownload?.progress ?? null;
    const downloadingModel = firstDownload?.displayName ?? null;

    const notificationCount =
      (hasActiveBanner ? 1 : 0) +
      (hasActiveEnhancement ? 1 : 0) +
      (hasActiveDownload ? 1 : 0);

    return {
      hasActiveBanner,
      hasActiveEnhancement,
      hasActiveDownload,
      downloadProgress,
      downloadingModel,
      activeDownloads: downloadsArray,
      notificationCount,
      shouldShowBadge: notificationCount > 0,
      localSttStatus,
      isLocalSttModel,
    };
  }, [
    hasConfigBanner,
    hasActiveEnhancement,
    activeDownloads,
    isAiTab,
    localSttStatus,
    isLocalSttModel,
  ]);

  return (
    <NotificationContext.Provider value={value}>
      {children}
    </NotificationContext.Provider>
  );
}

const DEFAULT_NOTIFICATION_STATE: NotificationState = {
  hasActiveBanner: false,
  hasActiveEnhancement: false,
  hasActiveDownload: false,
  downloadProgress: null,
  downloadingModel: null,
  activeDownloads: [],
  notificationCount: 0,
  shouldShowBadge: false,
  localSttStatus: null,
  isLocalSttModel: false,
};

export function useNotifications() {
  const context = useContext(NotificationContext);
  return context ?? DEFAULT_NOTIFICATION_STATE;
}
