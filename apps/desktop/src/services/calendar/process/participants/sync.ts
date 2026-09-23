import type { EventParticipant } from "../../fetch/types";
import type {
  HumanToCreate,
  HumanToEnrich,
  ParticipantMappingToAdd,
  ParticipantsSyncInput,
  ParticipantsSyncOutput,
} from "./types";

import { deriveContactIdentity } from "~/contacts/identity";
import { id } from "~/shared/utils";

type SnapshotHuman = ParticipantsSyncInput["snapshot"]["humans"][number];

export function syncSessionParticipants({
  incomingParticipants,
  snapshot,
}: ParticipantsSyncInput): ParticipantsSyncOutput {
  const output: ParticipantsSyncOutput = {
    toDelete: [],
    toAdd: [],
    humansToCreate: [],
    humansToEnrich: [],
  };
  const humansByEmail = new Map<string, string>();
  const humansById = new Map<string, SnapshotHuman>();
  for (const human of snapshot.humans) {
    humansById.set(human.id, human);
    const email = human.email.trim().toLowerCase();
    if (email && !humansByEmail.has(email)) {
      humansByEmail.set(email, human.id);
    }
  }
  const mappingsBySession = new Map<
    string,
    Map<string, (typeof snapshot.mappings)[number]>
  >();
  for (const mapping of snapshot.mappings) {
    const sessionMappings =
      mappingsBySession.get(mapping.sessionId) ?? new Map();
    if (!sessionMappings.has(mapping.humanId)) {
      sessionMappings.set(mapping.humanId, mapping);
    }
    mappingsBySession.set(mapping.sessionId, sessionMappings);
  }
  const humansToCreate = new Map<string, HumanToCreate>();
  const humansToEnrich = new Map<string, HumanToEnrich>();

  for (const session of snapshot.sessions) {
    const eventParticipants = incomingParticipants.get(session.trackingId);
    if (!eventParticipants) continue;

    const changes = computeSessionParticipantChanges({
      sessionId: session.id,
      ownerUserId: session.ownerUserId,
      eventParticipants,
      humansByEmail,
      humansById,
      humansToCreate,
      humansToEnrich,
      existingMappings:
        mappingsBySession.get(session.id) ??
        new Map<string, (typeof snapshot.mappings)[number]>(),
    });
    output.toDelete.push(...changes.toDelete);
    output.toAdd.push(...changes.toAdd);
  }

  output.humansToCreate = Array.from(humansToCreate.values());
  output.humansToEnrich = Array.from(humansToEnrich.values());
  return output;
}

function computeSessionParticipantChanges({
  sessionId,
  ownerUserId,
  eventParticipants,
  humansByEmail,
  humansById,
  humansToCreate,
  humansToEnrich,
  existingMappings,
}: {
  sessionId: string;
  ownerUserId: string;
  eventParticipants: EventParticipant[];
  humansByEmail: Map<string, string>;
  humansById: Map<string, SnapshotHuman>;
  humansToCreate: Map<string, HumanToCreate>;
  humansToEnrich: Map<string, HumanToEnrich>;
  existingMappings: Map<
    string,
    { id: string; humanId: string; source: string }
  >;
}): { toDelete: string[]; toAdd: ParticipantMappingToAdd[] } {
  const eventHumans = new Map<string, { humanId: string; email: string }>();

  for (const participant of eventParticipants) {
    const email = participant.email?.trim();
    if (!email) continue;

    const emailKey = email.toLowerCase();
    let humanId = humansByEmail.get(emailKey);
    if (!humanId) {
      humanId = id();
      humansByEmail.set(emailKey, humanId);
      humansToCreate.set(emailKey, {
        id: humanId,
        ownerUserId,
        ...deriveContactIdentity({ name: participant.name, email }),
        email,
      });
    } else if (humanId !== ownerUserId && !humansToEnrich.has(humanId)) {
      const existing = humansById.get(humanId);
      const enrichment = existing
        ? planHumanEnrichment({ existing, participant, email, ownerUserId })
        : undefined;
      if (enrichment) {
        humansToEnrich.set(humanId, enrichment);
      }
    }
    eventHumans.set(humanId, { humanId, email });
  }

  const toAdd: ParticipantMappingToAdd[] = [];
  const toDelete: string[] = [];
  for (const { humanId, email } of eventHumans.values()) {
    const existing = existingMappings.get(humanId);
    if (!existing) {
      toAdd.push({ sessionId, humanId, email });
    }
  }

  for (const [humanId, mapping] of existingMappings) {
    if (mapping.source === "auto" && !eventHumans.has(humanId)) {
      toDelete.push(mapping.id);
    }
  }

  return { toDelete, toAdd };
}

function planHumanEnrichment({
  existing,
  participant,
  email,
  ownerUserId,
}: {
  existing: SnapshotHuman;
  participant: EventParticipant;
  email: string;
  ownerUserId: string;
}): HumanToEnrich | undefined {
  const derived = deriveContactIdentity({ name: participant.name, email });
  const enrichment: HumanToEnrich = { id: existing.id, ownerUserId };

  const currentName = existing.name.trim();
  if (
    (!currentName || currentName.includes("@")) &&
    derived.name !== email &&
    derived.name !== currentName
  ) {
    enrichment.name = derived.name;
  }
  if (!existing.organizationId && derived.companyName) {
    enrichment.companyName = derived.companyName;
  }

  return enrichment.name || enrichment.companyName ? enrichment : undefined;
}
