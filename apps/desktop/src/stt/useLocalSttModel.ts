import { queryOptions } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";

import {
  commands as localSttCommands,
  events as localSttEvents,
  type LocalModel,
} from "@hypr/plugin-local-stt";

export const localSttKeys = {
  all: ["local-stt"] as const,
  models: () => [...localSttKeys.all, "model"] as const,
  model: (model: LocalModel) => [...localSttKeys.models(), model] as const,
  modelDownloaded: (model: LocalModel) =>
    [...localSttKeys.model(model), "downloaded"] as const,
  modelDownloading: (model: LocalModel) =>
    [...localSttKeys.model(model), "downloading"] as const,
};

export const localSttQueries = {
  supportedModels: () =>
    queryOptions({
      queryKey: [...localSttKeys.all, "supported-models"] as const,
      queryFn: () => localSttCommands.listSupportedModels(),
      staleTime: Infinity,
      select: (result) => {
        if (result.status === "error") {
          throw new Error(result.error);
        }
        return result.data;
      },
    }),
  isDownloaded: (model: LocalModel) =>
    queryOptions({
      refetchInterval: 1000,
      queryKey: localSttKeys.modelDownloaded(model),
      queryFn: () => localSttCommands.isModelDownloaded(model),
      select: (result) => {
        if (result.status === "error") {
          throw new Error(result.error);
        }
        return result.data;
      },
    }),
  isDownloading: (model: LocalModel) =>
    queryOptions({
      refetchInterval: 1000,
      queryKey: localSttKeys.modelDownloading(model),
      queryFn: () => localSttCommands.isModelDownloading(model),
      select: (result) => {
        if (result.status === "error") {
          throw new Error(result.error);
        }
        return result.data;
      },
    }),
};

export function useLocalModelDownload(
  model: LocalModel,
  onDownloadComplete?: (model: LocalModel) => void,
) {
  const [progress, setProgress] = useState<number>(0);
  const [isStarting, setIsStarting] = useState(false);
  const [hasError, setHasError] = useState(false);

  const isDownloaded = useQuery(localSttQueries.isDownloaded(model));
  const isDownloading = useQuery(localSttQueries.isDownloading(model));

  const showProgress =
    !isDownloaded.data && (isStarting || (isDownloading.data ?? false));

  useEffect(() => {
    if (isDownloading.data) {
      setIsStarting(false);
    }
  }, [isDownloading.data]);

  useEffect(() => {
    const unlisten = localSttEvents.downloadProgressPayload.listen((event) => {
      if (event.payload.model === model) {
        if (event.payload.progress < 0) {
          setHasError(true);
          setIsStarting(false);
          setProgress(0);
        } else {
          setHasError(false);
          const next = Math.max(0, Math.min(100, event.payload.progress));
          setProgress(next);
        }
      }
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [model]);

  useEffect(() => {
    if (isDownloaded.data && progress > 0) {
      setProgress(0);
      onDownloadComplete?.(model);
    }
  }, [isDownloaded.data, model, onDownloadComplete, progress]);

  const handleDownload = useCallback(() => {
    if (isDownloaded.data || isDownloading.data || isStarting) {
      return;
    }
    setHasError(false);
    setIsStarting(true);
    setProgress(0);
    void localSttCommands.downloadModel(model).then((result) => {
      if (result.status === "error") {
        setHasError(true);
        setIsStarting(false);
      }
    });
  }, [isDownloaded.data, isDownloading.data, isStarting, model]);

  const handleCancel = useCallback(() => {
    void localSttCommands.cancelDownload(model);
    setIsStarting(false);
    setProgress(0);
  }, [model]);

  const handleDelete = useCallback(() => {
    void localSttCommands.deleteModel(model).then((result) => {
      if (result.status === "ok") {
        void isDownloaded.refetch();
      }
    });
  }, [model, isDownloaded]);

  return {
    progress,
    hasError,
    isDownloaded: isDownloaded.data ?? false,
    isDownloadedLoading: isDownloaded.isLoading,
    showProgress,
    handleDownload,
    handleCancel,
    handleDelete,
  };
}
