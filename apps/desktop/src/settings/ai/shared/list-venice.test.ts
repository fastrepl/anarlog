import { beforeEach, expect, it, vi } from "vitest";

import { listVeniceModels } from "./list-venice";

import { providerFetch } from "~/ai/provider-fetch";

vi.mock("~/ai/provider-fetch", () => ({ providerFetch: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

it("uses Venice's text catalog and preserves provider model IDs and vision capabilities", async () => {
  vi.mocked(providerFetch).mockResolvedValue(
    Response.json({
      data: [
        {
          id: "venice-uncensored",
          type: "text",
          model_spec: { capabilities: {} },
        },
        {
          id: "vision-model",
          type: "text",
          model_spec: { capabilities: { supportsVision: true } },
        },
        {
          id: "offline",
          type: "text",
          model_spec: { offline: true, capabilities: {} },
        },
        { id: "image-model", type: "image", model_spec: { capabilities: {} } },
      ],
    }),
  );
  const result = await listVeniceModels(
    "https://api.venice.ai/api/v1/",
    "test-key",
  );
  expect(providerFetch).toHaveBeenCalledWith(
    "https://api.venice.ai/api/v1/models?type=text",
    {
      method: "GET",
      headers: { Authorization: "Bearer test-key" },
    },
  );
  expect(result.models).toEqual(
    expect.arrayContaining(["venice-uncensored", "vision-model"]),
  );
  expect(result.models).toHaveLength(2);
  expect(result.metadata).toEqual({
    "venice-uncensored": { input_modalities: ["text"] },
    "vision-model": { input_modalities: ["text", "image"] },
  });
});

it("returns no models when the catalog request fails", async () => {
  vi.mocked(providerFetch).mockResolvedValue(
    new Response("unavailable", { status: 503 }),
  );
  expect(
    await listVeniceModels("https://api.venice.ai/api/v1", "test-key"),
  ).toEqual({ models: [], ignored: [], metadata: {} });
});
