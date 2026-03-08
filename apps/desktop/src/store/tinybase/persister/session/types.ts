import type {
  MappingSessionParticipantStorage,
  SessionStorage,
  SpeakerHintStorage,
  TranscriptStorage,
  WordStorage,
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

export type TranscriptWithData = Pick<
  TranscriptStorage,
  "user_id" | "created_at" | "session_id" | "started_at" | "memo_md"
> & {
  id: string;
  ended_at?: number;
  words: Array<WordStorage & { id: string }>;
  speaker_hints: Array<SpeakerHintStorage & { id: string }>;
};

export type TranscriptJson = {
  transcripts: TranscriptWithData[];
};

export type NoteFrontmatter = {
  id: string;
  session_id: string;
  template_id?: string;
  position?: number;
  title?: string;
};
