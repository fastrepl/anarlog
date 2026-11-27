import { describe, expect, test, vi } from "vitest";

import {
  ANARLOG_PRO_LLM_MODELS,
  anarlogProLlmModelLabel,
  createStaticAnarlogProModelResult,
  listAnarlogProModels,
} from "./anarlog-pro-models";

describe("listAnarlogProModels", () => {
  test("fetches live models and registers their labels", async () => {
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(
      Response.json({
        object: "list",
        data: [
          {
            id: "Auto",
            label: "Pro (Cloud)",
            input_modalities: ["text", "image"],
          },
          {
            id: "~openai/gpt-live-new",
            label: "GPT Live New",
            input_modalities: ["text"],
          },
        ],
      }),
    );

    const result = await listAnarlogProModels(
      "https://api.anarlog.test/llm/",
      fetcher,
    );

    expect(fetcher).toHaveBeenCalledWith(
      "https://api.anarlog.test/llm/models",
      { method: "GET" },
    );
    expect(result.models).toEqual(["Auto", "~openai/gpt-live-new"]);
    expect(result.metadata["~openai/gpt-live-new"]).toEqual({
      input_modalities: ["text"],
    });
    expect(anarlogProLlmModelLabel("~openai/gpt-live-new")).toBe(
      "GPT Live New",
    );
  });

  test("falls back to static models on HTTP failure", async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response("error", { status: 503 }));

    const result = await listAnarlogProModels(
      "https://api.anarlog.test/llm",
      fetcher,
    );

    expect(result).toEqual(createStaticAnarlogProModelResult());
    expect(result.models).toEqual(ANARLOG_PRO_LLM_MODELS.map(({ id }) => id));
  });
});
