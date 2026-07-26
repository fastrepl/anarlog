import { expect, test } from "bun:test";

import {
  getCustomerIdentityMetadata,
  getCustomerUserId,
} from "./customer-metadata";

test("requires every Stripe owner alias to agree", () => {
  expect(
    getCustomerUserId({
      userId: "owner-user",
      user_id: "other-user",
    }),
  ).toBeNull();
});

test("returns the shared Stripe owner", () => {
  expect(
    getCustomerUserId({
      userId: "owner-user",
      user_id: "owner-user",
    }),
  ).toBe("owner-user");
});

test("repairs incomplete PostHog identity metadata", () => {
  expect(
    getCustomerIdentityMetadata({ userId: "owner-user" }, "owner-user"),
  ).toEqual({
    userId: "owner-user",
    posthog_person_distinct_id: "owner-user",
  });
});

test("does not rewrite complete identity metadata", () => {
  expect(
    getCustomerIdentityMetadata(
      {
        userId: "owner-user",
        posthog_person_distinct_id: "owner-user",
      },
      "owner-user",
    ),
  ).toBeNull();
});
