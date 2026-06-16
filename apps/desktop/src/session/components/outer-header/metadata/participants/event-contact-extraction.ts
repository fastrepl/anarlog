import { generateText, type LanguageModel } from "ai";
import { z } from "zod";

import {
  commands as templateCommands,
  type EventContactCandidate as TemplateEventContactCandidate,
} from "@hypr/plugin-template";
import type {
  EventParticipant,
  HumanStorage,
  MappingSessionParticipantStorage,
  OrganizationStorage,
  SessionEvent,
} from "@hypr/store";

import { DEFAULT_USER_ID, id } from "~/shared/utils";
import type * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

const MAX_EVENT_TEXT_CHARS = 6000;
const MAX_CONTACTS_TO_EXTRACT = 8;

export type EventContactCandidate = {
  humanId?: string;
  name?: string;
  email?: string;
  isCurrentUser?: boolean;
  isOrganizer?: boolean;
};

export type EventContactExtractionContext = {
  title?: string;
  description?: string;
  candidates: EventContactCandidate[];
};

export type ExtractedEventContact = {
  name: string;
  email?: string;
  companyName?: string;
};

export type ExtractEventContactsResult = {
  contacts: ExtractedEventContact[];
  source: "model";
};

export type ApplyExtractedContactsResult = {
  created: number;
  updated: number;
  linked: number;
  skipped: number;
  contacts: ExtractedEventContact[];
};

export type ApplyContactEnhancementResult = ApplyExtractedContactsResult & {
  matched: boolean;
};

const aiExtractionSchema = z.object({
  contacts: z
    .array(
      z.object({
        name: z.string(),
        email: z.union([z.string(), z.null()]).optional(),
        companyName: z.union([z.string(), z.null()]).optional(),
      }),
    )
    .max(MAX_CONTACTS_TO_EXTRACT),
});

export function buildEventContactExtractionContext(
  store: Store,
  sessionId: string,
  sessionEvent: SessionEvent | null,
): EventContactExtractionContext {
  const currentUserId = getCurrentUserId(store);
  const candidates = collectContactCandidates(store, sessionId, {
    sessionEvent,
    currentUserId,
  });

  return {
    title: sessionEvent?.title,
    description: sessionEvent?.description,
    candidates,
  };
}

export async function extractEventContacts({
  model,
  context,
}: {
  model: LanguageModel | null;
  context: EventContactExtractionContext;
}): Promise<ExtractEventContactsResult> {
  if (!model) {
    throw new Error("Language model needed");
  }

  const [system, prompt] = await Promise.all([
    getSystemPrompt(),
    getUserPrompt(context),
  ]);

  const result = await generateText({
    model,
    temperature: 0,
    maxRetries: 2,
    maxOutputTokens: 384,
    system,
    prompt,
  });

  const contacts = normalizeExtractedContacts(
    parseExtractionJson(result.text).contacts,
    context.candidates,
  );

  return { contacts, source: "model" };
}

export function applyExtractedContacts(
  store: Store,
  sessionId: string,
  contacts: ExtractedEventContact[],
  options: {
    userId?: string;
    createdAt?: string;
  } = {},
): ApplyExtractedContactsResult {
  const userId = options.userId || getCurrentUserId(store);
  const createdAt = options.createdAt || new Date().toISOString();
  const normalizedContacts = normalizeExtractedContacts(contacts, []);

  const result: ApplyExtractedContactsResult = {
    created: 0,
    updated: 0,
    linked: 0,
    skipped: 0,
    contacts: [],
  };

  if (normalizedContacts.length === 0) {
    return result;
  }

  store.transaction(() => {
    const humansByEmail = buildHumansByEmailIndex(store);
    const humansByName = buildHumansByNameIndex(store);
    const organizationsByName = buildOrganizationsByNameIndex(store);
    const sessionMappings = buildSessionMappingsByHuman(store, sessionId);
    const sessionHumansByName = buildSessionHumansByNameIndex(
      store,
      sessionMappings,
    );
    const currentUser = store.getRow("humans", userId);

    for (const contact of normalizedContacts) {
      if (isCurrentUserContact(contact, currentUser)) {
        result.skipped += 1;
        continue;
      }

      const emailLower = contact.email?.toLowerCase();
      const nameKey = normalizeName(contact.name);
      let humanId = emailLower
        ? (humansByEmail.get(emailLower) ?? sessionHumansByName.get(nameKey))
        : humansByName.get(nameKey);

      if (humanId === userId) {
        result.skipped += 1;
        continue;
      }

      if (!humanId) {
        const orgId = getOrCreateOrganizationId(
          store,
          organizationsByName,
          userId,
          contact.companyName,
          createdAt,
        );

        humanId = id();
        store.setRow("humans", humanId, {
          user_id: userId,
          created_at: createdAt,
          name: contact.name,
          email: contact.email ?? "",
          phone: "",
          org_id: orgId ?? "",
          job_title: "",
          linkedin_username: "",
          memo: "",
          pinned: false,
        } satisfies HumanStorage);
        result.created += 1;
        result.contacts.push(contact);

        if (emailLower) {
          humansByEmail.set(emailLower, humanId);
        }
        humansByName.set(nameKey, humanId);
      } else {
        const human = store.getRow("humans", humanId);
        const existingName = stringCell(human?.name);
        const existingEmail = stringCell(human?.email);
        const existingOrgId = stringCell(human?.org_id);
        const orgId = existingOrgId
          ? undefined
          : getOrCreateOrganizationId(
              store,
              organizationsByName,
              userId,
              contact.companyName,
              createdAt,
            );
        const shouldUpdateOrg = shouldUpdateHumanOrg(existingOrgId, orgId);
        const shouldUpdateName = shouldUpdateHumanName(
          existingName,
          contact.email,
        );
        const shouldUpdateEmail = shouldUpdateHumanEmail(
          existingEmail,
          contact.email,
        );

        if (shouldUpdateName) {
          store.setCell("humans", humanId, "name", contact.name);
          humansByName.set(nameKey, humanId);
          sessionHumansByName.set(nameKey, humanId);
        }
        if (shouldUpdateEmail) {
          store.setCell("humans", humanId, "email", contact.email ?? "");
          if (emailLower) {
            humansByEmail.set(emailLower, humanId);
          }
        }
        if (shouldUpdateOrg) {
          store.setCell("humans", humanId, "org_id", orgId ?? "");
        }
        if (shouldUpdateName || shouldUpdateEmail || shouldUpdateOrg) {
          result.updated += 1;
          result.contacts.push(contact);
        }
      }

      const existingMapping = sessionMappings.get(humanId);
      if (!existingMapping) {
        store.setRow("mapping_session_participant", id(), {
          user_id: userId,
          session_id: sessionId,
          human_id: humanId,
          source: "manual",
        } satisfies MappingSessionParticipantStorage);
        sessionMappings.set(humanId, { source: "manual" });
        result.linked += 1;
      } else if (existingMapping.source === "excluded") {
        result.skipped += 1;
      }
    }
  });

  return result;
}

export function applyExtractedContactToHuman(
  store: Store,
  sessionId: string,
  humanId: string,
  contacts: ExtractedEventContact[],
  options: {
    userId?: string;
  } = {},
): ApplyContactEnhancementResult {
  const userId = options.userId || getCurrentUserId(store);
  const normalizedContacts = normalizeExtractedContacts(contacts, []);
  const result: ApplyContactEnhancementResult = {
    created: 0,
    updated: 0,
    linked: 0,
    skipped: 0,
    contacts: [],
    matched: false,
  };

  if (normalizedContacts.length === 0) {
    return result;
  }

  store.transaction(() => {
    const sessionMappings = buildSessionMappingsByHuman(store, sessionId);
    const mapping = sessionMappings.get(humanId);
    if (!mapping || mapping.source === "excluded") {
      result.skipped += 1;
      return;
    }

    const human = store.getRow("humans", humanId);
    if (!human) {
      result.skipped += 1;
      return;
    }

    const contact = findContactForHuman(human, normalizedContacts);
    if (!contact) {
      if (humanId === userId) {
        result.matched = true;
        result.contacts.push(normalizedContacts[0]);
        result.skipped += 1;
      }
      return;
    }

    result.matched = true;
    result.contacts.push(contact);

    const currentUser = store.getRow("humans", userId);
    if (humanId === userId || isCurrentUserContact(contact, currentUser)) {
      result.skipped += 1;
      return;
    }

    const existingName = stringCell(human.name);
    const existingEmail = stringCell(human.email);
    const existingOrgId = stringCell(human.org_id);
    const organizationsByName = buildOrganizationsByNameIndex(store);
    const orgId = existingOrgId
      ? undefined
      : getOrCreateOrganizationId(
          store,
          organizationsByName,
          userId,
          contact.companyName,
          new Date().toISOString(),
        );
    const shouldUpdateName = shouldUpdateHumanName(existingName, contact.email);
    const shouldUpdateEmail = shouldUpdateHumanEmail(
      existingEmail,
      contact.email,
    );
    const shouldUpdateOrg = shouldUpdateHumanOrg(existingOrgId, orgId);

    if (shouldUpdateName) {
      store.setCell("humans", humanId, "name", contact.name);
    }
    if (shouldUpdateEmail) {
      store.setCell("humans", humanId, "email", contact.email ?? "");
    }
    if (shouldUpdateOrg) {
      store.setCell("humans", humanId, "org_id", orgId ?? "");
    }
    if (shouldUpdateName || shouldUpdateEmail || shouldUpdateOrg) {
      result.updated += 1;
    }
  });

  return result;
}

function collectContactCandidates(
  store: Store,
  sessionId: string,
  {
    sessionEvent,
    currentUserId,
  }: {
    sessionEvent: SessionEvent | null;
    currentUserId: string;
  },
): EventContactCandidate[] {
  const candidates: EventContactCandidate[] = [];

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mapping = store.getRow("mapping_session_participant", mappingId);
    if (mapping?.session_id !== sessionId || mapping.source === "excluded") {
      return;
    }

    const humanId = stringCell(mapping.human_id);
    if (!humanId) {
      return;
    }

    const human = store.getRow("humans", humanId);
    candidates.push({
      humanId,
      name: stringCell(human?.name),
      email: stringCell(human?.email),
      isCurrentUser: humanId === currentUserId,
    });
  });

  for (const participant of getMatchingEventParticipants(store, sessionEvent)) {
    candidates.push({
      name: participant.name,
      email: participant.email,
      isCurrentUser: participant.is_current_user,
      isOrganizer: participant.is_organizer,
    });
  }

  return dedupeCandidates(candidates);
}

function getMatchingEventParticipants(
  store: Store,
  sessionEvent: SessionEvent | null,
): EventParticipant[] {
  if (!sessionEvent?.tracking_id) {
    return [];
  }

  let participants: EventParticipant[] = [];
  store.forEachRow("events", (eventId, _forEachCell) => {
    if (participants.length > 0) {
      return;
    }

    const event = store.getRow("events", eventId);
    if (
      event?.tracking_id_event !== sessionEvent.tracking_id ||
      event.calendar_id !== sessionEvent.calendar_id
    ) {
      return;
    }

    const parsed = parseParticipantsJson(stringCell(event.participants_json));
    if (parsed) {
      participants = parsed;
    }
  });

  return participants;
}

function parseParticipantsJson(
  value: string | undefined,
): EventParticipant[] | null {
  if (!value) {
    return null;
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function dedupeCandidates(
  candidates: EventContactCandidate[],
): EventContactCandidate[] {
  const byKey = new Map<string, EventContactCandidate>();

  for (const candidate of candidates) {
    const email = normalizeEmail(candidate.email);
    const humanId = candidate.humanId?.trim();
    const name = cleanNameHint(candidate.name ?? "");
    const key = email
      ? `email:${email}`
      : humanId
        ? `human:${humanId}`
        : name
          ? `name:${normalizeName(name)}`
          : "";

    if (!key) {
      continue;
    }

    const existing = byKey.get(key);
    byKey.set(key, {
      ...existing,
      ...candidate,
      name: existing?.name || name || candidate.name,
      email: existing?.email || candidate.email,
      isCurrentUser: existing?.isCurrentUser || candidate.isCurrentUser,
      isOrganizer: existing?.isOrganizer || candidate.isOrganizer,
    });
  }

  return Array.from(byKey.values());
}

async function getSystemPrompt(): Promise<string> {
  const result = await templateCommands.render({ eventContactSystem: {} });
  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

function parseExtractionJson(text: string): z.infer<typeof aiExtractionSchema> {
  try {
    return aiExtractionSchema.parse(JSON.parse(stripJsonFence(text)));
  } catch {
    throw new Error("Invalid contact extraction JSON");
  }
}

function stripJsonFence(text: string): string {
  const trimmed = text.trim();
  const match = trimmed.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/i);
  return match ? match[1].trim() : trimmed;
}

async function getUserPrompt(
  context: EventContactExtractionContext,
): Promise<string> {
  const result = await templateCommands.render({
    eventContactUser: {
      title: context.title?.trim() || null,
      description: trimEventText(context.description) || null,
      candidates: context.candidates.map(toTemplateCandidate),
    },
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

function toTemplateCandidate(
  candidate: EventContactCandidate,
): TemplateEventContactCandidate {
  return {
    name: candidate.name?.trim() || null,
    email: candidate.email?.trim() || null,
    isCurrentUser: !!candidate.isCurrentUser,
    isOrganizer: !!candidate.isOrganizer,
  };
}

function trimEventText(value: string | undefined): string {
  if (!value) {
    return "";
  }

  return stripHtml(value).trim().slice(0, MAX_EVENT_TEXT_CHARS);
}

function stripHtml(value: string): string {
  return value
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/\u00a0/g, " ");
}

function normalizeExtractedContacts(
  contacts: Array<{
    name?: string | null;
    email?: string | null;
    companyName?: string | null;
  }>,
  candidates: EventContactCandidate[],
): ExtractedEventContact[] {
  const deduped = new Map<string, ExtractedEventContact>();

  for (const contact of contacts) {
    const name = cleanNameHint(contact.name ?? "");
    const email = normalizeEmail(contact.email ?? undefined);
    if (!isLikelyPersonName(name) || isSelfReference(name, candidates)) {
      continue;
    }

    const matchedEmail = email || matchCandidateEmail(name, candidates);
    const companyName = normalizeCompanyName(contact.companyName);
    const normalizedContact: ExtractedEventContact = {
      name,
    };
    if (matchedEmail) {
      normalizedContact.email = matchedEmail;
    }
    if (companyName) {
      normalizedContact.companyName = companyName;
    }

    if (isSelfContactFromCandidates(normalizedContact, candidates)) {
      continue;
    }

    const key = matchedEmail
      ? `email:${matchedEmail}`
      : `name:${normalizeName(name)}`;
    if (!deduped.has(key)) {
      deduped.set(key, normalizedContact);
    } else if (
      !deduped.get(key)?.companyName &&
      normalizedContact.companyName
    ) {
      const existingContact = deduped.get(key);
      if (!existingContact) {
        continue;
      }
      deduped.set(key, {
        ...existingContact,
        companyName: normalizedContact.companyName,
      });
    }
  }

  return Array.from(deduped.values()).slice(0, MAX_CONTACTS_TO_EXTRACT);
}

function cleanNameHint(value: string): string {
  return value
    .replace(/\s+/g, " ")
    .replace(/^[#>*\-\s]+/, "")
    .replace(/\s+-\s+(organizer|host|required|optional)$/i, "")
    .replace(/^["'`]+/, "")
    .replace(/["'`,.;:]+$/, "")
    .trim();
}

function isLikelyPersonName(value: string): boolean {
  if (!value || value.length < 2 || value.length > 80) {
    return false;
  }

  if (value.includes("@") || /^https?:\/\//i.test(value)) {
    return false;
  }

  const normalized = normalizeName(value);
  if (
    !normalized ||
    [
      "what",
      "who",
      "invitee timezone",
      "meeting link",
      "zoom",
      "google meet",
      "teams",
    ].includes(normalized)
  ) {
    return false;
  }

  return (value.match(/\p{L}/gu)?.length ?? 0) >= 2;
}

function matchCandidateEmail(
  name: string,
  candidates: EventContactCandidate[],
): string | undefined {
  const nameTokens = tokenizeName(name);
  if (nameTokens.length === 0) {
    return undefined;
  }

  let best: { email: string; score: number } | null = null;
  const firstNameMatches: string[] = [];
  const firstNameToken = nameTokens[0];

  for (const candidate of candidates) {
    const email = normalizeEmail(candidate.email);
    if (!email || candidate.isCurrentUser) {
      continue;
    }

    const candidateTokens = new Set([
      ...tokenizeName(candidate.name ?? ""),
      ...tokenizeName(email.split("@")[0]?.replace(/[._+-]+/g, " ") ?? ""),
    ]);
    if (candidateTokens.size === 0) {
      continue;
    }

    const matched = nameTokens.filter((token) => candidateTokens.has(token));
    const score = matched.length / nameTokens.length;
    const enoughSignal =
      nameTokens.length === 1
        ? score === 1
        : score >= 0.67 || matched.length >= 2;

    if (enoughSignal && (!best || score > best.score)) {
      best = { email, score };
    } else if (
      firstNameToken &&
      nameTokens.length > 1 &&
      matched.length === 1 &&
      matched[0] === firstNameToken
    ) {
      firstNameMatches.push(email);
    }
  }

  return (
    best?.email ??
    (firstNameMatches.length === 1 ? firstNameMatches[0] : undefined)
  );
}

function isSelfReference(
  value: string,
  candidates: EventContactCandidate[],
): boolean {
  const normalized = normalizeName(value);
  if (!normalized) {
    return false;
  }

  return candidates.some((candidate) => {
    if (!candidate.isCurrentUser) {
      return false;
    }

    return getCandidateAliases(candidate).has(normalized);
  });
}

function isSelfContactFromCandidates(
  contact: ExtractedEventContact,
  candidates: EventContactCandidate[],
): boolean {
  const email = normalizeEmail(contact.email);
  const name = normalizeName(contact.name);

  return candidates.some((candidate) => {
    if (!candidate.isCurrentUser) {
      return false;
    }

    if (email && email === normalizeEmail(candidate.email)) {
      return true;
    }

    return getCandidateAliases(candidate).has(name);
  });
}

function isCurrentUserContact(
  contact: ExtractedEventContact,
  currentUser: Record<string, unknown> | undefined,
): boolean {
  if (!currentUser) {
    return false;
  }

  const email = normalizeEmail(contact.email);
  if (email && email === normalizeEmail(stringCell(currentUser.email))) {
    return true;
  }

  const currentUserCandidate: EventContactCandidate = {
    name: stringCell(currentUser.name),
    email: stringCell(currentUser.email),
    isCurrentUser: true,
  };
  return getCandidateAliases(currentUserCandidate).has(
    normalizeName(contact.name),
  );
}

function findContactForHuman(
  human: Record<string, unknown>,
  contacts: ExtractedEventContact[],
): ExtractedEventContact | undefined {
  const humanCandidate: EventContactCandidate = {
    name: stringCell(human.name),
    email: stringCell(human.email),
  };
  const humanEmail = normalizeEmail(humanCandidate.email);
  const humanName = normalizeName(humanCandidate.name ?? "");
  const humanAliases = getStrongCandidateAliases(humanCandidate);

  return contacts.find((contact) => {
    const contactEmail = normalizeEmail(contact.email);
    if (humanEmail && contactEmail === humanEmail) {
      return true;
    }

    const contactName = normalizeName(contact.name);
    if (contactName && humanAliases.has(contactName)) {
      return true;
    }

    const contactAliases = getStrongCandidateAliases({
      name: contact.name,
      email: contact.email,
    });
    return Boolean(humanName && contactAliases.has(humanName));
  });
}

function getCandidateAliases(candidate: EventContactCandidate): Set<string> {
  const aliases = getStrongCandidateAliases(candidate);
  const nameTokens = tokenizeName(candidate.name ?? "");

  if (nameTokens[0]) {
    aliases.add(nameTokens[0]);
  }

  const emailLocal = candidate.email?.split("@")[0];
  if (emailLocal) {
    const emailTokens = tokenizeName(emailLocal.replace(/[._+-]+/g, " "));
    const normalizedEmailLocal = normalizeName(
      emailLocal.replace(/[._+-]+/g, " "),
    );
    if (normalizedEmailLocal) {
      aliases.add(normalizedEmailLocal);
    }
    if (emailTokens[0]) {
      aliases.add(emailTokens[0]);
    }
  }

  return aliases;
}

function getStrongCandidateAliases(
  candidate: EventContactCandidate,
): Set<string> {
  const aliases = new Set<string>();
  const normalizedName = normalizeName(candidate.name ?? "");

  if (normalizedName) {
    aliases.add(normalizedName);
  }

  const emailLocal = candidate.email?.split("@")[0];
  if (emailLocal) {
    const normalizedEmailLocal = normalizeName(
      emailLocal.replace(/[._+-]+/g, " "),
    );
    if (normalizedEmailLocal) {
      aliases.add(normalizedEmailLocal);
    }
  }

  return aliases;
}

function shouldUpdateHumanName(
  existingName: string | undefined,
  email: string | undefined,
): boolean {
  const current = existingName?.trim() ?? "";
  if (!current) {
    return true;
  }

  if (email && normalizeEmail(current) === normalizeEmail(email)) {
    return true;
  }

  return current.includes("@");
}

function shouldUpdateHumanEmail(
  existingEmail: string | undefined,
  email: string | undefined,
): boolean {
  return Boolean(normalizeEmail(email) && !normalizeEmail(existingEmail));
}

function shouldUpdateHumanOrg(
  existingOrgId: string | undefined,
  orgId: string | undefined,
): boolean {
  return Boolean(orgId && !existingOrgId);
}

function buildHumansByEmailIndex(store: Store): Map<string, string> {
  const humansByEmail = new Map<string, string>();
  store.forEachRow("humans", (humanId, _forEachCell) => {
    const human = store.getRow("humans", humanId);
    const email = normalizeEmail(stringCell(human?.email));
    if (email) {
      humansByEmail.set(email, humanId);
    }
  });
  return humansByEmail;
}

function buildHumansByNameIndex(store: Store): Map<string, string> {
  const humansByName = new Map<string, string>();
  store.forEachRow("humans", (humanId, _forEachCell) => {
    const human = store.getRow("humans", humanId);
    const name = normalizeName(stringCell(human?.name) ?? "");
    if (name) {
      humansByName.set(name, humanId);
    }
  });
  return humansByName;
}

function buildOrganizationsByNameIndex(store: Store): Map<string, string> {
  const organizationsByName = new Map<string, string>();
  store.forEachRow("organizations", (orgId, _forEachCell) => {
    const organization = store.getRow("organizations", orgId);
    const name = normalizeName(stringCell(organization?.name) ?? "");
    if (name) {
      organizationsByName.set(name, orgId);
    }
  });
  return organizationsByName;
}

function getOrCreateOrganizationId(
  store: Store,
  organizationsByName: Map<string, string>,
  userId: string,
  companyName: string | undefined,
  createdAt: string,
): string | undefined {
  if (!companyName) {
    return undefined;
  }

  const nameKey = normalizeName(companyName);
  const existingOrgId = organizationsByName.get(nameKey);
  if (existingOrgId) {
    return existingOrgId;
  }

  const orgId = id();
  store.setRow("organizations", orgId, {
    user_id: userId,
    created_at: createdAt,
    name: companyName,
    pinned: false,
  } satisfies OrganizationStorage);
  organizationsByName.set(nameKey, orgId);
  return orgId;
}

function buildSessionHumansByNameIndex(
  store: Store,
  sessionMappings: Map<string, { source?: string }>,
): Map<string, string> {
  const humansByName = new Map<string, string>();
  for (const [humanId, mapping] of sessionMappings) {
    if (mapping.source === "excluded") {
      continue;
    }

    const human = store.getRow("humans", humanId);
    const name = normalizeName(stringCell(human?.name) ?? "");
    if (name) {
      humansByName.set(name, humanId);
    }
  }
  return humansByName;
}

function buildSessionMappingsByHuman(
  store: Store,
  sessionId: string,
): Map<string, { source?: string }> {
  const mappings = new Map<string, { source?: string }>();
  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mapping = store.getRow("mapping_session_participant", mappingId);
    if (mapping?.session_id !== sessionId) {
      return;
    }

    const humanId = stringCell(mapping.human_id);
    if (humanId) {
      mappings.set(humanId, { source: stringCell(mapping.source) });
    }
  });
  return mappings;
}

function getCurrentUserId(store: Store): string {
  const userId = store.getValue("user_id");
  return typeof userId === "string" && userId ? userId : DEFAULT_USER_ID;
}

function normalizeEmail(value: string | undefined | null): string | undefined {
  const email = value?.trim().toLowerCase();
  if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    return undefined;
  }

  return email;
}

function normalizeCompanyName(
  value: string | undefined | null,
): string | undefined {
  const name = value?.trim().replace(/\s+/g, " ");
  if (!name || name.length < 2 || name.length > 80) {
    return undefined;
  }

  if (name.includes("@") || /^https?:\/\//i.test(name)) {
    return undefined;
  }

  return name;
}

function normalizeName(value: string): string {
  return value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function tokenizeName(value: string): string[] {
  return normalizeName(value)
    .split(" ")
    .filter((token) => token.length > 1);
}

function stringCell(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
