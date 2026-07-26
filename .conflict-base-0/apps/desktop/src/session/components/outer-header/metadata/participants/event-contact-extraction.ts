import { generateText, type LanguageModel } from "ai";
import { z } from "zod";

import {
  commands as templateCommands,
  type EventContactCandidate as TemplateEventContactCandidate,
} from "@hypr/plugin-template";
import type { EventParticipant, SessionEvent } from "@hypr/store";

import { deterministicGenerationSettings } from "~/ai/model-settings";

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

export type ContactEnhancementChanges = {
  name?: string;
  email?: string;
  companyName?: string;
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

export function buildEventContactExtractionContextFromRecords({
  sessionEvent,
  currentUserId,
  participants,
  eventParticipants,
}: {
  sessionEvent: SessionEvent | null;
  currentUserId: string;
  participants: Array<{
    humanId: string;
    name: string;
    email: string;
    source: string;
  }>;
  eventParticipants: EventParticipant[];
}): EventContactExtractionContext {
  return {
    title: sessionEvent?.title,
    description: sessionEvent?.description,
    candidates: dedupeCandidates([
      ...participants
        .filter((participant) => participant.source !== "excluded")
        .map((participant) => ({
          humanId: participant.humanId,
          name: participant.name,
          email: participant.email,
          isCurrentUser: participant.humanId === currentUserId,
        })),
      ...eventParticipants.map((participant) => ({
        name: participant.name,
        email: participant.email,
        isCurrentUser: participant.is_current_user,
        isOrganizer: participant.is_organizer,
      })),
    ]),
  };
}

export function planExtractedContactToHuman({
  humanId,
  userId,
  human,
  currentUser,
  mappingSource,
  contacts,
}: {
  humanId: string;
  userId: string;
  human: { name: string; email: string; organizationId: string } | undefined;
  currentUser: { name: string; email: string } | undefined;
  mappingSource: string | undefined;
  contacts: ExtractedEventContact[];
}): {
  result: ApplyContactEnhancementResult;
  changes: ContactEnhancementChanges;
} {
  const normalizedContacts = normalizeExtractedContacts(contacts, []);
  const result: ApplyContactEnhancementResult = {
    created: 0,
    updated: 0,
    linked: 0,
    skipped: 0,
    contacts: [],
    matched: false,
  };
  const changes: ContactEnhancementChanges = {};

  if (
    normalizedContacts.length === 0 ||
    !human ||
    !mappingSource ||
    mappingSource === "excluded"
  ) {
    if (normalizedContacts.length > 0) result.skipped += 1;
    return { result, changes };
  }

  const contact = findContactForHuman(human, normalizedContacts);
  if (!contact) {
    if (humanId === userId) {
      result.matched = true;
      result.contacts.push(normalizedContacts[0]!);
      result.skipped += 1;
    }
    return { result, changes };
  }

  result.matched = true;
  result.contacts.push(contact);
  if (humanId === userId || isCurrentUserContact(contact, currentUser)) {
    result.skipped += 1;
    return { result, changes };
  }

  if (shouldUpdateHumanName(human.name, contact.email)) {
    changes.name = contact.name;
  }
  if (shouldUpdateHumanEmail(human.email, contact.email)) {
    changes.email = contact.email;
  }
  if (!human.organizationId && contact.companyName) {
    changes.companyName = contact.companyName;
  }
  if (Object.keys(changes).length > 0) result.updated = 1;

  return { result, changes };
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
    ...deterministicGenerationSettings(model),
    maxRetries: 2,
    maxOutputTokens: 384,
    system,
    prompt,
  });

  const contacts = normalizeExtractedContacts(
    [
      ...inferContactsFromEventText(context),
      ...parseExtractionJson(result.text).contacts,
    ],
    context.candidates,
  );

  return { contacts, source: "model" };
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

function inferContactsFromEventText(
  context: EventContactExtractionContext,
): ExtractedEventContact[] {
  const contacts = new Map<string, ExtractedEventContact>();
  const lines = [context.title, context.description]
    .flatMap((value) => trimEventText(value).split(/\n+/))
    .map((line) => line.trim())
    .filter(Boolean);

  const addName = (value: string) => {
    const name = cleanNameHint(
      value
        .replace(/^[\w\s]+:\s*/, "")
        .replace(/<[^>]*@[^>]*>/g, "")
        .replace(/\([^)]*(organizer|host|required|optional)[^)]*\)/gi, ""),
    );
    const key = normalizeName(name);
    if (
      !key ||
      !isLikelyPersonName(name) ||
      isSelfReference(name, context.candidates)
    ) {
      return;
    }

    contacts.set(key, { name });
  };

  for (const line of lines) {
    const betweenMatch = line.match(
      /\bbetween\s+(.+?)(?:\s+(?:at|on|for)\b|$)/i,
    );
    if (betweenMatch?.[1]) {
      addDelimitedNames(betweenMatch[1], addName);
      continue;
    }

    if (line.includes("<>")) {
      addDelimitedNames(line, addName);
    }
  }

  return Array.from(contacts.values());
}

function addDelimitedNames(value: string, addName: (name: string) => void) {
  for (const part of value.split(/\s*(?:<>|<->)\s*|\s+\band\b\s+|\s+&\s+/i)) {
    addName(part);
  }
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
  const keysByName = new Map<string, string>();

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

    const nameKey = normalizeName(name);
    const key = matchedEmail ? `email:${matchedEmail}` : `name:${nameKey}`;
    const existingKey = deduped.has(key) ? key : keysByName.get(nameKey);
    const existingContact = existingKey ? deduped.get(existingKey) : undefined;
    if (!existingContact) {
      deduped.set(key, normalizedContact);
      keysByName.set(nameKey, key);
      continue;
    }

    const existingEmail = normalizeEmail(existingContact.email);
    if (matchedEmail && existingEmail && existingEmail !== matchedEmail) {
      if (existingKey) {
        deduped.delete(existingKey);
      }
      deduped.set(key, normalizedContact);
      keysByName.set(nameKey, key);
      continue;
    }

    const mergedContact: ExtractedEventContact = { name: existingContact.name };
    const mergedEmail = existingContact.email ?? normalizedContact.email;
    const mergedCompanyName =
      existingContact.companyName ?? normalizedContact.companyName;
    if (mergedEmail) {
      mergedContact.email = mergedEmail;
    }
    if (mergedCompanyName) {
      mergedContact.companyName = mergedCompanyName;
    }
    const mergedKey = mergedContact.email
      ? `email:${mergedContact.email}`
      : `name:${nameKey}`;

    if (existingKey && existingKey !== mergedKey) {
      deduped.delete(existingKey);
    }
    deduped.set(mergedKey, mergedContact);
    keysByName.set(nameKey, mergedKey);
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
