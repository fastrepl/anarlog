import { Effect, pipe, Schema } from "effect";

import { resolveSubscriptionAccess } from "./access";
import { COPILOT_REQUEST_HEADERS, type SubscriptionProviderId } from "./oauth";

import { listAnthropicModels } from "~/settings/ai/shared/list-anthropic";
import {
  DEFAULT_RESULT,
  extractMetadataMap,
  fetchJson,
  type ListModelsResult,
  REQUEST_TIMEOUT,
} from "~/settings/ai/shared/list-common";
import {
  listGenericModels,
  listOpenAIModels,
} from "~/settings/ai/shared/list-openai";

const FALLBACK_MODELS: Record<SubscriptionProviderId, string[]> = {
  claude: ["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
  chatgpt: ["gpt-5.4", "gpt-5.4-mini", "gpt-4.1"],
  grok: ["grok-4", "grok-4-fast", "grok-3"],
  github_copilot: ["gpt-4.1", "claude-sonnet-4", "gemini-2.5-pro"],
  kimi_code: ["kimi-for-coding"],
};

const CopilotModelSchema = Schema.Struct({
  data: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      model_picker_enabled: Schema.optional(Schema.Boolean),
    }),
  ),
});

function withFallback(
  providerId: SubscriptionProviderId,
  result: ListModelsResult,
): ListModelsResult {
  if (result.models.length > 0) {
    return result;
  }

  const models = FALLBACK_MODELS[providerId];
  return {
    models,
    ignored: result.ignored,
    metadata: Object.fromEntries(
      models.map((id) => [
        id,
        { input_modalities: ["text", "image"] as const },
      ]),
    ),
  };
}

export async function listSubscriptionModels(
  providerId: SubscriptionProviderId,
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (providerId === "kimi_code") {
    return withFallback(
      providerId,
      await listGenericModels(baseUrl, apiKey, { filterDateSnapshots: false }),
    );
  }

  const { token } = await resolveSubscriptionAccess(providerId, apiKey);

  if (providerId === "claude") {
    return withFallback(
      providerId,
      await listAnthropicModels(baseUrl, token, { authorization: "bearer" }),
    );
  }

  if (providerId === "github_copilot") {
    return withFallback(providerId, await listCopilotModels(baseUrl, token));
  }

  return withFallback(providerId, await listOpenAIModels(baseUrl, token));
}

async function listCopilotModels(
  baseUrl: string,
  accessToken: string,
): Promise<ListModelsResult> {
  if (!baseUrl || !accessToken) {
    return DEFAULT_RESULT;
  }

  return pipe(
    fetchJson(`${baseUrl.replace(/\/$/, "")}/models`, {
      Authorization: `Bearer ${accessToken}`,
      ...COPILOT_REQUEST_HEADERS,
    }),
    Effect.andThen((json) => Schema.decodeUnknown(CopilotModelSchema)(json)),
    Effect.map(({ data }) => {
      const models = data
        .filter((model) => model.model_picker_enabled !== false)
        .map((model) => model.id);
      return {
        models,
        ignored: [],
        metadata: extractMetadataMap(
          data,
          (model) => model.id,
          () => ({ input_modalities: ["text", "image"] as const }),
        ),
      };
    }),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}
