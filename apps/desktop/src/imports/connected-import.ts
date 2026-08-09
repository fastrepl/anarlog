import { queryOptions } from "@tanstack/react-query";

import {
  commands as importerCommands,
  type ConnectedImportCredentials,
} from "@anlg/plugin-importer";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as store2Commands } from "@anlg/plugin-store2";

import type { MeetingImportProvider } from "./providers";
import {
  getImportedMeetingIds,
  importConnectedMeetings,
  type MeetingImportResult,
} from "./queries";

const CONNECTED_IMPORT_SECRET_SCOPE = "meeting-imports";
const CONNECTED_IMPORT_SYNC_INTERVAL_MS = 5 * 60 * 1_000;

export type ConnectedImportSyncSummary = {
  result: MeetingImportResult;
  warnings: string[];
};

export function connectedImportCredentialsQueryKey(providerId: string) {
  return ["meeting-import", providerId, "credentials"] as const;
}

export function connectedImportSyncQueryKey(providerId: string) {
  return ["meeting-import", providerId, "sync"] as const;
}

export function connectedImportCredentialsQueryOptions(providerId: string) {
  return queryOptions({
    queryKey: connectedImportCredentialsQueryKey(providerId),
    queryFn: () => readConnectedImportCredentials(providerId),
    staleTime: Infinity,
  });
}

export function connectedImportSyncQueryOptions(
  provider: Pick<MeetingImportProvider, "id" | "name">,
  enabled: boolean,
) {
  return queryOptions({
    queryKey: connectedImportSyncQueryKey(provider.id),
    queryFn: () => syncConnectedMeetings(provider),
    enabled,
    retry: false,
    staleTime: CONNECTED_IMPORT_SYNC_INTERVAL_MS,
    refetchInterval: CONNECTED_IMPORT_SYNC_INTERVAL_MS,
    refetchIntervalInBackground: true,
    refetchOnMount: true,
    refetchOnWindowFocus: true,
  });
}

export async function connectConnectedImport(
  provider: Pick<MeetingImportProvider, "id" | "name">,
) {
  const authorization = await importerCommands.beginConnectedImport(
    provider.id,
  );
  if (authorization.status === "error") throw new Error(authorization.error);

  const opened = await openerCommands.openUrl(
    authorization.data.authorizationUrl,
    null,
  );
  if (opened.status === "error") throw new Error(opened.error);

  const credentials = await importerCommands.completeConnectedImport(
    provider.id,
  );
  if (credentials.status === "error") throw new Error(credentials.error);

  await writeConnectedImportCredentials(provider.id, credentials.data);
  return credentials.data;
}

export async function disconnectConnectedImport(providerId: string) {
  const result = await store2Commands.deleteSecret(
    CONNECTED_IMPORT_SECRET_SCOPE,
    connectedImportSecretKey(providerId),
  );
  if (result.status === "error") throw new Error(result.error);
}

async function syncConnectedMeetings(
  provider: Pick<MeetingImportProvider, "id" | "name">,
): Promise<ConnectedImportSyncSummary> {
  const credentials = await readConnectedImportCredentials(provider.id);
  if (!credentials) {
    throw new Error(`Reconnect ${provider.name} to keep importing`);
  }

  const knownMeetingIds = await getImportedMeetingIds(provider.id);
  const sync = await importerCommands.syncConnectedImport(
    provider.id,
    credentials,
    knownMeetingIds,
  );
  if (sync.status === "error") throw new Error(sync.error);

  await writeConnectedImportCredentials(provider.id, sync.data.credentials);
  const result = await importConnectedMeetings(provider.id, sync.data.files);
  return { result, warnings: sync.data.warnings };
}

async function readConnectedImportCredentials(
  providerId: string,
): Promise<ConnectedImportCredentials | null> {
  const result = await store2Commands.getSecret(
    CONNECTED_IMPORT_SECRET_SCOPE,
    connectedImportSecretKey(providerId),
  );
  if (result.status === "error") throw new Error(result.error);
  if (!result.data) return null;

  try {
    const credentials = JSON.parse(
      result.data,
    ) as Partial<ConnectedImportCredentials>;
    if (
      (credentials.providerId ?? providerId) !== providerId ||
      !credentials.clientId ||
      !credentials.tokenJson
    ) {
      return null;
    }
    return {
      providerId,
      clientId: credentials.clientId,
      clientSecret: credentials.clientSecret ?? null,
      tokenJson: credentials.tokenJson,
      tokenReceivedAt: credentials.tokenReceivedAt ?? null,
    };
  } catch {
    return null;
  }
}

async function writeConnectedImportCredentials(
  providerId: string,
  credentials: ConnectedImportCredentials,
) {
  const result = await store2Commands.setSecret(
    CONNECTED_IMPORT_SECRET_SCOPE,
    connectedImportSecretKey(providerId),
    JSON.stringify(credentials),
  );
  if (result.status === "error") throw new Error(result.error);
}

function connectedImportSecretKey(providerId: string) {
  return providerId === "granola" ? "granola-mcp" : `${providerId}-mcp`;
}
