import type { EventParticipant } from "@hypr/store";

import type { Store } from "~/store/tinybase/store/main";

export type { EventParticipant };

export interface ReconcileCtx {
  store: Store;
}

export type ReconcileIncomingEvent = {
  tracking_id_event: string;
  calendar_id: string;
  title?: string;
  started_at?: string;
  ended_at?: string;
  location?: string;
  meeting_link?: string;
  description?: string;
  recurrence_series_id?: string;
  has_recurrence_rules: boolean;
  is_all_day: boolean;
};

export type IncomingParticipantState =
  | {
      type: "observed";
      participants: EventParticipant[];
    }
  | {
      type: "deleted";
    };

export type IncomingParticipants = Map<string, IncomingParticipantState>;
