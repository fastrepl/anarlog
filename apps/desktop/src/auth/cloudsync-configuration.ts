import { configureCloudsyncToken } from "@anlg/plugin-db";

import {
  hasWorkspaceProjection,
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
      )
    : configureCloudsyncToken(
        credentials.databaseId,
        credentials.token,
        accountUserId,
        witness,
      );
}
