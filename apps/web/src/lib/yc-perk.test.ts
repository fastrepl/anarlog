import assert from "node:assert/strict";
import test from "node:test";

import {
  getYcVerificationApiUrl,
  isYcVerificationUrl,
  normalizeYcVerificationUrl,
  validateYcPerkEmail,
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
      email: "founder@example.com",
      verificationUrl: "https://www.ycombinator.com/verify/founder-token",
      additionalComments: "",
    }).success,
    true,
  );
  assert.equal(
    ycPerkRequestSchema.safeParse({
      email: "not-an-email",
      verificationUrl: "https://www.ycombinator.com/verify/founder-token",
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

test("returns field-level validation messages", () => {
  assert.equal(validateYcPerkEmail("founder@example.com"), undefined);
  assert.equal(validateYcPerkEmail("not-an-email"), "Enter a valid email");
  assert.equal(
    validateYcVerificationUrl("https://example.com/verify/founder-token"),
    "Use your ycombinator.com/verify link",
  );
});

test("verifies founders when the YC email matches", async () => {
  const requestedUrls: string[] = [];
  const result = await verifyYcFounder({
    email: "founder@example.com",
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

  assert.deepEqual(result, { status: "verified", firstName: "Ada" });
  assert.deepEqual(requestedUrls, [
    "https://www.ycombinator.com/verify/founder-token.json",
  ]);
});

test("rejects inactive verification links", async () => {
  const result = await verifyYcFounder({
    email: "founder@example.com",
    verificationUrl: "https://www.ycombinator.com/verify/founder-token",
    fetcher: (async () => Response.json({ verified: false })) as typeof fetch,
  });

  assert.deepEqual(result, { status: "invalid", reason: "not_verified" });
});

test("requires the YC verification email", async () => {
  const result = await verifyYcFounder({
    email: "founder@example.com",
    verificationUrl: "https://www.ycombinator.com/verify/founder-token",
    fetcher: (async () =>
      Response.json({ verified: true, name: "Ada Lovelace" })) as typeof fetch,
  });

  assert.deepEqual(result, { status: "invalid", reason: "email_missing" });
});

test("requires the submitted email to match YC", async () => {
  const result = await verifyYcFounder({
    email: "other@example.com",
    verificationUrl: "https://www.ycombinator.com/verify/founder-token",
    fetcher: (async () =>
      Response.json({
        verified: true,
        name: "Ada Lovelace",
        email: "founder@example.com",
      })) as typeof fetch,
  });

  assert.deepEqual(result, { status: "invalid", reason: "email_mismatch" });
});

test("fails closed on unexpected YC responses", async () => {
  await assert.rejects(
    verifyYcFounder({
      email: "founder@example.com",
      verificationUrl: "https://www.ycombinator.com/verify/founder-token",
      fetcher: (async () => Response.json({ verified: "yes" })) as typeof fetch,
    }),
    /unexpected response/,
  );
});
