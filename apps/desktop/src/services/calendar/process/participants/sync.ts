import type { Ctx } from "../../ctx";
import type { EventParticipant } from "../../fetch/types";
import type {
  HumanToCreate,
  ParticipantMappingToAdd,
  ParticipantsSyncInput,
  ParticipantsSyncOutput,
} from "./types";

import { findSessionByTrackingId } from "~/session/utils";
import { id } from "~/shared/utils";
import type { Store } from "~/store/tinybase/store/main";

export function syncSessionParticipants(
  ctx: Ctx,
  input: ParticipantsSyncInput,
): ParticipantsSyncOutput {
  const output: ParticipantsSyncOutput = {
    toDelete: [],
    toAdd: [],
    humansToCreate: [],
  };

  const humansByEmail = buildHumansByEmailIndex(ctx.store);
  const humansToCreateMap = new Map<string, HumanToCreate>();

  for (const [trackingId, participants] of input.incomingParticipants) {
    const sessionId = findSessionByTrackingId(ctx.store, trackingId);
    if (!sessionId) {
      continue;
    }

    const sessionOutput = computeSessionParticipantChanges(
      ctx.store,
      sessionId,
      participants,
      humansByEmail,
      humansToCreateMap,
    );

    output.toDelete.push(...sessionOutput.toDelete);
    output.toAdd.push(...sessionOutput.toAdd);
  }

  output.humansToCreate = Array.from(humansToCreateMap.values());

  return output;
}

function buildHumansByEmailIndex(store: Store): Map<string, string> {
  const humansByEmail = new Map<string, string>();

  store.forEachRow("humans", (humanId, _forEachCell) => {
    const human = store.getRow("humans", humanId);
    const email = human?.email;
    if (email && typeof email === "string" && email.trim()) {
      humansByEmail.set(email.toLowerCase(), humanId);
    }
  });

  return humansByEmail;
}

function computeSessionParticipantChanges(
  store: Store,
  sessionId: string,
  eventParticipants: EventParticipant[],
  humansByEmail: Map<string, string>,
  humansToCreateMap: Map<string, HumanToCreate>,
): { toDelete: string[]; toAdd: ParticipantMappingToAdd[] } {
  const eventHumanIds = new Set<string>();
  for (const participant of eventParticipants) {
    if (!participant.email) {
      continue;
    }

    const emailLower = participant.email.toLowerCase();
    let humanId = humansByEmail.get(emailLower);

    if (!humanId) {
      const existing = humansToCreateMap.get(emailLower);
      if (existing) {
        humanId = existing.id;
      } else {
        humanId = id();
        humansToCreateMap.set(emailLower, {
          id: humanId,
          name: participant.name || participant.email,
          email: participant.email,
        });
        humansByEmail.set(emailLower, humanId);
      }
    }

    eventHumanIds.add(humanId);
  }

  const existingMappings = getExistingMappings(store, sessionId);

  const toAdd: ParticipantMappingToAdd[] = [];
  const toDelete: string[] = [];

  for (const humanId of eventHumanIds) {
    const existing = existingMappings.get(humanId);
    if (!existing) {
      toAdd.push({ sessionId, humanId });
    } else if (existing.source === "excluded") {
      continue;
    }
  }

  for (const [humanId, mapping] of existingMappings) {
    if (mapping.source === "auto" && !eventHumanIds.has(humanId)) {
      toDelete.push(mapping.id);
    }
  }

  return { toDelete, toAdd };
}

type MappingInfo = {
  id: string;
  humanId: string;
  source: string | undefined;
};

function getExistingMappings(
  store: Store,
  sessionId: string,
): Map<string, MappingInfo> {
  const mappings = new Map<string, MappingInfo>();

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mapping = store.getRow("mapping_session_participant", mappingId);
    if (mapping?.session_id === sessionId && mapping.human_id) {
      const humanId = mapping.human_id;
      mappings.set(humanId, {
        id: mappingId,
        humanId,
        source: mapping.source,
      });
    }
  });

  return mappings;
}
