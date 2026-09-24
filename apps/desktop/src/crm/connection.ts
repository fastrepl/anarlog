import { queryOptions } from "@tanstack/react-query";

import { crmSearchContacts, type ConnectionItem } from "@anlg/api-client";
import type { CrmContact } from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";

import { env } from "~/env";
import {
  nangoConnectionIsReady,
  waitForNangoConnection,
} from "~/imports/connected-import";
import {
  integrationSetupError,
  openIntegrationUrl,
} from "~/shared/integration";

export type { CrmContact };

export type CrmProviderInfo = {
  id: string;
  name: string;
  nangoIntegrationId: string;
};

export const CRM_PROVIDERS: CrmProviderInfo[] = [
  {
    id: "hubspot",
    name: "HubSpot",
    nangoIntegrationId: "hubspot",
  },
  {
    id: "attio",
    name: "Attio",
    nangoIntegrationId: "attio",
  },
  {
    id: "salesforce",
    name: "Salesforce",
    nangoIntegrationId: "salesforce",
  },
  {
    id: "pipedrive",
    name: "Pipedrive",
    nangoIntegrationId: "pipedrive",
  },
  {
    id: "close",
    name: "Close",
    nangoIntegrationId: "close",
  },
];

export type CrmContactQuery = {
  email?: string;
  name?: string;
};

export function crmProvidersQueryOptions() {
  return queryOptions({
    queryKey: ["crm", "providers"],
    queryFn: () => CRM_PROVIDERS,
    staleTime: Infinity,
  });
}

export function findCrmConnection(
  provider: Pick<CrmProviderInfo, "nangoIntegrationId">,
  connections: ConnectionItem[] | undefined,
) {
  const matches = connections?.filter(
    (item) => item.integration_id === provider.nangoIntegrationId,
  );
  return matches?.find(nangoConnectionIsReady) ?? matches?.[0];
}

export async function connectCrm(
  provider: Pick<CrmProviderInfo, "name" | "nangoIntegrationId">,
  headers: Record<string, string>,
  signal?: AbortSignal,
  connectionId?: string,
): Promise<ConnectionItem> {
  const opened = await openIntegrationUrl(
    provider.nangoIntegrationId,
    connectionId,
    connectionId ? "reconnect" : "connect",
    "crm",
    headers,
    false,
  );
  if (!opened) {
    throw integrationSetupError();
  }
  return waitForNangoConnection(
    provider.name,
    provider.nangoIntegrationId,
    headers,
    signal,
  );
}

export async function disconnectCrm(
  provider: Pick<CrmProviderInfo, "nangoIntegrationId">,
  connectionId: string,
) {
  await openIntegrationUrl(
    provider.nangoIntegrationId,
    connectionId,
    "disconnect",
    "crm",
    null,
    false,
  );
}

export async function verifyCrmConnection(
  provider: Pick<CrmProviderInfo, "id" | "name">,
  connectionId: string,
  headers: Record<string, string>,
) {
  await lookupCrmContacts(provider, connectionId, { name: "test" }, headers);
}

export async function lookupCrmContacts(
  provider: Pick<CrmProviderInfo, "id" | "name">,
  connectionId: string,
  query: CrmContactQuery,
  headers: Record<string, string>,
): Promise<CrmContact[]> {
  const client = createClient({ baseUrl: env.VITE_API_URL, headers });
  const { data, error } = await crmSearchContacts({
    client,
    body: {
      provider: provider.id,
      connection_id: connectionId,
      email: query.email,
      name: query.name,
    },
  });
  if (error || !data) {
    throw new Error(`Failed to look up contacts in ${provider.name}`);
  }
  return data.contacts;
}

export { nangoConnectionIsReady };
