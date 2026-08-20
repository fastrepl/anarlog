import { describe, expect, test } from "vitest";

import {
  isOAuthCredentialFresh,
  parseOAuthCredential,
  serializeOAuthCredential,
} from "./credential";

describe("OAuth credentials", () => {
  test("round-trips a valid credential", () => {
    const credential = {
      type: "oauth" as const,
      refresh: "refresh-token",
      access: "access-token",
      expires: Date.now() + 60_000,
      accountId: "acct_1",
    };

    expect(parseOAuthCredential(serializeOAuthCredential(credential))).toEqual(
      credential,
    );
  });

  test("rejects API keys and malformed JSON", () => {
    expect(parseOAuthCredential("sk-test")).toBeNull();
    expect(parseOAuthCredential("{not-json")).toBeNull();
    expect(parseOAuthCredential(JSON.stringify({ type: "api" }))).toBeNull();
  });

  test("treats tokens near expiry as stale", () => {
    expect(
      isOAuthCredentialFresh(
        {
          type: "oauth",
          refresh: "refresh",
          access: "access",
          expires: Date.now() + 30_000,
        },
        Date.now(),
      ),
    ).toBe(false);
    expect(
      isOAuthCredentialFresh(
        {
          type: "oauth",
          refresh: "refresh",
          access: "access",
          expires: Date.now() + 10 * 60 * 1000,
        },
        Date.now(),
      ),
    ).toBe(true);
  });
});
