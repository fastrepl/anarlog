import { type LLM, LMStudioClient, type ModelInfo } from "@lmstudio/sdk";
import { Effect, pipe } from "effect";

import {
  DEFAULT_RESULT,
  type IgnoredModel,
  type ListModelsResult,
  type ModelIgnoreReason,
  type ModelMetadata,
  REQUEST_TIMEOUT,
} from "./list-common";

export async function listLMStudioModels(
  baseUrl: string,
  _apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return pipe(
    createLMStudioClient(baseUrl),
    Effect.flatMap((client) =>
      pipe(
        fetchLMStudioInventory(client),
        Effect.map(({ downloadedModels, loadedLLMs }) =>
          processLMStudioModels(downloadedModels, loadedLLMs),
        ),
      ),
    ),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

const createLMStudioClient = (baseUrl: string) =>
  Effect.sync(() => {
    const url = new URL(baseUrl);
    const port = url.port || "1234";
    const formattedUrl = `ws:127.0.0.1:${port}`;
    return new LMStudioClient({ baseUrl: formattedUrl });
  });

const fetchLMStudioInventory = (client: LMStudioClient) =>
  pipe(
    Effect.all(
      [
        Effect.tryPromise(() => client.system.listDownloadedModels()),
        Effect.tryPromise(() => client.llm.listLoaded()).pipe(
          Effect.catchAll(() => Effect.succeed([] as Array<LLM>)),
        ),
      ],
      { concurrency: "unbounded" },
    ),
    Effect.flatMap(([downloadedModels, _loadedLLMs]) =>
      pipe(
        Effect.all(
          _loadedLLMs.map((llm) =>
            Effect.tryPromise(async () => ({
              modelKey: llm.modelKey,
              context: await llm.getContextLength(),
            })),
          ),
          { concurrency: "unbounded" },
        ),
        Effect.map((loadedLLMs) => ({ downloadedModels, loadedLLMs })),
      ),
    ),
  );

const processLMStudioModels = (
  downloadedModels: Array<ModelInfo>,
  loadedLLMs: Array<{ modelKey: string; context: number }>,
): ListModelsResult => {
  const models: string[] = [];
  const ignored: IgnoredModel[] = [];
  const metadata: Record<string, ModelMetadata> = {};

  for (const model of downloadedModels) {
    const reasons: ModelIgnoreReason[] = [];

    if (model.type !== "llm") {
      reasons.push("not_llm");
    } else {
      if (!model.trainedForToolUse) {
        reasons.push("no_tool");
      }
      if (model.maxContextLength <= 15 * 1000) {
        reasons.push("context_too_small");
      }
    }

    if (reasons.length === 0) {
      models.push(model.path);
      // TODO: Seems like LMStudio do not have way to know input modality.
      metadata[model.path] = { input_modalities: ["text"] };
    } else {
      ignored.push({ id: model.path, reasons });
    }
  }

  const loadedLLMsSet = new Set(loadedLLMs.map((m) => m.modelKey));

  models.sort((a, b) => {
    const aLoaded = loadedLLMsSet.has(a);
    const bLoaded = loadedLLMsSet.has(b);
    if (aLoaded === bLoaded) {
      return 0;
    }
    return aLoaded ? -1 : 1;
  });

  return { models, ignored, metadata };
};
