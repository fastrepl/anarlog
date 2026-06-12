import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  isOldModel,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  shouldIgnoreCommonKeywords,
  sortModelsByRecency,
} from "./list-common";

const AzureAIDeploymentSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      model: Schema.optional(Schema.String),
    }),
  ),
});

export async function listAzureAIModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  const url = `${baseUrl.replace(/\/+$/, "")}/models`;

  return pipe(
    fetchJson(url, { "api-key": apiKey }),
    Effect.andThen((json) =>
      Schema.decodeUnknown(AzureAIDeploymentSchema)(json),
    ),
    Effect.map(({ data }) => {
      const result = partition(
        data,
        (model) => {
          const reasons: ModelIgnoreReason[] = [];
          if (shouldIgnoreCommonKeywords(model.id)) {
            reasons.push("common_keyword");
          }
          if (isOldModel(model.id)) {
            reasons.push("old_model");
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
