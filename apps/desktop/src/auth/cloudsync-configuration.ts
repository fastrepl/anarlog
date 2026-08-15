import { configureCloudsyncToken, configureE2eeReplica } from "@anlg/plugin-db";

import {
  hasWorkspaceProjection,
  isReplicaCredentials,
  type CloudsyncCredentials,
} from "./cloudsync-credentials";

import { env } from "~/env";

export function configureCloudsyncCredentials(
  credentials: CloudsyncCredentials,
  accessToken: string,
  accountUserId: string,
) {
  const witnessWorkspaceId = hasWorkspaceProjection(credentials)
    ? credentials.personalWorkspaceId
    : credentials.workspaceId;
  const witness = {
    endpoint: new URL(
      `/sync/e2ee/witness/${witnessWorkspaceId}`,
      env.VITE_API_URL,
    ).toString(),
    accessToken,
  };

  if (isReplicaCredentials(credentials)) {
    return configureE2eeReplica(credentials.workspaceId, witness);
  }

  return hasWorkspaceProjection(credentials)
    ? configureCloudsyncToken(
        credentials.databaseId,
        credentials.token,
        accountUserId,
        witness,
        {
          accountUserId: credentials.accountUserId,
          personalWorkspaceId: credentials.personalWorkspaceId,
          workspaces: credentials.workspaces,
        },
        credentials.workspaceKeyGrants ?? [],
      )
    : configureCloudsyncToken(
        credentials.databaseId,
        credentials.token,
        accountUserId,
        witness,
      );
}
