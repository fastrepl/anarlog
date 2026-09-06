import type { ProviderConfig } from "@/settings/providers-model";

export function summaryRequest(
  provider: ProviderConfig & { apiKey: string },
  system: string,
  source: string,
  apiUrl: string,
) {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
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
  if (provider.provider === "anthropic") {
    headers["x-api-key"] = provider.apiKey;
    headers["anthropic-version"] = "2023-06-01";
    return {
      url: `${provider.baseUrl}/messages`,
      headers,
      body: {
        model: provider.model,
        max_tokens: 4096,
        system,
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
