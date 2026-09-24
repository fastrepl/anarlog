import { queryOptions } from "@tanstack/react-query";

import {
  commands as importerCommands,
  type ConnectedImportCredentials,
  type CrmClientInput,
  type CrmContact,
  type CrmContactQuery,
  type CrmProviderInfo,
} from "@anlg/plugin-importer";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as store2Commands } from "@anlg/plugin-store2";

export type { CrmClientInput, CrmContact, CrmContactQuery, CrmProviderInfo };

const CRM_SECRET_SCOPE = "crm-connections";

export function crmProvidersQueryOptions() {
  return queryOptions({
    queryKey: ["crm", "providers"] as const,
    queryFn: () => importerCommands.listCrmProviders(),
    staleTime: Infinity,
  });
}

export function crmCredentialsQueryKey(providerId: string) {
  return ["crm", providerId, "credentials"] as const;
}

export function crmCredentialsQueryOptions(providerId: string) {
  return queryOptions({
    queryKey: crmCredentialsQueryKey(providerId),
    queryFn: () => readCrmCredentials(providerId),
    staleTime: Infinity,
  });
}

export async function connectCrm(
  provider: Pick<CrmProviderInfo, "id" | "name">,
  client: CrmClientInput | null,
  signal?: AbortSignal,
) {
  throwIfCancelled(signal);
  const authorization = await importerCommands.beginCrmConnection(
    provider.id,
    client,
  );
  if (authorization.status === "error") throw new Error(authorization.error);
  await cancelIfRequested(provider.id, signal);

  const opened = await openerCommands.openUrl(
    authorization.data.authorizationUrl,
    null,
  );
  if (opened.status === "error") throw new Error(opened.error);
  await cancelIfRequested(provider.id, signal);

  const credentials = await waitForCompletion(provider.id, signal);
  if (credentials.status === "error") throw new Error(credentials.error);
  throwIfCancelled(signal);

  await writeCrmCredentials(provider.id, credentials.data);
  return credentials.data;
}

export async function cancelCrmConnection(providerId: string) {
  const result = await importerCommands.cancelCrmConnection(providerId);
  if (result.status === "error") throw new Error(result.error);
  return result.data;
}

export async function disconnectCrm(providerId: string) {
  const result = await store2Commands.deleteSecret(
    CRM_SECRET_SCOPE,
    crmSecretKey(providerId),
  );
  if (result.status === "error") throw new Error(result.error);
}

export async function verifyCrmConnection(
  provider: Pick<CrmProviderInfo, "id" | "name">,
) {
  const credentials = await readCrmCredentials(provider.id);
  if (!credentials) throw new Error(`Connect ${provider.name} first`);

  const result = await importerCommands.verifyCrmConnection(
    provider.id,
    credentials,
  );
  if (result.status === "error") throw new Error(result.error);
  await writeCrmCredentials(provider.id, result.data);
  return result.data;
}

export async function lookupCrmContacts(
  provider: Pick<CrmProviderInfo, "id" | "name">,
  query: CrmContactQuery,
): Promise<CrmContact[]> {
  const credentials = await readCrmCredentials(provider.id);
  if (!credentials) throw new Error(`Connect ${provider.name} first`);

  const result = await importerCommands.lookupCrmContacts(
    provider.id,
    credentials,
    query,
  );
  if (result.status === "error") throw new Error(result.error);
  await writeCrmCredentials(provider.id, result.data.credentials);
  return result.data.contacts;
}

async function cancelIfRequested(providerId: string, signal?: AbortSignal) {
  if (!signal?.aborted) return;
  await cancelCrmConnection(providerId);
  throwIfCancelled(signal);
}

async function waitForCompletion(providerId: string, signal?: AbortSignal) {
  const completion = importerCommands.completeCrmConnection(providerId);
  if (!signal) return completion;
  throwIfCancelled(signal);

  let cancel!: () => void;
  const cancelled = new Promise<never>((_, reject) => {
    cancel = () => reject(cancellationError(signal));
    signal.addEventListener("abort", cancel, { once: true });
    if (signal.aborted) cancel();
  });

  try {
    return await Promise.race([completion, cancelled]);
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}

function throwIfCancelled(signal?: AbortSignal) {
  if (signal?.aborted) throw cancellationError(signal);
}

function cancellationError(signal: AbortSignal) {
  return signal.reason instanceof Error
    ? signal.reason
    : new Error("CRM connection cancelled");
}

export async function readCrmCredentials(
  providerId: string,
): Promise<ConnectedImportCredentials | null> {
  const result = await store2Commands.getSecret(
    CRM_SECRET_SCOPE,
    crmSecretKey(providerId),
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

async function writeCrmCredentials(
  providerId: string,
  credentials: ConnectedImportCredentials,
) {
  const result = await store2Commands.setSecret(
    CRM_SECRET_SCOPE,
    crmSecretKey(providerId),
    JSON.stringify(credentials),
  );
  if (result.status === "error") throw new Error(result.error);
}

function crmSecretKey(providerId: string) {
  return `${providerId}-mcp`;
}
