import { beforeEach, expect, it, vi } from "vitest";

import { listRuncrateModels } from "./list-runcrate";

import { providerFetch } from "~/ai/provider-fetch";

vi.mock("~/ai/provider-fetch", () => ({ providerFetch: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

it("keeps Runcrate's tool-capable text models and their vision input", async () => {
  vi.mocked(providerFetch).mockResolvedValue(
    Response.json({
      data: [
        {
          id: "deepseek-ai/DeepSeek-V4.1-Flash",
          input_modalities: ["text", "image"],
          output_modalities: ["text"],
          supported_features: ["tools", "json_mode", "reasoning"],
        },
        {
          id: "openai/gpt-oss-120b",
          input_modalities: ["text"],
          output_modalities: ["text"],
          supported_features: ["tools", "json_mode"],
        },
        {
          id: "meta-llama/Llama-Guard-4-12B",
          input_modalities: ["text", "image"],
          output_modalities: ["text"],
          supported_features: [],
        },
        {
          id: "black-forest-labs/FLUX.2-dev",
          input_modalities: ["text"],
          output_modalities: ["image"],
        },
        {
          id: "openai/whisper-large-v3",
          input_modalities: ["audio"],
          output_modalities: ["text"],
        },
      ],
    }),
  );

  const result = await listRuncrateModels(
    "https://api.runcrate.ai/v1/",
    "test-key",
  );

  expect(providerFetch).toHaveBeenCalledWith(
    "https://api.runcrate.ai/v1/models",
    {
      method: "GET",
      headers: { Authorization: "Bearer test-key" },
    },
  );
  expect(result.models).toEqual(
    expect.arrayContaining([
      "deepseek-ai/DeepSeek-V4.1-Flash",
      "openai/gpt-oss-120b",
    ]),
  );
  expect(result.models).toHaveLength(2);
  expect(result.ignored).toEqual(
    expect.arrayContaining([
      { id: "meta-llama/Llama-Guard-4-12B", reasons: ["no_tool"] },
      { id: "black-forest-labs/FLUX.2-dev", reasons: ["not_llm", "no_tool"] },
      { id: "openai/whisper-large-v3", reasons: ["no_text_input", "no_tool"] },
    ]),
  );
  expect(result.metadata).toEqual({
    "deepseek-ai/DeepSeek-V4.1-Flash": { input_modalities: ["text", "image"] },
    "openai/gpt-oss-120b": { input_modalities: ["text"] },
  });
});

it("returns no models when the catalog request fails", async () => {
  vi.mocked(providerFetch).mockResolvedValue(
    new Response("unavailable", { status: 503 }),
  );
  expect(
    await listRuncrateModels("https://api.runcrate.ai/v1", "test-key"),
  ).toEqual({ models: [], ignored: [], metadata: {} });
});
