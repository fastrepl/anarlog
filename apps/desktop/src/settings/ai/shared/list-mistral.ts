import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type InputModality,
  isDateSnapshot,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  shouldIgnoreCommonKeywords,
} from "./list-common";

const MistralCapabilitiesSchema = Schema.Struct({
  completion_chat: Schema.Boolean,
  vision: Schema.Boolean,
});

const MistralModelSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      capabilities: MistralCapabilitiesSchema,
    }),
  ),
});

type MistralModel = Schema.Schema.Type<
  typeof MistralModelSchema
>["data"][number];

export async function listMistralModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  const supportsChatCompletion = (model: MistralModel): boolean =>
    model.capabilities.completion_chat;

  const getIgnoreReasons = (
    model: MistralModel,
  ): ModelIgnoreReason[] | null => {
    const reasons: ModelIgnoreReason[] = [];
    if (shouldIgnoreCommonKeywords(model.id)) {
      reasons.push("common_keyword");
    }
    if (!supportsChatCompletion(model)) {
      reasons.push("no_completion");
    }
    if (isDateSnapshot(model.id)) {
      reasons.push("date_snapshot");
    }
    return reasons.length > 0 ? reasons : null;
  };

  const getInputModalities = (model: MistralModel): InputModality[] => {
    return model.capabilities.vision ? ["text", "image"] : ["text"];
  };

  return pipe(
    fetchJson(`${baseUrl}/models`, { Authorization: `Bearer ${apiKey}` }),
    Effect.andThen((json) => Schema.decodeUnknown(MistralModelSchema)(json)),
    Effect.map(({ data }) => ({
      ...partition(data, getIgnoreReasons, (model) => model.id),
      metadata: extractMetadataMap(
        data,
        (model) => model.id,
        (model) => ({
          input_modalities: getInputModalities(model),
        }),
      ),
    })),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}
