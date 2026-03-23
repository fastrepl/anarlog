import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type InputModality,
  isDateSnapshot,
  isNonChatModel,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  shouldIgnoreCommonKeywords,
} from "./list-common";

const GoogleModelSchema = Schema.Struct({
  models: Schema.Array(
    Schema.Struct({
      name: Schema.String,
      supportedGenerationMethods: Schema.optional(Schema.Array(Schema.String)),
    }),
  ),
});

type GoogleModel = Schema.Schema.Type<
  typeof GoogleModelSchema
>["models"][number];

export async function listGoogleModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  const supportsGeneration = (model: GoogleModel): boolean =>
    !model.supportedGenerationMethods ||
    model.supportedGenerationMethods.includes("generateContent");

  const extractModelId = (model: GoogleModel): string => {
    return model.name.replace(/^models\//, "");
  };

  const getIgnoreReasons = (model: GoogleModel): ModelIgnoreReason[] | null => {
    const reasons: ModelIgnoreReason[] = [];
    if (shouldIgnoreCommonKeywords(model.name)) {
      reasons.push("common_keyword");
    }
    if (isNonChatModel(extractModelId(model))) {
      reasons.push("not_chat_model");
    }
    if (!supportsGeneration(model)) {
      reasons.push("no_completion");
    }
    if (isDateSnapshot(extractModelId(model))) {
      reasons.push("date_snapshot");
    }
    return reasons.length > 0 ? reasons : null;
  };

  return pipe(
    fetchJson(`${baseUrl}/models`, { "x-goog-api-key": apiKey }),
    Effect.andThen((json) => Schema.decodeUnknown(GoogleModelSchema)(json)),
    Effect.map(({ models }) => ({
      ...partition(models, getIgnoreReasons, extractModelId),
      metadata: extractMetadataMap(models, extractModelId, (model) => ({
        input_modalities: getInputModalities(extractModelId(model)),
      })),
    })),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

const getInputModalities = (modelId: string): InputModality[] => {
  const normalizedId = modelId.toLowerCase();
  const supportsMultimodal = /gemini/.test(normalizedId);
  return supportsMultimodal ? ["text", "image"] : ["text"];
};
