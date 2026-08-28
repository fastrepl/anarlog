import assert from "node:assert/strict";
import test from "node:test";

import {
  getYcVerificationApiUrl,
  isYcVerificationUrl,
  normalizeYcVerificationUrl,
  parseYcPerkApplyValue,
  validateYcPerkApplyValue,
  validateYcVerificationUrl,
  verifyYcFounder,
  ycPerkRequestSchema,
} from "./yc-perk.ts";

test("accepts YC founder verification links", () => {
  assert.equal(
    isYcVerificationUrl("https://www.ycombinator.com/verify/founder-token"),
    true,
  );
  assert.equal(
    isYcVerificationUrl("https://ycombinator.com/verify/founder_token/"),
    true,
  );
});

test("rejects lookalike and generic YC URLs", () => {
  assert.equal(
    isYcVerificationUrl(
      "https://www.ycombinator.com.evil.example/verify/founder-token",
    ),
    false,
  );
  assert.equal(
    isYcVerificationUrl("https://www.ycombinator.com/verify"),
    false,
  );
  assert.equal(
    isYcVerificationUrl("http://www.ycombinator.com/verify/founder-token"),
    false,
  );
});

test("validates complete YC perk requests", () => {
  assert.equal(
    ycPerkRequestSchema.safeParse({
      verificationUrl: "https://www.ycombinator.com/verify/founder-token",
      additionalComments: "",
    }).success,
    true,
  );
  assert.equal(
    ycPerkRequestSchema.safeParse({
      verificationUrl: "https://example.com/verify/founder-token",
      additionalComments: "",
    }).success,
    false,
  );
});

test("normalizes founder verification links", () => {
  assert.equal(
    normalizeYcVerificationUrl(
      " https://ycombinator.com/verify/founder-token/?source=deal#details ",
    ),
    "https://www.ycombinator.com/verify/founder-token",
  );
  assert.equal(
    getYcVerificationApiUrl(
      "https://www.ycombinator.com/verify/founder-token/",
    ),
    "https://www.ycombinator.com/verify/founder-token.json",
  );
});

test("parses YC verification links and promotion codes for account apply", () => {
  assert.deepEqual(parseYcPerkApplyValue("  YC-0123456789abcdef01234567  "), {
    type: "promotion_code",
    code: "YC-0123456789ABCDEF01234567",
  });
  assert.deepEqual(
    parseYcPerkApplyValue(
      " https://ycombinator.com/verify/founder-token/?source=deal ",
    ),
    {
      type: "verification_url",
      verificationUrl: "https://www.ycombinator.com/verify/founder-token",
    },
  );
  assert.equal(
    validateYcPerkApplyValue("https://example.com/verify/founder-token"),
    "Use your ycombinator.com/verify link",
  );
  assert.equal(
    validateYcPerkApplyValue("SAVE20"),
    "Paste your YC verification link or YC- code",
  );
});

test("returns field-level validation messages", () => {
  assert.equal(
    validateYcVerificationUrl("https://example.com/verify/founder-token"),
    "Use your ycombinator.com/verify link",
  );
});

test("verifies founders and returns their normalized YC email", async () => {
  const requestedUrls: string[] = [];
  const result = await verifyYcFounder({
    verificationUrl: "https://ycombinator.com/verify/founder-token/",
    fetcher: (async (url) => {
      requestedUrls.push(String(url));
      return Response.json({
        verified: true,
        name: "Ada Lovelace",
        email: "Founder@Example.com",
        companies: [{ name: "Analytical Engines", batch: "S25" }],
      });
    }) as typeof fetch,
  });

  assert.deepEqual(result, {
    status: "verified",
    firstName: "Ada",
    email: "founder@example.com",
  });
  assert.deepEqual(requestedUrls, [
    "https://www.ycombinator.com/verify/founder-token.json",
  ]);
});

test("rejects inactive verification links", async () => {
  const result = await verifyYcFounder({
    verificationUrl: "https://www.ycombinator.com/verify/founder-token",
    fetcher: (async () => Response.json({ verified: false })) as typeof fetch,
  });

  assert.deepEqual(result, { status: "invalid", reason: "not_verified" });
});

test("requires the YC verification email", async () => {
  const result = await verifyYcFounder({
    verificationUrl: "https://www.ycombinator.com/verify/founder-token",
    fetcher: (async () =>
      Response.json({ verified: true, name: "Ada Lovelace" })) as typeof fetch,
  });

  assert.deepEqual(result, { status: "invalid", reason: "email_missing" });
});

test("fails closed on unexpected YC responses", async () => {
  await assert.rejects(
    verifyYcFounder({
      verificationUrl: "https://www.ycombinator.com/verify/founder-token",
      fetcher: (async () => Response.json({ verified: "yes" })) as typeof fetch,
    }),
    /unexpected response/,
  );
});
