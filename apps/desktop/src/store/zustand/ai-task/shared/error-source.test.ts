import { APICallError } from "ai";
import { describe, expect, it } from "vitest";

import { attributeAIError } from "./error-source";

const HOSTED_URL = "https://api.anarlog.so/llm/chat/completions";
const OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions";

function apiError(url: string, statusCode: number, responseBody: string) {
  let data: unknown;
  try {
    data = JSON.parse(responseBody);
  } catch {
    data = undefined;
  }
  return new APICallError({
    message: "request failed",
    url,
    requestBodyValues: {},
    statusCode,
    responseBody,
    data,
  });
}

describe("attributeAIError", () => {
  it.each([
    [
      "OpenRouter relaying a provider failure",
      apiError(
        HOSTED_URL,
        502,
        JSON.stringify({
          error: {
            code: 502,
            message: "Provider returned error",
            metadata: { provider_name: "Anthropic", raw: "overloaded" },
          },
        }),
      ),
      "openrouter",
      { kind: "provider", provider: "Anthropic" },
    ],
    [
      "OpenRouter's own error",
      apiError(
        OPENROUTER_URL,
        402,
        JSON.stringify({
          error: { code: 402, message: "Insufficient credits" },
        }),
      ),
      "openrouter",
      { kind: "openrouter" },
    ],
    [
      "the Anarlog proxy failing to reach OpenRouter",
      apiError(HOSTED_URL, 502, "Upstream request failed"),
      "openrouter",
      { kind: "openrouter" },
    ],
    [
      "an Anarlog API rejection",
      apiError(HOSTED_URL, 401, "invalid_token"),
      "openrouter",
      { kind: "anarlog" },
    ],
    [
      "a mid-stream OpenRouter error naming the provider",
      {
        code: "server_error",
        message: "Provider disconnected",
        metadata: { provider_name: "Google" },
      },
      "openrouter",
      { kind: "provider", provider: "Google" },
    ],
    [
      "a direct provider API error",
      apiError("https://api.openai.com/v1/responses", 500, "{}"),
      "openai",
      { kind: "provider", provider: "OpenAI" },
    ],
    [
      "an unknown custom endpoint",
      apiError("http://localhost:1234/v1/chat/completions", 500, "{}"),
      "openai",
      undefined,
    ],
    [
      "a direct provider error with no request URL",
      { message: "Overloaded" },
      "anthropic",
      undefined,
    ],
    [
      "a non-API error",
      new Error("AI generation exceeded the safe output limit."),
      "openrouter",
      undefined,
    ],
  ])("attributes %s", (_name, error, providerId, expected) => {
    expect(attributeAIError(error, providerId)).toEqual(expected);
  });
});
