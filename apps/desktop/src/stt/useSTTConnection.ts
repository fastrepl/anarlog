import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { commands as localSttCommands } from "@hypr/plugin-local-stt";
import type { AIProviderStorage } from "@hypr/store";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { type ProviderId } from "~/settings/ai/stt/shared";
import { useAiProvider } from "~/settings/providers";
import { useConfigValues } from "~/shared/config";
import {
  isHyprnoteCloudSttModel,
  isHyprnoteLocalSttModel,
} from "~/stt/capabilities";

export const useSTTConnection = () => {
  const auth = useAuth();
  const billing = useBillingAccess();
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const) as {
    current_stt_provider: ProviderId | undefined;
    current_stt_model: string | undefined;
  };

  const providerConfig = useAiProvider("stt", current_stt_provider) as
    | AIProviderStorage
    | undefined;

  const localModel = isHyprnoteLocalSttModel(
    current_stt_provider,
    current_stt_model,
  )
    ? current_stt_model
    : null;
  const isLocalModel = !!localModel;

  const isCloudModel = isHyprnoteCloudSttModel(
    current_stt_provider,
    current_stt_model,
  );

  const local = useQuery({
    enabled: current_stt_provider === "hyprnote",
    queryKey: ["stt-connection", localModel],
    refetchInterval: 1000,
    queryFn: async () => {
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
        baseUrl: baseUrl ?? new URL("/stt", env.VITE_API_URL).toString(),
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
    isLocalModel,
    isCloudModel,
  };
};
