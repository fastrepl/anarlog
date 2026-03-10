import type {
  MappingSessionParticipantStorage,
  SessionStorage,
} from "@hypr/store";

export type ParticipantData = MappingSessionParticipantStorage & { id: string };

export type SessionMetaJson = Pick<
  SessionStorage,
  "user_id" | "created_at" | "title"
> & {
  id: string;
  event?: Record<string, unknown>;
  event_id?: string;
  participants: ParticipantData[];
  tags?: string[];
};

export type NoteFrontmatter = {
  id: string;
  session_id: string;
  template_id?: string;
  position?: number;
  title?: string;
};
