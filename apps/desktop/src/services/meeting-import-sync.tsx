import { useQueries } from "@tanstack/react-query";

import { useAuth } from "~/auth";
import { useConnections } from "~/auth/useConnections";
import {
  connectedImportCredentialsQueryOptions,
  connectedImportSyncQueryOptions,
  isLocalConnectedImport,
  isNangoMeetingImport,
  nangoConnectionIsReady,
  nangoImportSyncQueryOptions,
} from "~/imports/connected-import";
import { MEETING_IMPORT_PROVIDERS } from "~/imports/providers";

const LOCAL_CONNECTED_PROVIDERS = MEETING_IMPORT_PROVIDERS.filter(
  isLocalConnectedImport,
);
const NANGO_PROVIDERS = MEETING_IMPORT_PROVIDERS.filter(isNangoMeetingImport);

export function MeetingImportSync() {
  const auth = useAuth();
  const signedIn = Boolean(auth.session);
  const headers = auth.getHeaders();
  const connectionsQuery = useConnections(signedIn);
  const credentialQueries = useQueries({
    queries: LOCAL_CONNECTED_PROVIDERS.map((provider) =>
      connectedImportCredentialsQueryOptions(provider.id),
    ),
  });
  useQueries({
    queries: LOCAL_CONNECTED_PROVIDERS.map((provider, index) =>
      connectedImportSyncQueryOptions(
        provider,
        signedIn && Boolean(credentialQueries[index]?.data),
      ),
    ),
  });
  useQueries({
    queries: NANGO_PROVIDERS.map((provider) => {
      const connection = connectionsQuery.data?.find(
        (item) => item.integration_id === provider.nangoIntegrationId,
      );
      return nangoImportSyncQueryOptions(
        provider,
        connection?.connection_id,
        headers,
        signedIn && nangoConnectionIsReady(connection),
      );
    }),
  });

  return null;
}
