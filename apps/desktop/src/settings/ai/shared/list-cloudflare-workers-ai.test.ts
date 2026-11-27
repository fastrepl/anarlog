import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  CLOUDFLARE_WORKERS_AI_MODELS,
  createStaticCloudflareWorkersAIModelResult,
  listCloudflareWorkersAIModels,
} from "./list-cloudflare-workers-ai";

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

beforeEach(() => {
  vi.mocked(tauriFetch).mockReset();
});

describe("createStaticCloudflareWorkersAIModelResult", () => {
  test("keeps a selectable fallback for Workers AI chat models", () => {
    const result = createStaticCloudflareWorkersAIModelResult();

    expect(result.models).toEqual([...CLOUDFLARE_WORKERS_AI_MODELS]);
    expect(result.models[0]).toBe("@cf/moonshotai/kimi-k2.7-code");
    expect(result.metadata["@cf/moonshotai/kimi-k2.7-code"]).toEqual({
      input_modalities: ["text", "image"],
    });
    expect(result.metadata["@cf/zai-org/glm-5.2"]).toEqual({
      input_modalities: ["text"],
    });
    expect(result.metadata["@cf/openai/gpt-oss-120b"]).toEqual({
      input_modalities: ["text"],
    });
  });
});

describe("listCloudflareWorkersAIModels", () => {
  test("fetches live models from the OpenAI-compatible endpoint", async () => {
    vi.mocked(tauriFetch).mockResolvedValueOnce(
      Response.json({
        data: [
          { id: "@cf/moonshotai/kimi-k2.7-code" },
          { id: "@cf/qwen/qwen3-coder" },
        ],
      }),
    );

    const result = await listCloudflareWorkersAIModels(
      "https://api.cloudflare.com/client/v4/accounts/acct/ai/v1",
      "cf-key",
    );

    expect(vi.mocked(tauriFetch)).toHaveBeenCalledWith(
      "https://api.cloudflare.com/client/v4/accounts/acct/ai/v1/models",
      expect.objectContaining({ method: "GET" }),
    );
    expect(result.models).toEqual([
      "@cf/moonshotai/kimi-k2.7-code",
      "@cf/qwen/qwen3-coder",
    ]);
    expect(result.metadata["@cf/moonshotai/kimi-k2.7-code"]).toEqual({
      input_modalities: ["text", "image"],
    });
  });

  test("falls back to Cloudflare Workers AI models/search endpoint when /models is unavailable", async () => {
    vi.mocked(tauriFetch)
      .mockResolvedValueOnce(new Response(null, { status: 404 }))
      .mockResolvedValueOnce(
        Response.json({
          result: [
            {
              name: "@cf/meta/llama-4-scout-17b-16e-instruct",
              task: { name: "Text Generation" },
            },
            {
              name: "@cf/baai/bge-m3",
              task: { name: "Text Embeddings" },
            },
          ],
        }),
      );

    const result = await listCloudflareWorkersAIModels(
      "https://api.cloudflare.com/client/v4/accounts/acct/ai/v1",
      "cf-key",
    );

    expect(result.models).toEqual(["@cf/meta/llama-4-scout-17b-16e-instruct"]);
  });

  test("falls back to curated models when fetching fails", async () => {
    vi.mocked(tauriFetch).mockRejectedValueOnce(new Error("network error"));

    const result = await listCloudflareWorkersAIModels(
      "https://api.cloudflare.com/client/v4/accounts/acct/ai/v1",
      "cf-key",
    );

    expect(result.models).toEqual([...CLOUDFLARE_WORKERS_AI_MODELS]);
  });
});
