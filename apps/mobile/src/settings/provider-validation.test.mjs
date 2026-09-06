import assert from "node:assert/strict";
import { test } from "node:test";

import { providersFor, validateProviderConfig } from "./providers-model.ts";

test("incomplete custom endpoints show actionable validation instead of a URL exception", () => {
  for (const kind of ["stt", "llm"]) {
    for (const provider of providersFor(kind).filter(
      ({ id, baseUrl }) => id !== "anarlog" && !baseUrl,
    )) {
      for (const baseUrl of ["", "   ", "not-a-url", "https://"]) {
        assert.throws(
          () =>
            validateProviderConfig(kind, {
              provider: provider.id,
              baseUrl,
              model: "test-model",
            }),
          {
            name: "Error",
            message:
              "Enter an HTTPS base URL without credentials or query parameters.",
          },
          `${kind}/${provider.id}: ${JSON.stringify(baseUrl)}`,
        );
      }
      assert.equal(
        validateProviderConfig(kind, {
          provider: provider.id,
          baseUrl: "https://provider.example/v1/",
          model: "test-model",
        }).baseUrl,
        "https://provider.example/v1",
      );
    }
  }
});
