import { queryOptions } from "@tanstack/react-query";

import {
  fathomImportMeetings,
  googleMeetImportMeetings,
  listConnections,
  microsoftTeamsImportMeetings,
  notionImportMeetings,
  webexImportMeetings,
  zoomImportMeetings,
  type ConnectionItem,
} from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";
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

import { env } from "~/env";
import { openIntegrationUrl } from "~/shared/integration";

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

const NANGO_CONNECTION_POLL_MS = 2_000;
const NANGO_CONNECTION_TIMEOUT_MS = 5 * 60 * 1_000;

export function isDirectMeetingImport(
  provider: Pick<MeetingImportProvider, "directImport">,
) {
  return Boolean(provider.directImport);
}

export function isNangoMeetingImport(
  provider: Pick<MeetingImportProvider, "directImport">,
) {
  return provider.directImport === "nango-oauth";
}

export function isLocalConnectedImport(
  provider: Pick<MeetingImportProvider, "directImport">,
) {
  return (
    provider.directImport === "mcp-oauth" || provider.directImport === "cli"
  );
}

export function nangoConnectionIsReady(connection: ConnectionItem | undefined) {
  return Boolean(connection) && connection?.status !== "reconnect_required";
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

export function nangoImportSyncQueryOptions(
  provider: Pick<MeetingImportProvider, "id" | "name" | "nangoIntegrationId">,
  connectionId: string | undefined,
  headers: Record<string, string> | null,
  enabled: boolean,
) {
  return queryOptions({
    queryKey: [...connectedImportSyncQueryKey(provider.id), connectionId],
    queryFn: () => syncNangoMeetings(provider, connectionId!, headers!),
    enabled: enabled && Boolean(connectionId) && Boolean(headers),
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
  signal?: AbortSignal,
) {
  throwIfConnectionCancelled(signal);
  const authorization = await importerCommands.beginConnectedImport(
    provider.id,
  );
  if (authorization.status === "error") throw new Error(authorization.error);
  await cancelConnectionIfRequested(provider.id, signal);

  if (authorization.data.authorizationUrl) {
    const opened = await openerCommands.openUrl(
      authorization.data.authorizationUrl,
      null,
    );
    if (opened.status === "error") throw new Error(opened.error);
    await cancelConnectionIfRequested(provider.id, signal);
  }

  const credentials = await waitForConnectionCompletion(provider.id, signal);
  if (credentials.status === "error") throw new Error(credentials.error);
  throwIfConnectionCancelled(signal);

  await writeConnectedImportCredentials(provider.id, credentials.data);
  return credentials.data;
}

export async function connectNangoImport(
  provider: Pick<MeetingImportProvider, "id" | "name" | "nangoIntegrationId">,
  headers: Record<string, string>,
  signal?: AbortSignal,
) {
  const integrationId = provider.nangoIntegrationId;
  if (!integrationId) {
    throw new Error(`${provider.name} connection is not available`);
  }
  throwIfConnectionCancelled(signal);
  await openIntegrationUrl(
    integrationId,
    undefined,
    "connect",
    "imports",
    headers,
  );
  await cancelNangoConnectionIfRequested(signal);
  return waitForNangoConnection(provider.name, integrationId, headers, signal);
}

export async function disconnectNangoImport(
  nangoIntegrationId: string,
  connectionId: string,
) {
  await openIntegrationUrl(
    nangoIntegrationId,
    connectionId,
    "disconnect",
    "imports",
    null,
    false,
  );
}

export async function cancelConnectedImport(providerId: string) {
  const result = await importerCommands.cancelConnectedImport(providerId);
  if (result.status === "error") throw new Error(result.error);
  return result.data;
}

async function cancelConnectionIfRequested(
  providerId: string,
  signal?: AbortSignal,
) {
  if (!signal?.aborted) return;
  await cancelConnectedImport(providerId);
  throwIfConnectionCancelled(signal);
}

async function waitForConnectionCompletion(
  providerId: string,
  signal?: AbortSignal,
) {
  const completion = importerCommands.completeConnectedImport(providerId);
  if (!signal) return completion;
  throwIfConnectionCancelled(signal);

  let cancel!: () => void;
  const cancelled = new Promise<never>((_, reject) => {
    cancel = () => reject(connectionCancellationError(signal));
    signal.addEventListener("abort", cancel, { once: true });
    if (signal.aborted) cancel();
  });

  try {
    return await Promise.race([completion, cancelled]);
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}

function throwIfConnectionCancelled(signal?: AbortSignal) {
  if (signal?.aborted) throw connectionCancellationError(signal);
}

function connectionCancellationError(signal: AbortSignal) {
  return signal.reason instanceof Error
    ? signal.reason
    : new Error("Meeting import connection cancelled");
}

export async function disconnectConnectedImport(providerId: string) {
  const result = await store2Commands.deleteSecret(
    CONNECTED_IMPORT_SECRET_SCOPE,
    connectedImportSecretKey(providerId),
  );
  if (result.status === "error") throw new Error(result.error);
}

async function waitForNangoConnection(
  providerName: string,
  integrationId: string,
  headers: Record<string, string>,
  signal?: AbortSignal,
) {
  const client = createClient({ baseUrl: env.VITE_API_URL, headers });
  const deadline = Date.now() + NANGO_CONNECTION_TIMEOUT_MS;

  while (true) {
    throwIfConnectionCancelled(signal);
    const { data, error } = await listConnections({ client });
    if (error) throw new Error("Failed to load integrations");
    const connection = data?.connections.find(
      (item) =>
        item.integration_id === integrationId && nangoConnectionIsReady(item),
    );
    if (connection) return connection;
    if (Date.now() >= deadline) {
      throw new Error(`${providerName} sign-in timed out. Try again.`);
    }
    await sleep(NANGO_CONNECTION_POLL_MS, signal);
  }
}

async function cancelNangoConnectionIfRequested(signal?: AbortSignal) {
  throwIfConnectionCancelled(signal);
}

function sleep(ms: number, signal?: AbortSignal) {
  return new Promise<void>((resolve, reject) => {
    if (!signal) {
      setTimeout(resolve, ms);
      return;
    }
    if (signal.aborted) {
      reject(connectionCancellationError(signal));
      return;
    }

    const onAbort = () => {
      clearTimeout(timer);
      reject(connectionCancellationError(signal));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

async function syncNangoMeetings(
  provider: Pick<MeetingImportProvider, "id" | "name">,
  connectionId: string,
  headers: Record<string, string>,
): Promise<ConnectedImportSyncSummary> {
  const knownMeetingIds = await getImportedMeetingIds(provider.id);
  const client = createClient({ baseUrl: env.VITE_API_URL, headers });
  const body = {
    connection_id: connectionId,
    known_meeting_ids: knownMeetingIds,
  };
  const { data, error } = await nangoImportMeetings(provider.id, {
    client,
    body,
  });
  if (error || !data) {
    throw new Error(`Reconnect ${provider.name} to keep importing`);
  }

  const result = await importConnectedMeetings(provider.id, data.files);
  return { result, warnings: data.warnings };
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
  if (providerId === "granola") return "granola-mcp";
  if (providerId === "plaud") return "plaud-cli";
  return `${providerId}-mcp`;
}

function nangoImportMeetings(
  providerId: string,
  options: {
    client: ReturnType<typeof createClient>;
    body: { connection_id: string; known_meeting_ids: string[] };
  },
) {
  switch (providerId) {
    case "fathom":
      return fathomImportMeetings(options);
    case "google-meet":
      return googleMeetImportMeetings(options);
    case "microsoft-teams":
      return microsoftTeamsImportMeetings(options);
    case "notion":
      return notionImportMeetings(options);
    case "webex":
      return webexImportMeetings(options);
    case "zoom":
      return zoomImportMeetings(options);
    default:
      throw new Error(`${providerId} import is not available`);
  }
}
