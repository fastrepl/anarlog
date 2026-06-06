import { useQuery } from "@tanstack/react-query";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { Spinner } from "@hypr/ui/components/ui/spinner";

import { useConfigValues } from "~/shared/config";
import {
  isHyprnoteCloudSttModel,
  isHyprnoteLocalSttModel,
} from "~/stt/capabilities";
import { useSTTConnection } from "~/stt/useSTTConnection";

export type HealthStatus = {
  status: "pending" | "error" | "success" | null;
  message?: string;
};

export function HealthStatusIndicator() {
  const health = useConnectionHealth();

  if (health.status === "pending") {
    return <Spinner size={14} className="text-muted-foreground shrink-0" />;
  }

  return null;
}

function useDeepgramHealth(enabled: boolean, apiKey?: string) {
  return useQuery({
    enabled,
    queryKey: ["stt-health-check", "deepgram", apiKey],
    staleTime: 0,
    retry: 3,
    retryDelay: 200,
    queryFn: async () => {
      const response = await tauriFetch(
        "https://api.deepgram.com/v1/projects",
        {
          headers: {
            Authorization: `Token ${apiKey}`,
          },
        },
      );
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      return response.status;
    },
  });
}

export function useConnectionHealth(): HealthStatus {
  const { conn, local } = useSTTConnection();
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);

  const isLocalModel = isHyprnoteLocalSttModel(
    current_stt_provider,
    current_stt_model,
  );
  const isCloud =
    isHyprnoteCloudSttModel(current_stt_provider, current_stt_model) ||
    current_stt_provider !== "hyprnote";
  const isDeepgram = current_stt_provider === "deepgram";

  const deepgramHealth = useDeepgramHealth(isDeepgram && !!conn, conn?.apiKey);

  if (
    current_stt_provider === "hyprnote" &&
    current_stt_model &&
    !isCloud &&
    !isLocalModel
  ) {
    return {
      status: "error",
      message: "Selected model is no longer available.",
    };
  }

  if (isLocalModel) {
    const serverStatus = local.data?.status ?? "unavailable";
    if (serverStatus === "not_downloaded") {
      return {
        status: "error",
        message: "Selected model is not downloaded.",
      };
    }
    if (serverStatus === "loading") {
      return {
        status: "pending",
        message: "Local STT server is starting up…",
      };
    }
    if (serverStatus === "ready" && conn) {
      return { status: "success" };
    }
    return {
      status: "error",
      message: "Could not connect to the local speech-to-text model.",
    };
  }

  if (!conn) {
    return { status: "error", message: "Provider not configured." };
  }

  if (isDeepgram) {
    if (deepgramHealth.isPending) {
      return { status: "pending", message: "Verifying API key..." };
    }
    if (deepgramHealth.isError) {
      return {
        status: "error",
        message: `API key verification failed: ${deepgramHealth.error.message}`,
      };
    }
    if (deepgramHealth.isSuccess) {
      return { status: "success" };
    }
  }

  return { status: "success" };
}
