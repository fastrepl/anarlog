import { describe, expect, test } from "vitest";

import { llmHealthErrorMessage } from "./health";

describe("llmHealthErrorMessage", () => {
  test("prefers provider error payloads over a generic Bad Request", () => {
    expect(
      llmHealthErrorMessage({
        message: "Bad Request",
        data: { error: { message: "missing ChatGPT-Account-ID" } },
      }),
    ).toBe("missing ChatGPT-Account-ID");
  });

  test("reads JSON response bodies", () => {
    expect(
      llmHealthErrorMessage({
        message: "Bad Request",
        responseBody: '{"error":{"message":"invalid_model"}}',
      }),
    ).toBe("invalid_model");
  });

  test("falls back to the first useful error line", () => {
    expect(
      llmHealthErrorMessage({
        message: 'Bad Request\nRequest body: {"prompt":"Hi"}',
      }),
    ).toBe("Bad Request");
  });
});
