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
} from "./list-common";

const OpenAIModelSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
    }),
  ),
});

export async function listOpenAIModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return pipe(
    fetchJson(`${baseUrl}/models`, { Authorization: `Bearer ${apiKey}` }),
    Effect.andThen((json) => Schema.decodeUnknown(OpenAIModelSchema)(json)),
    Effect.map(({ data }) => ({
      ...partition(
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
          return reasons.length > 0 ? reasons : null;
        },
        (model) => model.id,
      ),
      metadata: extractMetadataMap(
        data,
        (model) => model.id,
        (_model) => ({ input_modalities: ["text", "image"] }),
      ),
    })),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

export async function listGenericModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return pipe(
    fetchJson(`${baseUrl}/models`, { Authorization: `Bearer ${apiKey}` }),
    Effect.andThen((json) => Schema.decodeUnknown(OpenAIModelSchema)(json)),
    Effect.map(({ data }) => ({
      ...partition(
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
          return reasons.length > 0 ? reasons : null;
        },
        (model) => model.id,
      ),
      metadata: extractMetadataMap(
        data,
        (model) => model.id,
        () => ({ input_modalities: ["text"] }),
      ),
    })),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}
