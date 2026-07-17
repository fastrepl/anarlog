import { Effect, pipe, Schema } from "effect";

import {
  DEFAULT_RESULT,
  type IgnoredModel,
  type ListModelsResult,
  type ModelIgnoreReason,
  type ModelMetadata,
  REQUEST_TIMEOUT,
  fetchJson,
} from "./list-common";
import { listGenericModels } from "./list-openai";

const LMStudioModelSchema = Schema.Struct({
  models: Schema.Array(
    Schema.Struct({
      type: Schema.String,
      key: Schema.String,
      loaded_instances: Schema.Array(Schema.Unknown),
      max_context_length: Schema.Number,
      capabilities: Schema.optional(
        Schema.Struct({
          trained_for_tool_use: Schema.optional(Schema.Boolean),
          vision: Schema.optional(Schema.Boolean),
        }),
      ),
    }),
  ),
});

type LMStudioModel = Schema.Schema.Type<
  typeof LMStudioModelSchema
>["models"][number];

export async function listLMStudioModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return pipe(
    fetchJson(getLMStudioNativeModelsUrl(baseUrl), getLMStudioHeaders(apiKey)),
    Effect.andThen((json) => Schema.decodeUnknown(LMStudioModelSchema)(json)),
    Effect.map(({ models }) => processLMStudioModels(models)),
    Effect.catchAll(() =>
      Effect.tryPromise(() => listGenericModels(baseUrl, apiKey)),
    ),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

export const getLMStudioNativeModelsUrl = (baseUrl: string) => {
  const url = new URL(baseUrl);
  const path = url.pathname.replace(/\/+$/, "");

  if (path.endsWith("/api/v1")) {
    url.pathname = `${path}/models`;
    return url.toString();
  }

  if (path.endsWith("/v1")) {
    url.pathname = `${path.slice(0, -3)}/api/v1/models`;
    return url.toString();
  }

  url.pathname = `${path}/api/v1/models`;
  return url.toString();
};

const getLMStudioHeaders = (apiKey: string) => {
  const trimmedApiKey = apiKey.trim();
  const headers: Record<string, string> = {};
  if (trimmedApiKey.length > 0) {
    headers.Authorization = `Bearer ${trimmedApiKey}`;
  }
  return headers;
};

export const processLMStudioModels = (
  downloadedModels: ReadonlyArray<LMStudioModel>,
): ListModelsResult => {
  const models: string[] = [];
  const ignored: IgnoredModel[] = [];
  const metadata: Record<string, ModelMetadata> = {};

  for (const model of downloadedModels) {
    const reasons: ModelIgnoreReason[] = [];

    if (model.type !== "llm") {
      reasons.push("not_llm");
    } else {
      if (model.capabilities?.trained_for_tool_use === false) {
        reasons.push("no_tool");
      }
      if (model.max_context_length <= 15 * 1000) {
        reasons.push("context_too_small");
      }
    }

    if (reasons.length === 0) {
      models.push(model.key);
      metadata[model.key] = {
        input_modalities: model.capabilities?.vision
          ? ["text", "image"]
          : ["text"],
      };
    } else {
      ignored.push({ id: model.key, reasons });
    }
  }

  const loadedModelsSet = new Set(
    downloadedModels
      .filter((model) => model.loaded_instances.length > 0)
      .map((model) => model.key),
  );

  models.sort((a, b) => {
    const aLoaded = loadedModelsSet.has(a);
    const bLoaded = loadedModelsSet.has(b);
    if (aLoaded === bLoaded) {
      return 0;
    }
    return aLoaded ? -1 : 1;
  });

  return { models, ignored, metadata };
};
