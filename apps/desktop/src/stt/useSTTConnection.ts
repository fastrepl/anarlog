import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { commands as localSttCommands } from "@anlg/plugin-local-stt";
import type { AIProviderStorage } from "@anlg/store";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { env } from "~/env";
import { type ProviderId } from "~/settings/ai/stt/shared";
import { useAiProvider } from "~/settings/providers";
import { useConfigValues } from "~/shared/config";
import {
  isAnarlogCloudSttModel,
  isLocalFileSttModel,
  isOnDeviceSttModel,
  isRealtimeLocalModel,
} from "~/stt/capabilities";
import { localSttQueries } from "~/stt/useLocalSttModel";

export const useSTTConnection = () => {
  const auth = useAuth();
  const billing = useBillingAccess();
  const { current_stt_provider, current_stt_model, local_stt_model_path } =
    useConfigValues([
      "current_stt_provider",
      "current_stt_model",
      "local_stt_model_path",
    ] as const) as {
      current_stt_provider: ProviderId | undefined;
      current_stt_model: string | undefined;
      local_stt_model_path: string | undefined;
    };

  const providerConfig = useAiProvider("stt", current_stt_provider) as
    | AIProviderStorage
    | undefined;

  const localModel = isOnDeviceSttModel(current_stt_provider, current_stt_model)
    ? current_stt_model
    : null;
  const isLocalFile = isLocalFileSttModel(
    current_stt_provider,
    current_stt_model,
  );
  const isLocalModel = !!localModel || isLocalFile;

  const isCloudModel = isAnarlogCloudSttModel(
    current_stt_provider,
    current_stt_model,
  );
  const localBatchModel = useQuery({
    ...localSttQueries.isDownloaded("soniqo-parakeet-batch"),
    enabled: isRealtimeLocalModel(current_stt_model),
  });

  const local = useQuery({
    enabled: isLocalModel,
    queryKey: [
      "stt-connection",
      current_stt_provider,
      localModel,
      local_stt_model_path,
    ],
    refetchInterval: (query) =>
      query.state.data?.status === "loading" ? 1000 : false,
    queryFn: async () => {
      if (isLocalFile) {
        const path = local_stt_model_path?.trim();
        if (!path) {
          return {
            status: "not_selected" as const,
            connection: null,
          };
        }

        const started = await localSttCommands.startServerForPath(path);
        if (started.status === "error") {
          return {
            status: "error" as const,
            error: started.error,
            connection: null,
          };
        }

        return {
          status: "ready" as const,
          connection: {
            provider: "local_file" as const,
            model: "local-file" as const,
            baseUrl: started.data,
            apiKey: "",
          },
        };
      }

      if (!localModel) {
        return null;
      }

      const downloaded = await localSttCommands.isModelDownloaded(localModel);
      if (downloaded.status !== "ok" || !downloaded.data) {
        return { status: "not_downloaded" as const, connection: null };
      }

      const serverResult = await localSttCommands.getServerForModel(localModel);

      if (serverResult.status !== "ok") {
        return null;
      }

      const server = serverResult.data;

      if (server?.status === "ready" && server.url) {
        return {
          status: "ready" as const,
          connection: {
            provider: current_stt_provider!,
            model: localModel,
            baseUrl: server.url,
            apiKey: "",
          },
        };
      }

      return {
        status: server?.status ?? "loading",
        connection: null,
      };
    },
  });

  const baseUrl = providerConfig?.base_url?.trim();
  const apiKey = providerConfig?.api_key?.trim();

  const connection = useMemo(() => {
    if (!current_stt_provider || !current_stt_model) {
      return null;
    }

    if (isLocalModel) {
      return local.data?.connection ?? null;
    }

    if (isCloudModel) {
      if (!auth?.session || !billing.isPaid) {
        return null;
      }

      return {
        provider: current_stt_provider,
        model: current_stt_model,
        baseUrl: baseUrl || new URL("/stt", env.VITE_API_URL).toString(),
        apiKey: auth.session.access_token,
      };
    }

    if (!baseUrl || !apiKey) {
      return null;
    }

    return {
      provider: current_stt_provider,
      model: current_stt_model,
      baseUrl,
      apiKey,
    };
  }, [
    current_stt_provider,
    current_stt_model,
    localModel,
    isLocalModel,
    isCloudModel,
    local.data,
    baseUrl,
    apiKey,
    auth,
    billing.isPaid,
  ]);

  return {
    conn: connection,
    local,
    localBatchDiarizationAvailable: localBatchModel.data === true,
    isLocalModel,
    isCloudModel,
  };
};
