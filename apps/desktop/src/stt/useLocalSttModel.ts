import { queryOptions } from "@tanstack/react-query";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@anlg/plugin-local-stt";

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
