import { describe, expect, test } from "vitest";

import {
  CLOUDFLARE_WORKERS_AI_MODELS,
  createStaticCloudflareWorkersAIModelResult,
} from "./list-cloudflare-workers-ai";

describe("createStaticCloudflareWorkersAIModelResult", () => {
  test("keeps a selectable fallback for Workers AI chat models", () => {
    const result = createStaticCloudflareWorkersAIModelResult();

    expect(result.models).toEqual([...CLOUDFLARE_WORKERS_AI_MODELS]);
    expect(result.models[0]).toBe("@cf/moonshotai/kimi-k2.6");
    expect(result.metadata["@cf/moonshotai/kimi-k2.6"]).toEqual({
      input_modalities: ["text", "image"],
    });
    expect(result.metadata["@cf/openai/gpt-oss-120b"]).toEqual({
      input_modalities: ["text"],
    });
  });
});
