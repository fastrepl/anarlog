import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  isDateSnapshot,
  isNonChatModel,
  isOldModel,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  shouldIgnoreCommonKeywords,
  sortModelsByRecency,
} from "./list-common";

const AzureOpenAIModelSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      capabilities: Schema.optional(
        Schema.Struct({
          chat_completion: Schema.optional(Schema.Boolean),
          completion: Schema.optional(Schema.Boolean),
          embeddings: Schema.optional(Schema.Boolean),
          inference: Schema.optional(Schema.Boolean),
        }),
      ),
    }),
  ),
});

export async function listAzureOpenAIModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  const url = `${baseUrl.replace(/\/+$/, "")}/openai/models?api-version=2024-10-21`;

  return pipe(
    fetchJson(url, { "api-key": apiKey }),
    Effect.andThen((json) =>
      Schema.decodeUnknown(AzureOpenAIModelSchema)(json),
    ),
    Effect.map(({ data }) => {
      const result = partition(
        data,
        (model) => {
          const reasons: ModelIgnoreReason[] = [];
          if (shouldIgnoreCommonKeywords(model.id)) {
            reasons.push("common_keyword");
          }
          if (isNonChatModel(model.id)) {
            reasons.push("not_chat_model");
          }
          if (isOldModel(model.id)) {
            reasons.push("old_model");
          }
          if (isDateSnapshot(model.id)) {
            reasons.push("date_snapshot");
          }
          if (
            model.capabilities &&
            model.capabilities.chat_completion === false &&
            model.capabilities.completion === false
          ) {
            reasons.push("not_chat_model");
          }
          return reasons.length > 0 ? reasons : null;
        },
        (model) => model.id,
      );

      return {
        models: sortModelsByRecency(result.models),
        ignored: result.ignored,
        metadata: extractMetadataMap(
          data,
          (model) => model.id,
          (_model) => ({ input_modalities: ["text", "image"] }),
        ),
      };
    }),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}
