import type { ConnectionItem } from "@anlg/api-client";

import {
  type CrmContact,
  type CrmProviderInfo,
  findCrmConnection,
  lookupCrmContacts,
  nangoConnectionIsReady,
} from "./connection";

import { applyContactEnhancement, type HumanRecord } from "~/contacts/queries";

export type CrmEnrichmentChanges = {
  name?: string;
  email?: string;
  companyName?: string;
  jobTitle?: string;
  phone?: string;
  linkedinUsername?: string;
};

export type CrmEnrichmentField = keyof CrmEnrichmentChanges;

export type CrmEnrichmentResult =
  | { status: "not_connected" }
  | { status: "no_match"; providers: string[] }
  | {
      status: "matched";
      provider: CrmProviderInfo;
      contact: CrmContact;
      changes: CrmEnrichmentChanges;
    };

type EnrichableHuman = Pick<
  HumanRecord,
  | "name"
  | "email"
  | "jobTitle"
  | "phone"
  | "linkedinUsername"
  | "organizationId"
>;

/** Only fills fields that are currently empty; never overwrites local edits. */
export function planCrmEnrichment(
  human: EnrichableHuman,
  contact: CrmContact,
): CrmEnrichmentChanges {
  const changes: CrmEnrichmentChanges = {};
  const name = clean(contact.name);
  if (!clean(human.name) && name) changes.name = name;
  const email = clean(contact.email);
  if (!clean(human.email) && email) changes.email = email;
  const companyName = clean(contact.companyName);
  if (!clean(human.organizationId) && companyName) {
    changes.companyName = companyName;
  }
  const jobTitle = clean(contact.jobTitle);
  if (!clean(human.jobTitle) && jobTitle) changes.jobTitle = jobTitle;
  const phone = clean(contact.phone);
  if (!clean(human.phone) && phone) changes.phone = phone;
  const linkedinUsername = linkedinUsernameFromUrl(contact.linkedinUrl);
  if (!clean(human.linkedinUsername) && linkedinUsername) {
    changes.linkedinUsername = linkedinUsername;
  }
  return changes;
}

export function linkedinUsernameFromUrl(
  url: string | null | undefined,
): string | undefined {
  const value = clean(url);
  if (!value) return undefined;
  const match = /linkedin\.com\/in\/([^/?#]+)/i.exec(value);
  if (match) return decodeURIComponent(match[1]);
  if (!value.includes("/") && !value.includes(".")) return value;
  return undefined;
}

/** Prefers the record that fills the most empty fields; ties keep CRM order. */
export function pickBestCrmContact(
  human: EnrichableHuman,
  contacts: CrmContact[],
): { contact: CrmContact; changes: CrmEnrichmentChanges } | null {
  let best: { contact: CrmContact; changes: CrmEnrichmentChanges } | null =
    null;
  for (const contact of contacts) {
    const changes = planCrmEnrichment(human, contact);
    if (
      !best ||
      Object.keys(changes).length > Object.keys(best.changes).length
    ) {
      best = { contact, changes };
    }
  }
  return best;
}

export function connectedCrmProviders(
  providers: CrmProviderInfo[],
  connections: ConnectionItem[] | undefined,
): { provider: CrmProviderInfo; connectionId: string }[] {
  return providers
    .map((provider) => {
      const connection = findCrmConnection(provider, connections);
      return connection && nangoConnectionIsReady(connection)
        ? { provider, connectionId: connection.connection_id }
        : null;
    })
    .filter((entry) => entry !== null);
}

export async function enrichHumanFromCrm({
  human,
  ownerUserId,
  providers,
  connections,
  headers,
}: {
  human: Pick<HumanRecord, "id"> & EnrichableHuman;
  ownerUserId: string;
  providers: CrmProviderInfo[];
  connections: ConnectionItem[] | undefined;
  headers: Record<string, string>;
}): Promise<CrmEnrichmentResult> {
  const connected = connectedCrmProviders(providers, connections);
  if (connected.length === 0) return { status: "not_connected" };

  const providerNames = connected.map(({ provider }) => provider.name);
  const email = clean(human.email);
  const name = clean(human.name);
  if (!email && !name) {
    return { status: "no_match", providers: providerNames };
  }

  let firstError: unknown = null;
  for (const { provider, connectionId } of connected) {
    let contacts: CrmContact[];
    try {
      contacts = await lookupCrmContacts(
        provider,
        connectionId,
        { email, name },
        headers,
      );
    } catch (error) {
      firstError ??= error;
      continue;
    }
    const best = pickBestCrmContact(human, contacts);
    if (!best) continue;
    if (Object.keys(best.changes).length > 0) {
      await applyContactEnhancement({
        humanId: human.id,
        ownerUserId,
        changes: best.changes,
      });
    }
    return { status: "matched", provider, ...best };
  }

  if (firstError) throw firstError;
  return { status: "no_match", providers: providerNames };
}

function clean(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}
