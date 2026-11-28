import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  GOOGLE_VERTEX_AI_MODELS,
  listGoogleVertexAIModels,
} from "./list-google-vertex-ai";

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

beforeEach(() => {
  vi.mocked(tauriFetch).mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("listGoogleVertexAIModels", () => {
  test("lists models from an OpenAI-compatible Vertex endpoint", async () => {
    vi.mocked(tauriFetch).mockResolvedValueOnce(
      Response.json({
        data: [{ id: "google/gemini-3.8-flash" }, { id: "text-embedding-005" }],
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

  test("falls back to curated models when discovery exceeds REQUEST_TIMEOUT", async () => {
    vi.useFakeTimers();
    let aborted = false;
    vi.mocked(tauriFetch).mockImplementationOnce(
      (_input, init) =>
        new Promise((_resolve, reject) => {
          const signal = (init as RequestInit | undefined)?.signal;
          signal?.addEventListener(
            "abort",
            () => {
              aborted = true;
              reject(new Error("aborted"));
            },
            { once: true },
          );
        }),
    );

    const promise = listGoogleVertexAIModels(
      "https://us-central1-aiplatform.googleapis.com/v1beta1/projects/p/locations/us-central1/endpoints/openapi",
      "vertex-token",
    );
    await vi.advanceTimersByTimeAsync(5_000);
    const result = await promise;

    expect(aborted).toBe(true);
    expect(result.models).toEqual([...GOOGLE_VERTEX_AI_MODELS]);
  });
});
