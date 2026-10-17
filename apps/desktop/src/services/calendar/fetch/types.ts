import type { EventParticipant } from "@anlg/store";

export type { EventParticipant };
export type { IncomingCalendarEvent as IncomingEvent } from "@anlg/plugin-calendar";

export type IncomingParticipants = Map<string, EventParticipant[]>;
