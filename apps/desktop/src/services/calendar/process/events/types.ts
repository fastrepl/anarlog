import type { EventParticipant } from "@hypr/store";

import type {
  ExistingEvent,
  IncomingEvent,
  IncomingParticipants,
} from "../../fetch/types";

export type EventId = string;

export type EventsSyncInput = {
  incoming: IncomingEvent[];
  existing: ExistingEvent[];
  incomingParticipants: IncomingParticipants;
};

export type EventToAdd = IncomingEvent & {
  participants: EventParticipant[];
};

export type EventToUpdate = ExistingEvent &
  Omit<IncomingEvent, "tracking_id_calendar"> & {
    participants: EventParticipant[];
  };

export type EventsSyncOutput = {
  toDelete: EventId[];
  toUpdate: EventToUpdate[];
  toAdd: EventToAdd[];
};
