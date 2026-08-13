import { useQueries } from "@tanstack/react-query";

import { useAuth } from "~/auth";
import {
  connectedImportCredentialsQueryOptions,
  connectedImportSyncQueryOptions,
} from "~/imports/connected-import";
import { MEETING_IMPORT_PROVIDERS } from "~/imports/providers";

const CONNECTED_PROVIDERS = MEETING_IMPORT_PROVIDERS.filter(
  (provider) => provider.directImport === "mcp-oauth",
);

export function MeetingImportSync() {
  const auth = useAuth();
  const signedIn = Boolean(auth.session);
  const credentialQueries = useQueries({
    queries: CONNECTED_PROVIDERS.map((provider) =>
      connectedImportCredentialsQueryOptions(provider.id),
    ),
  });
  useQueries({
    queries: CONNECTED_PROVIDERS.map((provider, index) =>
      connectedImportSyncQueryOptions(
        provider,
        signedIn && Boolean(credentialQueries[index]?.data),
      ),
    ),
  });

  return null;
}
