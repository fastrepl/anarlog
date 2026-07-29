import { describe, expect, it } from "vitest";

import { normalizeLLMProviderId } from "./useLLMConnection";

describe("normalizeLLMProviderId", () => {
  it("maps the legacy hosted provider id to Anarlog", () => {
    expect(normalizeLLMProviderId("hyprnote")).toBe("anarlog");
  });

  it("preserves current provider ids", () => {
    expect(normalizeLLMProviderId("openai")).toBe("openai");
  });
});
