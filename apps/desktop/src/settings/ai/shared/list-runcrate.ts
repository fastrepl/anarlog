import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type InputModality,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  sortModelsByRecency,
} from "./list-common";

// Runcrate's /models catalog also lists image, video, speech and embedding
// models, so chat models are picked from the modalities and features it
// reports for each entry.
const RuncrateModelsSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      input_modalities: Schema.optional(Schema.Array(Schema.String)),
      output_modalities: Schema.optional(Schema.Array(Schema.String)),
      supported_features: Schema.optional(Schema.Array(Schema.String)),
    }),
  ),
});

type RuncrateModel = Schema.Schema.Type<
  typeof RuncrateModelsSchema
>["data"][number];

export async function listRuncrateModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) return DEFAULT_RESULT;

  return pipe(
    fetchJson(`${baseUrl.replace(/\/+$/, "")}/models`, {
      Authorization: `Bearer ${apiKey}`,
    }),
    Effect.andThen((json) => Schema.decodeUnknown(RuncrateModelsSchema)(json)),
    Effect.map(({ data }) => processRuncrateModels(data)),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

export const processRuncrateModels = (
  data: ReadonlyArray<RuncrateModel>,
): ListModelsResult => {
  const result = partition(data, getIgnoreReasons, (model) => model.id);

  return {
    models: sortModelsByRecency(result.models),
    ignored: result.ignored,
    metadata: extractMetadataMap(
      data.filter((model) => result.models.includes(model.id)),
      (model) => model.id,
      (model) => ({
        input_modalities: (model.input_modalities?.includes("image")
          ? ["text", "image"]
          : ["text"]) satisfies InputModality[],
      }),
    ),
  };
};

const getIgnoreReasons = (model: RuncrateModel): ModelIgnoreReason[] | null => {
  const reasons: ModelIgnoreReason[] = [];
  if (!model.output_modalities?.includes("text")) {
    reasons.push("not_llm");
  }
  if (!model.input_modalities?.includes("text")) {
    reasons.push("no_text_input");
  }
  if (!model.supported_features?.includes("tools")) {
    reasons.push("no_tool");
  }
  return reasons.length > 0 ? reasons : null;
};
