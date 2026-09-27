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

const VeniceModelsSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      type: Schema.String,
      model_spec: Schema.Struct({
        offline: Schema.optional(Schema.Boolean),
        capabilities: Schema.Struct({
          supportsVision: Schema.optional(Schema.Boolean),
        }),
      }),
    }),
  ),
});

export async function listVeniceModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) return DEFAULT_RESULT;

  return pipe(
    fetchJson(`${baseUrl.replace(/\/+$/, "")}/models?type=text`, {
      Authorization: `Bearer ${apiKey}`,
    }),
    Effect.andThen((json) => Schema.decodeUnknown(VeniceModelsSchema)(json)),
    Effect.map(({ data }) => {
      const available = data.filter((model) => !model.model_spec.offline);
      const result = partition(
        available,
        (model) => (model.type !== "text" ? ["not_llm"] : null),
        (model) => model.id,
      );
      return {
        ...result,
        models: sortModelsByRecency(result.models),
        metadata: extractMetadataMap(
          available.filter((model) => model.type === "text"),
          (model) => model.id,
          (model) => ({
            input_modalities: model.model_spec.capabilities.supportsVision
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
