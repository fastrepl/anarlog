import { describe, expect, test } from "vitest";

import { processOpenRouterModels } from "./list-openrouter";

describe("processOpenRouterModels", () => {
  test("keeps current provider-prefixed dated models", () => {
    expect(
      processOpenRouterModels([
        {
          id: "mistralai/mistral-large-2512",
          supported_parameters: ["tools", "tool_choice"],
          architecture: { input_modalities: ["text"] },
        },
        {
          id: "anthropic/claude-haiku-4-5-20251001",
          supported_parameters: ["tools", "tool_choice"],
          architecture: { input_modalities: ["text", "image"] },
        },
        {
          id: "mistralai/mistral-large-2508",
          supported_parameters: ["tools", "tool_choice"],
          architecture: { input_modalities: ["text"] },
        },
      ]),
    ).toEqual({
      models: [
        "anthropic/claude-haiku-4-5-20251001",
        "mistralai/mistral-large-2512",
      ],
      ignored: [{ id: "mistralai/mistral-large-2508", reasons: ["old_model"] }],
      metadata: {
        "anthropic/claude-haiku-4-5-20251001": {
          input_modalities: ["text", "image"],
        },
        "mistralai/mistral-large-2512": { input_modalities: ["text"] },
        "mistralai/mistral-large-2508": { input_modalities: ["text"] },
      },
    });
  });
});
