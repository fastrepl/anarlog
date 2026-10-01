import type { EventParticipant } from "@anlg/store";

import type { EventAttendanceSnapshotV1 } from "../attendance";

export type { EventParticipant };

export type IncomingEvent = {
  tracking_id_event: string;
  tracking_id_calendar: string;
  legacy_tracking_ids?: string[];
  is_cancelled?: boolean;
  title?: string;
  started_at?: string;
  ended_at?: string;
  location?: string;
  meeting_link?: string;
  description?: string;
  recurrence_series_id?: string;
  has_recurrence_rules: boolean;
  is_all_day: boolean;
  attendance?: EventAttendanceSnapshotV1 | null;
};

export type IncomingParticipants = Map<string, EventParticipant[]>;
