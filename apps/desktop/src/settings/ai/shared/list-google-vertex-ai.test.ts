import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  GOOGLE_VERTEX_AI_MODELS,
  listGoogleVertexAIModels,
} from "./list-google-vertex-ai";

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

beforeEach(() => {
  vi.mocked(tauriFetch).mockReset();
});

describe("listGoogleVertexAIModels", () => {
  test("lists models from an OpenAI-compatible Vertex endpoint", async () => {
    vi.mocked(tauriFetch).mockResolvedValueOnce(
      Response.json({
        data: [
          { id: "google/gemini-3.8-flash" },
          { id: "text-embedding-005" },
        ],
      }),
    );

    const result = await listGoogleVertexAIModels(
      "https://us-central1-aiplatform.googleapis.com/v1beta1/projects/p/locations/us-central1/endpoints/openapi",
      "vertex-token",
    );

    expect(result.models).toEqual(["google/gemini-3.8-flash"]);
    expect(result.metadata["google/gemini-3.8-flash"]).toEqual({
      input_modalities: ["text", "image"],
    });
  });

  test("falls back to publisherModels endpoint on aiplatform hosts", async () => {
    vi.mocked(tauriFetch)
      .mockResolvedValueOnce(new Response(null, { status: 404 }))
      .mockResolvedValueOnce(
        Response.json({
          publisherModels: [
            { name: "publishers/google/models/gemini-3.5-flash" },
          ],
        }),
      );

    const result = await listGoogleVertexAIModels(
      "https://us-central1-aiplatform.googleapis.com/v1beta1/projects/p/locations/us-central1/endpoints/openapi",
      "vertex-token",
    );

    expect(result.models).toEqual(["google/gemini-3.5-flash"]);
  });

  test("falls back to curated models when request fails or credentials are absent", async () => {
    vi.mocked(tauriFetch).mockRejectedValueOnce(new Error("offline"));

    expect(
      (await listGoogleVertexAIModels("https://vertex.example/v1", "tok"))
        .models,
    ).toEqual([...GOOGLE_VERTEX_AI_MODELS]);
    expect((await listGoogleVertexAIModels("", "")).models).toEqual([
      ...GOOGLE_VERTEX_AI_MODELS,
    ]);
  });
});
