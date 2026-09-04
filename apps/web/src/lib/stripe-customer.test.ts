import assert from "node:assert/strict";
import test from "node:test";

import {
  getStripeCustomerIdentityMetadata,
  getStripeCustomerOwnership,
  isUnavailableStripeCustomerError,
} from "./stripe-customer.ts";

test("recognizes stale personal Stripe customer references", () => {
  assert.equal(
    isUnavailableStripeCustomerError({ code: "resource_missing" }),
    true,
  );
  assert.equal(
    isUnavailableStripeCustomerError({ raw: { code: "resource_missing" } }),
    true,
  );
  assert.equal(
    isUnavailableStripeCustomerError(
      new Error("Stripe customer does not belong to authenticated user"),
    ),
    true,
  );
  assert.equal(
    isUnavailableStripeCustomerError(new Error("Stripe is unavailable")),
    false,
  );
});

test("Stripe metadata must belong to the authenticated user", () => {
  assert.equal(
    getStripeCustomerOwnership(
      { metadata: { userId: "other-user" }, email: "owner@example.com" },
      { id: "owner-user", email: "owner@example.com" },
    ),
    "unowned",
  );
});

test("a legacy email match is claimable only when owner metadata is absent", () => {
  assert.equal(
    getStripeCustomerOwnership(
      { metadata: {}, email: " Owner@Example.com " },
      { id: "owner-user", email: "owner@example.com" },
    ),
    "claimable",
  );
});

test("matching owner metadata is authoritative", () => {
  assert.equal(
    getStripeCustomerOwnership(
      { metadata: { user_id: "owner-user" }, email: "old@example.com" },
      { id: "owner-user", email: "new@example.com" },
    ),
    "owned",
  );
});

test("every nonempty owner metadata alias must match", () => {
  assert.equal(
    getStripeCustomerOwnership(
      {
        metadata: {
          userId: "owner-user",
          user_id: "other-user",
          userID: "owner-user",
        },
        email: "owner@example.com",
      },
      { id: "owner-user", email: "owner@example.com" },
    ),
    "unowned",
  );
});

test("empty aliases do not hide a conflicting owner", () => {
  assert.equal(
    getStripeCustomerOwnership(
      {
        metadata: { userId: "", user_id: "other-user" },
        email: "owner@example.com",
      },
      { id: "owner-user", email: "owner@example.com" },
    ),
    "unowned",
  );

  assert.equal(
    getStripeCustomerOwnership(
      { metadata: { userId: "", user_id: "" }, email: "owner@example.com" },
      { id: "owner-user", email: "owner@example.com" },
    ),
    "claimable",
  );
});

test("repairs missing Stripe identity metadata", () => {
  assert.deepEqual(
    getStripeCustomerIdentityMetadata({ user_id: "owner-user" }, "owner-user"),
    {
      userId: "owner-user",
      posthog_person_distinct_id: "owner-user",
    },
  );
});

test("leaves complete Stripe identity metadata unchanged", () => {
  assert.equal(
    getStripeCustomerIdentityMetadata(
      {
        userId: "owner-user",
        posthog_person_distinct_id: "owner-user",
      },
      "owner-user",
    ),
    null,
  );
});
