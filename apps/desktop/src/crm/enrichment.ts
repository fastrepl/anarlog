import {
  type CrmContact,
  type CrmProviderInfo,
  lookupCrmContacts,
  readCrmCredentials,
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

export async function connectedCrmProviders(
  providers: CrmProviderInfo[],
): Promise<CrmProviderInfo[]> {
  const connected = await Promise.all(
    providers.map(async (provider) =>
      (await readCrmCredentials(provider.id)) ? provider : null,
    ),
  );
  return connected.filter((provider) => provider !== null);
}

export async function enrichHumanFromCrm({
  human,
  ownerUserId,
  providers,
}: {
  human: Pick<HumanRecord, "id"> & EnrichableHuman;
  ownerUserId: string;
  providers: CrmProviderInfo[];
}): Promise<CrmEnrichmentResult> {
  const connected = await connectedCrmProviders(providers);
  if (connected.length === 0) return { status: "not_connected" };

  const email = clean(human.email) ?? null;
  const name = clean(human.name) ?? null;
  if (!email && !name) {
    return { status: "no_match", providers: connected.map((p) => p.name) };
  }

  let firstError: unknown = null;
  for (const provider of connected) {
    let contacts: CrmContact[];
    try {
      contacts = await lookupCrmContacts(provider, { email, name });
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
  return { status: "no_match", providers: connected.map((p) => p.name) };
}

function clean(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}
