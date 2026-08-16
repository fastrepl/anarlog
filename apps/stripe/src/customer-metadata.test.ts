import { expect, test } from "bun:test";

import {
  getCustomerIdentityMetadata,
  getCustomerOwner,
  getCustomerUserId,
  getCustomerWorkspaceId,
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

test("requires every Stripe workspace alias to agree", () => {
  expect(
    getCustomerWorkspaceId({
      workspaceId: "workspace-one",
      workspace_id: "workspace-two",
    }),
  ).toBeNull();
});

test("returns the shared Stripe workspace", () => {
  expect(
    getCustomerWorkspaceId({
      workspaceId: "workspace-one",
      workspace_id: "workspace-one",
    }),
  ).toBe("workspace-one");
});

test("rejects ambiguous user and workspace ownership", () => {
  expect(
    getCustomerOwner({
      userId: "owner-user",
      workspaceId: "workspace-one",
    }),
  ).toBeNull();
});

test("returns an unambiguous workspace owner", () => {
  expect(getCustomerOwner({ workspaceId: "workspace-one" })).toEqual({
    kind: "workspace",
    id: "workspace-one",
  });
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
