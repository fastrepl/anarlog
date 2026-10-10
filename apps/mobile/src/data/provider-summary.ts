import type { ProviderConfig } from "@/settings/providers-model";

import { CHATGPT_BASE_URL, chatgptHeaders } from "../settings/chatgpt-oauth";
import {
  CLAUDE_CODE_IDENTITY,
  CLAUDE_HEADERS,
  COPILOT_HEADERS,
} from "../settings/subscription-oauth";

export function summaryRequest(
  provider: ProviderConfig & { apiKey: string; accountId?: string },
  system: string,
  source: string,
  apiUrl: string,
) {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (provider.provider === "chatgpt") {
    return {
      url: `${CHATGPT_BASE_URL}/responses`,
      headers: {
        ...headers,
        ...chatgptHeaders(provider.apiKey, provider.accountId),
        Accept: "text/event-stream",
      },
      body: {
        model: provider.model,
        instructions: system,
        input: [
          { role: "user", content: [{ type: "input_text", text: source }] },
        ],
        store: false,
        stream: true,
      },
    };
  }
  if (provider.provider === "google_generative_ai") {
    headers["x-goog-api-key"] = provider.apiKey;
    const model = encodeURIComponent(provider.model.replace(/^models\//, ""));
    return {
      url: `${provider.baseUrl}/models/${model}:generateContent`,
      headers,
      body: {
        systemInstruction: { parts: [{ text: system }] },
        contents: [{ role: "user", parts: [{ text: source }] }],
      },
    };
  }
  if (provider.provider === "anthropic" || provider.provider === "claude") {
    if (provider.provider === "claude") {
      Object.assign(headers, CLAUDE_HEADERS, {
        Authorization: `Bearer ${provider.apiKey}`,
      });
    } else headers["x-api-key"] = provider.apiKey;
    headers["anthropic-version"] = "2023-06-01";
    return {
      url: `${provider.baseUrl}/messages`,
      headers,
      body: {
        model: provider.model,
        max_tokens: 32_000,
        system:
          provider.provider === "claude"
            ? [
                { type: "text", text: CLAUDE_CODE_IDENTITY },
                { type: "text", text: system },
              ]
            : system,
        messages: [{ role: "user", content: source }],
      },
    };
  }
  let baseUrl = provider.baseUrl;
  if (provider.provider === "azure_openai") {
    headers["api-key"] = provider.apiKey;
    const url = new URL(baseUrl);
    if (url.pathname === "/" || url.pathname === "/openai")
      baseUrl = `${url.origin}/openai/v1`;
  } else if (provider.apiKey) {
    headers.Authorization = `Bearer ${provider.apiKey}`;
  }
  if (provider.provider === "azure_ai") headers["api-key"] = provider.apiKey;
  if (provider.provider === "github_copilot")
    Object.assign(headers, COPILOT_HEADERS);
  if (provider.provider === "anarlog") {
    baseUrl = `${apiUrl}/llm`;
    headers["x-char-task"] = "enhance";
  }
  return {
    url: `${baseUrl}/chat/completions`,
    headers,
    body: {
      model: provider.model || "default",
      stream: false,
      messages: [
        { role: "system", content: system },
        { role: "user", content: source },
      ],
    },
  };
}
