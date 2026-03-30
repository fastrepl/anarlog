import { useCallback } from "react";

import type { ModelInfo } from "@hypr/plugin-template";

import { useMCP } from "./useMCP";

import { collectSupportContextBlock } from "~/chat/context/support-block";
import { useConfigValues } from "~/shared/config";

export function useSupportMCP(enabled: boolean, accessToken?: string | null) {
  const {
    current_llm_provider,
    current_llm_model,
    current_stt_provider,
    current_stt_model,
  } = useConfigValues([
    "current_llm_provider",
    "current_llm_model",
    "current_stt_provider",
    "current_stt_model",
  ] as const);

  const modelInfo: ModelInfo | null =
    current_llm_provider || current_stt_provider
      ? {
          llmProvider: current_llm_provider ?? null,
          llmModel: current_llm_model ?? null,
          sttProvider: current_stt_provider ?? null,
          sttModel: current_stt_model ?? null,
        }
      : null;

  const collectContext = useCallback(
    () => collectSupportContextBlock(modelInfo),
    [
      current_llm_provider,
      current_llm_model,
      current_stt_provider,
      current_stt_model,
    ],
  );

  return useMCP({
    enabled,
    endpoint: "/support/mcp",
    clientName: "hyprnote-support-client",
    accessToken,
    promptName: "support_chat",
    collectContext,
  });
}
