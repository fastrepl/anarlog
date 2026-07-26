import type { EventParticipant } from "@hypr/store";

export type { EventParticipant };

export type IncomingEvent = {
  tracking_id_event: string;
  tracking_id_calendar: string;
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

export type IncomingParticipants = Map<string, EventParticipant[]>;

export type ExistingEvent = {
  id: string;
  tracking_id_event: string;
  calendar_id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location: string;
  meeting_link: string;
  description: string;
  note: string;
  recurrence_series_id: string;
  has_recurrence_rules: boolean;
  is_all_day: boolean;
  provider: string;
  created_at: string;
  deleted_at: string | null;
};
