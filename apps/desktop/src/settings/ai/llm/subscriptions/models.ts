import { Effect, pipe, Schema } from "effect";

import { resolveSubscriptionAccess } from "./access";
import {
  CHATGPT_API_BASE_URL,
  CHATGPT_REQUEST_HEADERS,
  COPILOT_REQUEST_HEADERS,
  parseChatgptAccountId,
  type SubscriptionProviderId,
} from "./oauth";

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
  chatgpt: [],
  grok: ["grok-4.6", "grok-4.5", "grok-4.3"],
  github_copilot: ["gpt-5.5", "claude-sonnet-5", "gemini-3.6-flash"],
  kimi_code: ["kimi-for-coding"],
};

const ChatgptModelSchema = Schema.Struct({
  models: Schema.Array(
    Schema.Struct({
      slug: Schema.String,
      visibility: Schema.optional(Schema.String),
    }),
  ),
});

const CHATGPT_CODEX_CLIENT_VERSION = "0.145.0";

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

  const { token, credential } = await resolveSubscriptionAccess(
    providerId,
    apiKey,
  );

  if (providerId === "github_copilot") {
    return withFallback(providerId, await listCopilotModels(baseUrl, token));
  }

  if (providerId === "chatgpt") {
    return withFallback(
      providerId,
      await listChatgptModels(
        baseUrl,
        token,
        credential?.accountId ?? parseChatgptAccountId(token),
      ),
    );
  }

  return withFallback(providerId, await listOpenAIModels(baseUrl, token));
}

async function listChatgptModels(
  baseUrl: string,
  accessToken: string,
  accountId?: string,
): Promise<ListModelsResult> {
  const endpoint = chatgptModelsUrl(baseUrl);
  if (!endpoint || !accessToken) {
    return DEFAULT_RESULT;
  }

  const headers: Record<string, string> = {
    Authorization: `Bearer ${accessToken}`,
    ...CHATGPT_REQUEST_HEADERS,
  };
  if (accountId) {
    headers["ChatGPT-Account-ID"] = accountId;
  }

  return pipe(
    fetchJson(
      `${endpoint}/models?client_version=${CHATGPT_CODEX_CLIENT_VERSION}`,
      headers,
    ),
    Effect.andThen((json) => Schema.decodeUnknown(ChatgptModelSchema)(json)),
    Effect.map(({ models: catalog }) => {
      const models = catalog
        .filter((model) => model.visibility !== "hide")
        .map((model) => model.slug);
      return {
        models,
        ignored: [],
        metadata: extractMetadataMap(
          catalog,
          (model) => model.slug,
          () => ({ input_modalities: ["text", "image"] as const }),
        ),
      };
    }),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(DEFAULT_RESULT)),
    Effect.runPromise,
  );
}

function chatgptModelsUrl(baseUrl: string): string {
  if (!baseUrl || baseUrl.includes("api.openai.com")) {
    return CHATGPT_API_BASE_URL;
  }
  return baseUrl.replace(/\/$/, "");
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
