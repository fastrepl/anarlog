import { APICallError } from "ai";
import { describe, expect, it } from "vitest";

import { extractUnderlyingError } from "./tasks";

describe("extractUnderlyingError", () => {
  it("normalizes exhausted provider overload retries", () => {
    const retryError = new Error(
      "Failed after 3 attempts. Last error: Overloaded",
    );
    retryError.name = "AI_RetryError";
    (retryError as any).lastError = new Error("Overloaded");

    expect(extractUnderlyingError(retryError).message).toBe(
      "The AI model is overloaded right now. Wait a moment, then retry.",
    );
  });

  it("normalizes retryable API call failures", () => {
    const error = new APICallError({
      message: "Service unavailable",
      url: "https://example.com",
      requestBodyValues: {},
      statusCode: 503,
    });

    expect(extractUnderlyingError(error).message).toBe(
      "The AI model is overloaded right now. Wait a moment, then retry.",
    );
  });

  it("preserves API conflict errors", () => {
    const error = new APICallError({
      message: "Conflict",
      url: "https://example.com",
      requestBodyValues: {},
      statusCode: 409,
    });

    expect(extractUnderlyingError(error)).toBe(error);
  });

  it("preserves non-transient errors", () => {
    const error = new Error("Invalid API key");

    expect(extractUnderlyingError(error)).toBe(error);
  });
});
