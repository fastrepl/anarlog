import type { IncomingParticipants } from "../../fetch/types";
import type { ParticipantSyncSnapshot } from "../../storage";

export type ParticipantMappingId = string;

export type ParticipantsSyncInput = {
  incomingParticipants: IncomingParticipants;
  snapshot: ParticipantSyncSnapshot;
};

export type ParticipantMappingToAdd = {
  sessionId: string;
  humanId: string;
  email: string;
};

export type HumanToCreate = {
  id: string;
  ownerUserId: string;
  name: string;
  email: string;
};

export type ParticipantsSyncOutput = {
  toDelete: ParticipantMappingId[];
  toAdd: ParticipantMappingToAdd[];
  humansToCreate: HumanToCreate[];
};
