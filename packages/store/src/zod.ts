import { z } from "zod";

import { jsonObject, type ToStorageType } from "./shared";

export const humanSchema = z.object({
  user_id: z.string(),
  created_at: z.preprocess((val) => val ?? undefined, z.string().optional()),
  name: z.string(),
  email: z.string(),
  org_id: z.string(),
  job_title: z.preprocess((val) => val ?? undefined, z.string().optional()),
  linkedin_username: z.preprocess(
    (val) => val ?? undefined,
    z.string().optional(),
  ),
  memo: z.preprocess((val) => val ?? undefined, z.string().optional()),
  pinned: z.preprocess((val) => val ?? false, z.boolean()),
  pin_order: z.preprocess((val) => val ?? undefined, z.number().optional()),
});

export const sessionEventSchema = z.object({
  tracking_id: z.string(),
  calendar_id: z.string(),
  title: z.string(),
  started_at: z.string(),
  ended_at: z.string(),
  is_all_day: z.boolean(),
  has_recurrence_rules: z.boolean(),
  location: z.string().optional(),
  meeting_link: z.string().optional(),
  description: z.string().optional(),
  recurrence_series_id: z.string().optional(),
});

export const ignoredEventEntrySchema = z.object({
  tracking_id: z.string(),
  last_seen: z.string(),
});

export const ignoredRecurringSeriesEntrySchema = z.object({
  id: z.string(),
  last_seen: z.string(),
});

export const eventParticipantSchema = z.object({
  name: z.string().optional(),
  email: z.string().optional(),
  is_organizer: z.boolean().optional(),
  is_current_user: z.boolean().optional(),
});

export const calendarProviderSchema = z.enum(["apple", "google", "outlook"]);
export type CalendarProvider = z.infer<typeof calendarProviderSchema>;

export const eventSchema = z.object({
  user_id: z.string(),
  created_at: z.string(),
  tracking_id_event: z.string(),
  calendar_id: z.string(),
  title: z.string(),
  started_at: z.string(),
  ended_at: z.string(),
  location: z.preprocess((val) => val ?? undefined, z.string().optional()),
  meeting_link: z.preprocess((val) => val ?? undefined, z.string().optional()),
  description: z.preprocess((val) => val ?? undefined, z.string().optional()),
  note: z.preprocess((val) => val ?? undefined, z.string().optional()),
  recurrence_series_id: z.preprocess(
    (val) => val ?? undefined,
    z.string().optional(),
  ),
  has_recurrence_rules: z.preprocess(
    (val) => val ?? undefined,
    z.boolean().optional(),
  ),
  is_all_day: z.preprocess((val) => val ?? undefined, z.boolean().optional()),
  provider: calendarProviderSchema,
  participants_json: z.preprocess(
    (val) => val ?? undefined,
    z.string().optional(),
  ),
});

export const calendarSchema = z.object({
  user_id: z.string(),
  created_at: z.string(),
  tracking_id_calendar: z.string(),
  name: z.string(),
  enabled: z.preprocess((val) => val ?? false, z.boolean()),
  provider: calendarProviderSchema,
  source: z.preprocess((val) => val ?? undefined, z.string().optional()),
  color: z.preprocess((val) => val ?? undefined, z.string().optional()),
  connection_id: z.preprocess((val) => val ?? undefined, z.string().optional()),
});

export const organizationSchema = z.object({
  user_id: z.string(),
  created_at: z.preprocess((val) => val ?? undefined, z.string().optional()),
  name: z.string(),
  pinned: z.preprocess((val) => val ?? false, z.boolean()),
  pin_order: z.preprocess((val) => val ?? undefined, z.number().optional()),
});

export const sessionSchema = z.object({
  user_id: z.string(),
  created_at: z.string(),
  folder_id: z.preprocess((val) => val ?? undefined, z.string().optional()),
  event_json: z.preprocess((val) => val ?? undefined, z.string().optional()),
  title: z.string(),
  raw_md: z.string(),
});

export const transcriptSchema = z.object({
  user_id: z.string(),
  created_at: z.string(),
  session_id: z.string(),
  started_at: z.number(),
  ended_at: z.preprocess((val) => val ?? undefined, z.number().optional()),
  words: z.preprocess((val) => val ?? "[]", z.string()),
  speaker_hints: z.preprocess((val) => val ?? "[]", z.string()),
  memo_md: z.preprocess((val) => val ?? "", z.string()),
});

export const participantSourceSchema = z.enum(["manual", "auto", "excluded"]);
export type ParticipantSource = z.infer<typeof participantSourceSchema>;

export const mappingSessionParticipantSchema = z.object({
  user_id: z.string(),
  session_id: z.string(),
  human_id: z.string(),
  source: z.preprocess(
    (val) => val ?? undefined,
    participantSourceSchema.optional(),
  ),
});

export const tagSchema = z.object({
  user_id: z.string(),
  name: z.string(),
});

export const mappingTagSessionSchema = z.object({
  user_id: z.string(),
  tag_id: z.string(),
  session_id: z.string(),
});

export const mentionTargetTypeSchema = z.enum([
  "session",
  "human",
  "organization",
]);
export type MentionTargetType = z.infer<typeof mentionTargetTypeSchema>;

export const mentionSourceTypeSchema = z.enum(["session", "enhanced_note"]);
export type MentionSourceType = z.infer<typeof mentionSourceTypeSchema>;

export const mappingMentionSchema = z.object({
  user_id: z.string(),
  source_id: z.string(),
  source_type: mentionSourceTypeSchema,
  target_id: z.string(),
  target_type: mentionTargetTypeSchema,
});

export const templateSectionSchema = z.object({
  title: z.string(),
  description: z.string(),
});

export const templateSchema = z.object({
  user_id: z.string(),
  title: z.string(),
  description: z.string(),
  category: z.preprocess((val) => val ?? undefined, z.string().optional()),
  targets: z.preprocess(
    (val) => val ?? undefined,
    jsonObject(z.array(z.string())).optional(),
  ),
  sections: jsonObject(z.array(templateSectionSchema)),
});

export const chatGroupSchema = z.object({
  user_id: z.string(),
  created_at: z.string(),
  title: z.string(),
});

export const chatMessageStatusSchema = z.enum([
  "streaming",
  "ready",
  "error",
  "aborted",
]);

export const chatMessageSchema = z.object({
  user_id: z.string(),
  created_at: z.string(),
  chat_group_id: z.string(),
  role: z.string(),
  content: z.string(),
  metadata: jsonObject(z.any()),
  parts: jsonObject(z.any()),
  status: chatMessageStatusSchema.default("ready"),
});

export const chatShortcutSchema = z.object({
  user_id: z.string(),
  title: z.string(),
  content: z.string(),
});

export const memorySchema = z.object({
  user_id: z.string(),
  type: z.string(),
  text: z.string(),
  created_at: z.string(),
});

export const dailyNoteSchema = z.object({
  user_id: z.string(),
  date: z.string(),
  content: z.string(),
});

export const enhancedNoteSchema = z.object({
  user_id: z.string(),
  session_id: z.string(),
  content: z.string(),
  template_id: z.preprocess((val) => val ?? undefined, z.string().optional()),
  position: z.number(),
  title: z.preprocess((val) => val ?? undefined, z.string().optional()),
});

export const promptSchema = z.object({
  user_id: z.string(),
  task_type: z.string(),
  content: z.string(),
});

export const wordSchema = z.object({
  text: z.string(),
  start_ms: z.number(),
  end_ms: z.number(),
  channel: z.number(),
  speaker: z.preprocess((val) => val ?? undefined, z.string().optional()),
  metadata: z.preprocess(
    (val) => val ?? undefined,
    jsonObject(z.record(z.string(), z.unknown())).optional(),
  ),
});

export const speakerHintSchema = z.object({
  word_id: z.string(),
  type: z.string(),
  value: jsonObject(z.record(z.string(), z.unknown())),
});

export const providerSpeakerIndexSchema = z.object({
  speaker_index: z.number(),
  provider: z.string().optional(),
  channel: z.number().optional(),
});

export const generalSchema = z.object({
  user_id: z.string(),
  autostart: z.boolean().default(false),
  telemetry_consent: z.boolean().default(true),
  save_recordings: z.boolean().default(true),
  notification_event: z.boolean().default(true),
  notification_detect: z.boolean().default(true),
  respect_dnd: z.boolean().default(false),
  quit_intercept: z.boolean().default(false),
  ai_language: z.string().default("en"),
  spoken_languages: jsonObject(z.array(z.string()).default(["en"])),
  ignored_platforms: jsonObject(z.array(z.string()).default([])),
  included_platforms: jsonObject(z.array(z.string()).default([])),
  ignored_events: jsonObject(z.array(ignoredEventEntrySchema).default([])),
  ignored_recurring_series: jsonObject(
    z.array(ignoredRecurringSeriesEntrySchema).default([]),
  ),
  current_llm_provider: z.string().optional(),
  current_llm_model: z.string().optional(),
  current_stt_provider: z.string().optional(),
  current_stt_model: z.string().optional(),
  timezone: z.string().optional(),
  week_start: z.string().optional(),
});

export const aiProviderSchema = z
  .object({
    type: z.enum(["stt", "llm"]),
    base_url: z.url().min(1),
    api_key: z.string(),
  })
  .refine(
    (data) => !data.base_url.startsWith("https:") || data.api_key.length > 0,
    {
      message: "API key is required for HTTPS URLs",
      path: ["api_key"],
    },
  );

export type ProviderSpeakerIndexHint = z.infer<
  typeof providerSpeakerIndexSchema
>;

export type Human = z.infer<typeof humanSchema>;
export type SessionEvent = z.infer<typeof sessionEventSchema>;
export type IgnoredEvent = z.infer<typeof ignoredEventEntrySchema>;
export type IgnoredRecurringSeries = z.infer<
  typeof ignoredRecurringSeriesEntrySchema
>;
export type EventParticipant = z.infer<typeof eventParticipantSchema>;
export type Event = z.infer<typeof eventSchema>;
export type Calendar = z.infer<typeof calendarSchema>;
export type CalendarStorage = ToStorageType<typeof calendarSchema>;
export type Organization = z.infer<typeof organizationSchema>;
export type Session = z.infer<typeof sessionSchema>;
export type Transcript = z.infer<typeof transcriptSchema>;
export type Word = z.infer<typeof wordSchema>;
export type SpeakerHint = z.infer<typeof speakerHintSchema>;
export type MappingSessionParticipant = z.infer<
  typeof mappingSessionParticipantSchema
>;
export type Tag = z.infer<typeof tagSchema>;
export type MappingTagSession = z.infer<typeof mappingTagSessionSchema>;
export type MappingMention = z.infer<typeof mappingMentionSchema>;
export type Template = z.infer<typeof templateSchema>;
export type TemplateSection = z.infer<typeof templateSectionSchema>;
export type ChatGroup = z.infer<typeof chatGroupSchema>;
export type ChatMessageStatus = z.infer<typeof chatMessageStatusSchema>;
export type ChatMessage = z.infer<typeof chatMessageSchema>;
export type ChatShortcut = z.infer<typeof chatShortcutSchema>;
export type Memory = z.infer<typeof memorySchema>;
export type DailyNote = z.infer<typeof dailyNoteSchema>;
export type EnhancedNote = z.infer<typeof enhancedNoteSchema>;
export type Prompt = z.infer<typeof promptSchema>;
export type AIProvider = z.infer<typeof aiProviderSchema>;
export type General = z.infer<typeof generalSchema>;

export type SessionStorage = ToStorageType<typeof sessionSchema>;
export type TranscriptStorage = ToStorageType<typeof transcriptSchema>;
export type WordStorage = ToStorageType<typeof wordSchema>;
export type SpeakerHintStorage = ToStorageType<typeof speakerHintSchema>;
export type TemplateStorage = ToStorageType<typeof templateSchema>;
export type ChatMessageStorage = ToStorageType<typeof chatMessageSchema>;
export type EnhancedNoteStorage = ToStorageType<typeof enhancedNoteSchema>;
export type HumanStorage = ToStorageType<typeof humanSchema>;
export type OrganizationStorage = ToStorageType<typeof organizationSchema>;
export type PromptStorage = ToStorageType<typeof promptSchema>;
export type ChatShortcutStorage = ToStorageType<typeof chatShortcutSchema>;
export type MemoryStorage = ToStorageType<typeof memorySchema>;
export type DailyNoteStorage = ToStorageType<typeof dailyNoteSchema>;
export type EventStorage = ToStorageType<typeof eventSchema>;
export type MappingSessionParticipantStorage = ToStorageType<
  typeof mappingSessionParticipantSchema
>;
export type MappingMentionStorage = ToStorageType<typeof mappingMentionSchema>;
export type AIProviderStorage = ToStorageType<typeof aiProviderSchema>;
export type GeneralStorage = ToStorageType<typeof generalSchema>;
