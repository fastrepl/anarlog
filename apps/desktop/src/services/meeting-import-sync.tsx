import { useQueries } from "@tanstack/react-query";

import {
  connectedImportCredentialsQueryOptions,
  connectedImportSyncQueryOptions,
  isLocalConnectedImport,
} from "~/imports/connected-import";
import { MEETING_IMPORT_PROVIDERS } from "~/imports/providers";

const LOCAL_CONNECTED_PROVIDERS = MEETING_IMPORT_PROVIDERS.filter(
  isLocalConnectedImport,
);

export function MeetingImportSync() {
  const credentialQueries = useQueries({
    queries: LOCAL_CONNECTED_PROVIDERS.map((provider) =>
      connectedImportCredentialsQueryOptions(provider.id),
    ),
  });
  useQueries({
    queries: LOCAL_CONNECTED_PROVIDERS.map((provider, index) =>
      connectedImportSyncQueryOptions(
        provider,
        Boolean(credentialQueries[index]?.data),
      ),
    ),
  });

  return null;
}
