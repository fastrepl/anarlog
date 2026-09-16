import assert from "node:assert/strict";
import test from "node:test";

import { resolveCloudSyncOptIn } from "./opt-in-model.ts";

const claimedBinding = JSON.stringify({
  workspace_id: "user-a",
  account_user_id: "user-a",
});

test("stays off until the user opts in", () => {
  assert.equal(resolveCloudSyncOptIn([]), false);
  assert.equal(
    resolveCloudSyncOptIn([
      { account_user_id: "user-a", preference_json: null, binding_json: null },
    ]),
    false,
  );
});

test("keeps syncing on a device already claimed for the account", () => {
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-a",
        preference_json: null,
        binding_json: claimedBinding,
      },
    ]),
    true,
  );
});

test("does not claim a device bound to another account on sign-in", () => {
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-b",
        preference_json: null,
        binding_json: claimedBinding,
      },
    ]),
    false,
  );
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-b",
        preference_json: null,
        binding_json: JSON.stringify({
          workspace_id: "local-workspace",
          account_user_id: "user-b",
        }),
      },
    ]),
    false,
  );
});

test("an explicit preference wins over the binding", () => {
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-a",
        preference_json: "false",
        binding_json: claimedBinding,
      },
    ]),
    false,
  );
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-b",
        preference_json: "true",
        binding_json: claimedBinding,
      },
    ]),
    true,
  );
});

test("ignores malformed stored values", () => {
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-a",
        preference_json: "not-json",
        binding_json: "not-json",
      },
    ]),
    false,
  );
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-a",
        preference_json: '"yes"',
        binding_json: claimedBinding,
      },
    ]),
    true,
  );
});

test("resumes an explicitly connected library and respects sync being disabled", () => {
  const row = {
    account_user_id: "user-b",
    binding_json: claimedBinding,
    preference_json: null,
    connected_account: 1,
  };
  assert.equal(resolveCloudSyncOptIn([row]), true);
  assert.equal(
    resolveCloudSyncOptIn([{ ...row, preference_json: "false" }]),
    false,
  );
});

test("does not fall back to a legacy binding for an inactive connection", () => {
  assert.equal(
    resolveCloudSyncOptIn([
      {
        account_user_id: "user-a",
        preference_json: null,
        binding_json: claimedBinding,
        connected_account: 0,
        has_connections: 1,
      },
    ]),
    false,
  );
});
