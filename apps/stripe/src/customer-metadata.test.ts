import { expect, test } from "bun:test";

import {
  getCustomerIdentityMetadata,
  getCustomerOwner,
} from "./customer-metadata";

test.each([
  [{ userId: "owner-user", user_id: "other-user" }, null],
  [
    { userId: "owner-user", user_id: "owner-user" },
    { kind: "user", id: "owner-user" },
  ],
  [{ workspaceId: "workspace-one", workspace_id: "workspace-two" }, null],
  [
    { workspaceId: "workspace-one" },
    { kind: "workspace", id: "workspace-one" },
  ],
  [{ userId: "owner-user", workspaceId: "workspace-one" }, null],
] as const)("resolves customer owner metadata %#", (metadata, expected) => {
  expect(getCustomerOwner(metadata)).toEqual(expected);
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
