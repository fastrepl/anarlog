import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type ListModelsResult,
  type ModelIgnoreReason,
  partition,
  REQUEST_TIMEOUT,
  shouldIgnoreCommonKeywords,
} from "./list-common";

const UnslothModelSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
    }),
  ),
});

export async function listUnslothModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return pipe(
    fetchJson(getUnslothModelsUrl(baseUrl), getUnslothHeaders(apiKey)),
    Effect.andThen((json) => Schema.decodeUnknown(UnslothModelSchema)(json)),
    Effect.map(({ data }) => processUnslothModels(data)),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

export const getUnslothModelsUrl = (baseUrl: string) =>
  `${baseUrl.replace(/\/+$/, "")}/models`;

export const getUnslothHeaders = (apiKey: string) => {
  const trimmedApiKey = apiKey.trim();
  const headers: Record<string, string> = {};
  if (trimmedApiKey.length > 0) {
    headers.Authorization = `Bearer ${trimmedApiKey}`;
  }
  return headers;
};

// Unsloth serves whatever GGUF the user loaded locally, so the hosted-catalog
// heuristics (release recency, deprecated families) would hide valid picks.
// Only entries that are clearly not chat models get dropped.
export function processUnslothModels(
  data: readonly { id: string }[],
): ListModelsResult {
  const { models, ignored } = partition(
    data,
    (model) =>
      shouldIgnoreCommonKeywords(model.id)
        ? (["common_keyword"] as ModelIgnoreReason[])
        : null,
    (model) => model.id,
  );

  return {
    models: [...models].sort((a, b) => a.localeCompare(b)),
    ignored,
    metadata: extractMetadataMap(
      data,
      (model) => model.id,
      () => ({ input_modalities: ["text"] }),
    ),
  };
}
