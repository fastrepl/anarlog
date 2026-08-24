import { Effect } from "effect";
import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  fetchJson: vi.fn(),
  resolveSubscriptionAccess: vi.fn(),
}));

vi.mock("./access", () => ({
  resolveSubscriptionAccess: mocks.resolveSubscriptionAccess,
}));

vi.mock("~/settings/ai/shared/list-common", async (importOriginal) => ({
  ...(await importOriginal()),
  fetchJson: mocks.fetchJson,
}));

import { listSubscriptionModels } from "./models";

describe("ChatGPT subscription models", () => {
  beforeEach(() => {
    mocks.fetchJson.mockReset();
    mocks.resolveSubscriptionAccess.mockReset();
    mocks.resolveSubscriptionAccess.mockResolvedValue({
      token: "access-token",
      credential: { accountId: "account-1" },
    });
  });

  test("parses the Codex catalog and omits hidden models", async () => {
    mocks.fetchJson.mockReturnValue(
      Effect.succeed({
        models: [
          { slug: "gpt-5.6-sol", visibility: "list" },
          { slug: "codex-auto-review", visibility: "hide" },
          {
            slug: "gpt-5.3-codex-spark",
            visibility: "list",
            supported_in_api: false,
          },
        ],
      }),
    );

    await expect(
      listSubscriptionModels(
        "chatgpt",
        "https://api.openai.com/v1",
        "stored-credential",
      ),
    ).resolves.toMatchObject({
      models: ["gpt-5.6-sol", "gpt-5.3-codex-spark"],
    });
    expect(mocks.fetchJson).toHaveBeenCalledWith(
      "https://chatgpt.com/backend-api/codex/models?client_version=0.145.0",
      expect.objectContaining({
        Authorization: "Bearer access-token",
        "ChatGPT-Account-ID": "account-1",
      }),
    );
  });

  test("does not offer stale fallback models when discovery fails", async () => {
    mocks.fetchJson.mockReturnValue(Effect.fail(new Error("unavailable")));

    await expect(
      listSubscriptionModels(
        "chatgpt",
        "https://api.openai.com/v1",
        "stored-credential",
      ),
    ).resolves.toEqual({ models: [], ignored: [], metadata: {} });
  });
});
