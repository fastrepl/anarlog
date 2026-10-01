import { beforeEach, expect, it, vi } from "vitest";

import { listCheaperInferenceModels } from "./list-cheaperinference";

import { providerFetch } from "~/ai/provider-fetch";

vi.mock("~/ai/provider-fetch", () => ({ providerFetch: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

it("uses the Cheaper Inference text catalog and preserves model IDs and vision capabilities", async () => {
  vi.mocked(providerFetch).mockResolvedValue(
    Response.json({
      data: [
        { id: "gpt-5.4-mini", type: "text", capabilities: { vision: false } },
        { id: "vision-model", type: "text", capabilities: { vision: true } },
        { id: "image-model", type: "image", capabilities: { vision: false } },
      ],
    }),
  );
  const result = await listCheaperInferenceModels(
    "https://api.cheaperinference.com/v1/",
    "test-key",
  );
  expect(providerFetch).toHaveBeenCalledWith(
    "https://api.cheaperinference.com/v1/models?type=text",
    {
      method: "GET",
      headers: { Authorization: "Bearer test-key" },
    },
  );
  expect(result.models).toEqual(
    expect.arrayContaining(["gpt-5.4-mini", "vision-model"]),
  );
  expect(result.models).toHaveLength(2);
  expect(result.metadata).toEqual({
    "gpt-5.4-mini": { input_modalities: ["text"] },
    "vision-model": { input_modalities: ["text", "image"] },
  });
});

it("returns no models when the catalog request fails", async () => {
  vi.mocked(providerFetch).mockResolvedValue(
    new Response("unavailable", { status: 503 }),
  );
  expect(
    await listCheaperInferenceModels(
      "https://api.cheaperinference.com/v1",
      "test-key",
    ),
  ).toEqual({ models: [], ignored: [], metadata: {} });
});
