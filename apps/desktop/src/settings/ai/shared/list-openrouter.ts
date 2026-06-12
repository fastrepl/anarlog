import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type InputModality,
  isNonChatModel,
  isOldModel,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  shouldIgnoreCommonKeywords,
  sortModelsByRecency,
} from "./list-common";

const OpenRouterModelSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      supported_parameters: Schema.optional(Schema.Array(Schema.String)),
      architecture: Schema.optional(
        Schema.Struct({
          input_modalities: Schema.optional(Schema.Array(Schema.String)),
          output_modalities: Schema.optional(Schema.Array(Schema.String)),
        }),
      ),
    }),
  ),
});

type OpenRouterModel = Schema.Schema.Type<
  typeof OpenRouterModelSchema
>["data"][number];

export async function listOpenRouterModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return pipe(
    fetchJson(`${baseUrl}/models`, { Authorization: `Bearer ${apiKey}` }),
    Effect.andThen((json) => Schema.decodeUnknown(OpenRouterModelSchema)(json)),
    Effect.map(({ data }) => processOpenRouterModels(data)),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

export const processOpenRouterModels = (
  data: ReadonlyArray<OpenRouterModel>,
): ListModelsResult => {
  const result = partition(data, getIgnoreReasons, (model) => model.id);

  return {
    models: sortModelsByRecency(result.models),
    ignored: result.ignored,
    metadata: extractMetadataMap(
      data,
      (model) => model.id,
      (model) => ({ input_modalities: getInputModalities(model) }),
    ),
  };
};

const hasCommonIgnoreKeywords = (model: OpenRouterModel): boolean =>
  shouldIgnoreCommonKeywords(model.id);

const supportsTextInput = (model: OpenRouterModel): boolean =>
  !Array.isArray(model.architecture?.input_modalities) ||
  model.architecture.input_modalities.includes("text");

const supportsToolUse = (model: OpenRouterModel): boolean =>
  !model.supported_parameters ||
  ["tools", "tool_choice"].every((parameter) =>
    model.supported_parameters?.includes(parameter),
  );

const getIgnoreReasons = (
  model: OpenRouterModel,
): ModelIgnoreReason[] | null => {
  const reasons: ModelIgnoreReason[] = [];
  if (hasCommonIgnoreKeywords(model)) {
    reasons.push("common_keyword");
  }
  if (isNonChatModel(model.id)) {
    reasons.push("not_chat_model");
  }
  if (!supportsTextInput(model)) {
    reasons.push("no_text_input");
  }
  if (!supportsToolUse(model)) {
    reasons.push("no_tool");
  }
  if (isOldModel(model.id)) {
    reasons.push("old_model");
  }
  return reasons.length > 0 ? reasons : null;
};

const getInputModalities = (model: OpenRouterModel): InputModality[] => {
  const modalities = model.architecture?.input_modalities ?? [];

  return [
    ...((modalities.includes("text")
      ? ["text"]
      : []) satisfies InputModality[]),
    ...((modalities.includes("image")
      ? ["image"]
      : []) satisfies InputModality[]),
  ];
};
