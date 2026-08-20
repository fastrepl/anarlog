import { describe, expect, it, vi } from "vitest";

import { omitTemperatureMiddleware } from "./omit-temperature-middleware";

describe("omitTemperatureMiddleware", () => {
  it("removes temperature from generation params", async () => {
    const transformParams = omitTemperatureMiddleware.transformParams;
    expect(transformParams).toEqual(expect.any(Function));

    const params = {
      prompt: [
        {
          role: "user" as const,
          content: [{ type: "text" as const, text: "Hi" }],
        },
      ],
      temperature: 0,
      maxOutputTokens: 8192,
    };

    const next = await transformParams!({
      type: "generate",
      params,
      model: {
        specificationVersion: "v3",
        provider: "openai",
        modelId: "gpt-5.4",
        supportedUrls: {},
        doGenerate: vi.fn(),
        doStream: vi.fn(),
      },
    });

    expect(next).not.toHaveProperty("temperature");
    expect(next.maxOutputTokens).toBe(8192);
  });
});
