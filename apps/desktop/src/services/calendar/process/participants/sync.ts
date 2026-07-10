import type { EventParticipant } from "../../fetch/types";
import type {
  HumanToCreate,
  ParticipantMappingToAdd,
  ParticipantsSyncInput,
  ParticipantsSyncOutput,
} from "./types";

import { id } from "~/shared/utils";

export function syncSessionParticipants({
  incomingParticipants,
  snapshot,
}: ParticipantsSyncInput): ParticipantsSyncOutput {
  const output: ParticipantsSyncOutput = {
    toDelete: [],
    toAdd: [],
    humansToCreate: [],
  };
  const sessionsByTrackingId = new Map<
    string,
    (typeof snapshot.sessions)[number]
  >();
  for (const session of snapshot.sessions) {
    if (!sessionsByTrackingId.has(session.trackingId)) {
      sessionsByTrackingId.set(session.trackingId, session);
    }
  }
  const humansByEmail = new Map<string, string>();
  for (const human of snapshot.humans) {
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

  for (const [trackingId, eventParticipants] of incomingParticipants) {
    const session = sessionsByTrackingId.get(trackingId);
    if (!session) continue;

    const changes = computeSessionParticipantChanges({
      sessionId: session.id,
      ownerUserId: session.ownerUserId,
      eventParticipants,
      humansByEmail,
      humansToCreate,
      existingMappings:
        mappingsBySession.get(session.id) ??
        new Map<string, (typeof snapshot.mappings)[number]>(),
    });
    output.toDelete.push(...changes.toDelete);
    output.toAdd.push(...changes.toAdd);
  }

  output.humansToCreate = Array.from(humansToCreate.values());
  return output;
}

function computeSessionParticipantChanges({
  sessionId,
  ownerUserId,
  eventParticipants,
  humansByEmail,
  humansToCreate,
  existingMappings,
}: {
  sessionId: string;
  ownerUserId: string;
  eventParticipants: EventParticipant[];
  humansByEmail: Map<string, string>;
  humansToCreate: Map<string, HumanToCreate>;
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
        name: participant.name || email,
        email,
      });
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
