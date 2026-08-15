import { sealWorkspaceE2eeKeyForRecipients } from "@anlg/plugin-db";

import type { ProjectedCloudsyncCredentials } from "./cloudsync-credentials";

import { env } from "~/env";

export async function provisionMissingWorkspaceKeys(
  credentials: ProjectedCloudsyncCredentials,
  accessToken: string,
  accountUserId: string,
  signal: AbortSignal,
) {
  const activeGrants = new Map(
    (credentials.workspaceKeyGrants ?? [])
      .filter((grant) => grant.isActive)
      .map((grant) => [grant.workspaceId, grant]),
  );
  const sharedWorkspaces = credentials.workspaces.filter(
    (workspace) => workspace.kind === "shared",
  );
  if (sharedWorkspaces.length === 0) {
    return "ready" as const;
  }

  let provisioned = false;
  let waiting = false;
  for (const workspace of sharedWorkspaces) {
    const activeGrant = activeGrants.get(workspace.id);
    if (workspace.role !== "owner" && workspace.role !== "admin") {
      waiting ||= activeGrant === undefined;
      continue;
    }
    const workspacePath = encodeURIComponent(workspace.id);
    const recipientsResponse = await fetch(
      new URL(
        `/sync/e2ee/workspaces/${workspacePath}/recipients`,
        env.VITE_API_URL,
      ),
      {
        headers: { Authorization: `Bearer ${accessToken}` },
        signal,
      },
    );
    if (!recipientsResponse.ok) {
      throw new Error("workspace E2EE recipients are unavailable");
    }
    const recipientsValue: unknown = await recipientsResponse.json();
    if (!Array.isArray(recipientsValue) || recipientsValue.length === 0) {
      throw new Error("workspace E2EE recipients are invalid");
    }
    const recipientIds = new Set<string>();
    const recipients: Array<{
      userId: string;
      publicKey: string;
      grantedKeyIds: string[];
    }> = [];
    let waitingForIdentity = false;
    for (const value of recipientsValue) {
      if (!value || typeof value !== "object") {
        throw new Error("workspace E2EE recipients are invalid");
      }
      const recipient = value as Record<string, unknown>;
      if (
        typeof recipient.userId !== "string" ||
        recipient.userId.length === 0 ||
        recipientIds.has(recipient.userId) ||
        !Array.isArray(recipient.grantedKeyIds) ||
        recipient.grantedKeyIds.some(
          (keyId) =>
            typeof keyId !== "string" || !/^[A-Za-z0-9_-]{22}$/.test(keyId),
        ) ||
        (recipient.publicKey !== null &&
          (typeof recipient.publicKey !== "string" ||
            !/^[A-Za-z0-9_-]{43}$/.test(recipient.publicKey)))
      ) {
        throw new Error("workspace E2EE recipients are invalid");
      }
      recipientIds.add(recipient.userId);
      if (recipient.publicKey === null) {
        waitingForIdentity = true;
      } else {
        recipients.push({
          userId: recipient.userId,
          publicKey: recipient.publicKey,
          grantedKeyIds: recipient.grantedKeyIds,
        });
      }
    }
    if (!recipientIds.has(accountUserId)) {
      throw new Error("workspace E2EE issuer is missing");
    }
    if (
      activeGrant &&
      recipientsValue.every(
        (recipient) =>
          recipient &&
          typeof recipient === "object" &&
          Array.isArray(recipient.grantedKeyIds) &&
          recipient.grantedKeyIds.includes(activeGrant.keyId),
      )
    ) {
      continue;
    }
    if (waitingForIdentity) {
      waiting = true;
      continue;
    }

    const sealed = await sealWorkspaceE2eeKeyForRecipients(
      accountUserId,
      workspace.id,
      recipients.map(({ userId, publicKey }) => ({ userId, publicKey })),
      activeGrant === undefined,
      activeGrant ?? null,
    );
    const publicationResponse = await fetch(
      new URL(`/sync/e2ee/workspaces/${workspacePath}/key`, env.VITE_API_URL),
      {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(sealed),
        signal,
      },
    );
    if (!publicationResponse.ok) {
      throw new Error("workspace E2EE key publication failed");
    }
    const publication: unknown = await publicationResponse.json();
    if (
      !publication ||
      typeof publication !== "object" ||
      !("keyId" in publication) ||
      publication.keyId !== sealed.keyId
    ) {
      throw new Error("workspace E2EE key publication response is invalid");
    }
    provisioned = true;
  }

  if (provisioned) {
    return "provisioned" as const;
  }
  return waiting ? ("waiting" as const) : ("ready" as const);
}
