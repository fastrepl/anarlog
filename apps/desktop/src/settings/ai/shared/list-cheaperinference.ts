import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type ListModelsResult,
  partition,
  REQUEST_TIMEOUT,
  sortModelsByRecency,
} from "./list-common";

const CheaperInferenceModelsSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      type: Schema.String,
      capabilities: Schema.optional(
        Schema.Struct({
          vision: Schema.optional(Schema.Boolean),
        }),
      ),
    }),
  ),
});

export async function listCheaperInferenceModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) return DEFAULT_RESULT;

  return pipe(
    fetchJson(`${baseUrl.replace(/\/+$/, "")}/models?type=text`, {
      Authorization: `Bearer ${apiKey}`,
    }),
    Effect.andThen((json) =>
      Schema.decodeUnknown(CheaperInferenceModelsSchema)(json),
    ),
    Effect.map(({ data }) => {
      const result = partition(
        data,
        (model) => (model.type !== "text" ? ["not_llm"] : null),
        (model) => model.id,
      );
      return {
        ...result,
        models: sortModelsByRecency(result.models),
        metadata: extractMetadataMap(
          data.filter((model) => model.type === "text"),
          (model) => model.id,
          (model) => ({
            input_modalities: model.capabilities?.vision
              ? ["text", "image"]
              : ["text"],
          }),
        ),
      };
    }),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}
