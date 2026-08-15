import { beforeEach, expect, test, vi } from "vitest";

import { sealWorkspaceE2eeKeyForRecipients } from "@anlg/plugin-db";

import type { ProjectedCloudsyncCredentials } from "./cloudsync-credentials";
import { provisionMissingWorkspaceKeys } from "./cloudsync-workspace-keys";

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.test" },
}));

const OWNER_ID = "11111111-1111-4111-8111-111111111111";
const MEMBER_ID = "22222222-2222-4222-8222-222222222222";
const WORKSPACE_ID = "33333333-3333-4333-8333-333333333333";

function credentials(role = "owner"): ProjectedCloudsyncCredentials {
  return {
    encryptionVersion: 2,
    encryptionKeyId: "abcdefghijklmnopqrstuv",
    databaseId: "database-id",
    token: "token",
    expiresAt: "2026-08-15T12:00:00Z",
    workspaceId: OWNER_ID,
    accountUserId: OWNER_ID,
    personalWorkspaceId: OWNER_ID,
    workspaces: [
      {
        id: OWNER_ID,
        ownerUserId: OWNER_ID,
        kind: "personal",
        name: "Personal",
        membershipId: OWNER_ID,
        role: "owner",
        membershipCreatedAt: "2026-08-15T09:00:00Z",
        membershipUpdatedAt: "2026-08-15T09:00:00Z",
        createdAt: "2026-08-15T09:00:00Z",
        updatedAt: "2026-08-15T09:00:00Z",
      },
      {
        id: WORKSPACE_ID,
        ownerUserId: OWNER_ID,
        kind: "shared",
        name: "Team",
        membershipId: "44444444-4444-4444-8444-444444444444",
        role,
        membershipCreatedAt: "2026-08-15T09:00:00Z",
        membershipUpdatedAt: "2026-08-15T09:00:00Z",
        createdAt: "2026-08-15T09:00:00Z",
        updatedAt: "2026-08-15T09:00:00Z",
      },
    ],
    workspaceKeyGrants: [],
  };
}

function recipients(memberPublicKey: string | null = "B".repeat(43)): Array<{
  userId: string;
  userEmail: string;
  role: string;
  publicKey: string | null;
  grantedKeyIds: string[];
}> {
  return [
    {
      userId: OWNER_ID,
      userEmail: "owner@example.com",
      role: "owner",
      publicKey: "A".repeat(43),
      grantedKeyIds: [],
    },
    {
      userId: MEMBER_ID,
      userEmail: "member@example.com",
      role: "member",
      publicKey: memberPublicKey,
      grantedKeyIds: [],
    },
  ];
}

beforeEach(() => {
  vi.clearAllMocks();
});

test("mints and publishes the first shared workspace key", async () => {
  const sealed = {
    keyId: "AAAAAAAAAAAAAAAAAAAAAA",
    grants: [
      {
        userId: OWNER_ID,
        ephemeralPublicKey: "C".repeat(43),
        nonce: "D".repeat(32),
        ciphertext: "E".repeat(64),
      },
      {
        userId: MEMBER_ID,
        ephemeralPublicKey: "F".repeat(43),
        nonce: "G".repeat(32),
        ciphertext: "H".repeat(64),
      },
    ],
  };
  vi.mocked(sealWorkspaceE2eeKeyForRecipients).mockResolvedValue(sealed);
  const fetchMock = vi
    .fn()
    .mockResolvedValueOnce(Response.json(recipients()))
    .mockResolvedValueOnce(
      Response.json({ keyId: sealed.keyId, grantedMemberCount: 2 }),
    );
  vi.stubGlobal("fetch", fetchMock);

  await expect(
    provisionMissingWorkspaceKeys(
      credentials(),
      "access-token",
      OWNER_ID,
      new AbortController().signal,
    ),
  ).resolves.toBe("provisioned");

  expect(sealWorkspaceE2eeKeyForRecipients).toHaveBeenCalledWith(
    OWNER_ID,
    WORKSPACE_ID,
    [
      { userId: OWNER_ID, publicKey: "A".repeat(43) },
      { userId: MEMBER_ID, publicKey: "B".repeat(43) },
    ],
    true,
    null,
  );
  expect(fetchMock).toHaveBeenNthCalledWith(
    2,
    new URL(`https://api.test/sync/e2ee/workspaces/${WORKSPACE_ID}/key`),
    expect.objectContaining({
      method: "PUT",
      body: JSON.stringify(sealed),
    }),
  );
});

test("waits when the caller cannot provision or a member identity is missing", async () => {
  const fetchMock = vi.fn(() =>
    Promise.resolve(Response.json(recipients(null))),
  );
  vi.stubGlobal("fetch", fetchMock);

  await expect(
    provisionMissingWorkspaceKeys(
      credentials("member"),
      "access-token",
      OWNER_ID,
      new AbortController().signal,
    ),
  ).resolves.toBe("waiting");
  expect(fetchMock).not.toHaveBeenCalled();

  await expect(
    provisionMissingWorkspaceKeys(
      credentials(),
      "access-token",
      OWNER_ID,
      new AbortController().signal,
    ),
  ).resolves.toBe("waiting");
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(sealWorkspaceE2eeKeyForRecipients).not.toHaveBeenCalled();
});

test("does nothing when every shared workspace has an active grant", async () => {
  const value = credentials();
  value.workspaceKeyGrants = [
    {
      workspaceId: WORKSPACE_ID,
      keyId: "AAAAAAAAAAAAAAAAAAAAAA",
      ephemeralPublicKey: "A".repeat(43),
      nonce: "B".repeat(32),
      ciphertext: "C".repeat(64),
      isActive: true,
    },
  ];
  const fetchMock = vi.fn(() =>
    Promise.resolve(
      Response.json(
        recipients().map((recipient) => ({
          ...recipient,
          grantedKeyIds: ["AAAAAAAAAAAAAAAAAAAAAA"],
        })),
      ),
    ),
  );
  vi.stubGlobal("fetch", fetchMock);

  await expect(
    provisionMissingWorkspaceKeys(
      value,
      "access-token",
      OWNER_ID,
      new AbortController().signal,
    ),
  ).resolves.toBe("ready");
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(sealWorkspaceE2eeKeyForRecipients).not.toHaveBeenCalled();
});

test("re-wraps the active key for a newly joined member", async () => {
  const value = credentials();
  const sourceGrant = {
    workspaceId: WORKSPACE_ID,
    keyId: "AAAAAAAAAAAAAAAAAAAAAA",
    ephemeralPublicKey: "A".repeat(43),
    nonce: "B".repeat(32),
    ciphertext: "C".repeat(64),
    isActive: true,
  };
  value.workspaceKeyGrants = [sourceGrant];
  const sealed = {
    keyId: sourceGrant.keyId,
    grants: [
      {
        userId: OWNER_ID,
        ephemeralPublicKey: "D".repeat(43),
        nonce: "E".repeat(32),
        ciphertext: "F".repeat(64),
      },
      {
        userId: MEMBER_ID,
        ephemeralPublicKey: "G".repeat(43),
        nonce: "H".repeat(32),
        ciphertext: "I".repeat(64),
      },
    ],
  };
  vi.mocked(sealWorkspaceE2eeKeyForRecipients).mockResolvedValue(sealed);
  const recipientRows = recipients();
  recipientRows[0]!.grantedKeyIds = [sourceGrant.keyId];
  const fetchMock = vi
    .fn()
    .mockResolvedValueOnce(Response.json(recipientRows))
    .mockResolvedValueOnce(
      Response.json({ keyId: sealed.keyId, grantedMemberCount: 2 }),
    );
  vi.stubGlobal("fetch", fetchMock);

  await expect(
    provisionMissingWorkspaceKeys(
      value,
      "access-token",
      OWNER_ID,
      new AbortController().signal,
    ),
  ).resolves.toBe("provisioned");

  expect(sealWorkspaceE2eeKeyForRecipients).toHaveBeenCalledWith(
    OWNER_ID,
    WORKSPACE_ID,
    [
      { userId: OWNER_ID, publicKey: "A".repeat(43) },
      { userId: MEMBER_ID, publicKey: "B".repeat(43) },
    ],
    false,
    sourceGrant,
  );
});

test("mints a new generation after membership revocation retires the old key", async () => {
  const value = credentials();
  value.workspaceKeyGrants = [
    {
      workspaceId: WORKSPACE_ID,
      keyId: "AAAAAAAAAAAAAAAAAAAAAA",
      ephemeralPublicKey: "A".repeat(43),
      nonce: "B".repeat(32),
      ciphertext: "C".repeat(64),
      isActive: false,
    },
  ];
  const sealed = {
    keyId: "BBBBBBBBBBBBBBBBBBBBBB",
    grants: [
      {
        userId: OWNER_ID,
        ephemeralPublicKey: "D".repeat(43),
        nonce: "E".repeat(32),
        ciphertext: "F".repeat(64),
      },
    ],
  };
  vi.mocked(sealWorkspaceE2eeKeyForRecipients).mockResolvedValue(sealed);
  const fetchMock = vi
    .fn()
    .mockResolvedValueOnce(Response.json([recipients()[0]]))
    .mockResolvedValueOnce(
      Response.json({ keyId: sealed.keyId, grantedMemberCount: 1 }),
    );
  vi.stubGlobal("fetch", fetchMock);

  await expect(
    provisionMissingWorkspaceKeys(
      value,
      "access-token",
      OWNER_ID,
      new AbortController().signal,
    ),
  ).resolves.toBe("provisioned");

  expect(sealWorkspaceE2eeKeyForRecipients).toHaveBeenCalledWith(
    OWNER_ID,
    WORKSPACE_ID,
    [{ userId: OWNER_ID, publicKey: "A".repeat(43) }],
    true,
    null,
  );
});
