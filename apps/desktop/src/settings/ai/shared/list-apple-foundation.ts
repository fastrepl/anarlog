import { commands as localLlmCommands } from "@anlg/plugin-local-llm";

import type { ListModelsResult } from "./list-common";

const MODEL_ID = "System Language Model";

export async function checkAppleFoundationModelAvailability(): Promise<boolean> {
  const result = await localLlmCommands.foundationModelAvailability();
  return result.status === "ok" && result.data.status === "available";
}

export async function listAppleFoundationModels(): Promise<ListModelsResult> {
  const result = await localLlmCommands.foundationModelAvailability();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  if (result.data.status !== "available") {
    throw new Error(availabilityMessage(result.data.reason));
  }

  return {
    models: [MODEL_ID],
    ignored: [],
    metadata: { [MODEL_ID]: { input_modalities: ["text"] } },
  };
}

function availabilityMessage(reason: string | null): string {
  switch (reason) {
    case "unsupported_os":
      return "Apple Foundation Models requires macOS 26 or later.";
    case "device_not_eligible":
      return "This Mac does not support Apple Intelligence.";
    case "apple_intelligence_not_enabled":
      return "Turn on Apple Intelligence in System Settings.";
    case "model_not_ready":
      return "The Apple Intelligence model is still downloading.";
    default:
      return "Apple Foundation Models is not available.";
  }
}
