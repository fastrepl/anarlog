import {
  blob,
  index,
  integer,
  sqliteTable,
  text,
  uniqueIndex,
} from "drizzle-orm/sqlite-core";

export const dailyNotes = sqliteTable(
  "daily_notes",
  {
    id: text("id").primaryKey().notNull(),
    date: text("date").notNull().default(""),
    body: text("body", { mode: "json" }).notNull().default("{}"),
    userId: text("user_id").notNull().default(""),
    visibility: text("visibility").notNull().default("public"),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [
    uniqueIndex("idx_daily_notes_date_user").on(table.date, table.userId),
  ],
);

export const dailySummaries = sqliteTable(
  "daily_summaries",
  {
    id: text("id").primaryKey().notNull(),
    dailyNoteId: text("daily_note_id").notNull().default(""),
    date: text("date").notNull().default(""),
    content: text("content").notNull().default(""),
    timelineJson: text("timeline_json", { mode: "json" })
      .notNull()
      .default("[]"),
    topicsJson: text("topics_json", { mode: "json" }).notNull().default("[]"),
    status: text("status").notNull().default("idle"),
    sourceCursorMs: integer("source_cursor_ms").notNull().default(0),
    sourceFingerprint: text("source_fingerprint").notNull().default(""),
    generationError: text("generation_error").notNull().default(""),
    generatedAt: text("generated_at").notNull().default(""),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [
    uniqueIndex("idx_daily_summaries_daily_note").on(table.dailyNoteId),
    index("idx_daily_summaries_date").on(table.date),
  ],
);

export const activityObservationEvents = sqliteTable(
  "activity_observation_events",
  {
    id: text("id").primaryKey().notNull(),
    observationId: text("observation_id").notNull(),
    occurredAtMs: integer("occurred_at_ms").notNull().default(0),
    eventKind: text("event_kind").notNull().default(""),
    endReason: text("end_reason"),
    changeClass: text("change_class"),
    appId: text("app_id").notNull().default(""),
    bundleId: text("bundle_id").notNull().default(""),
    appName: text("app_name").notNull().default(""),
    activityKind: text("activity_kind").notNull().default(""),
    windowTitle: text("window_title").notNull().default(""),
    url: text("url").notNull().default(""),
    domain: text("domain").notNull().default(""),
    textAnchorIdentity: text("text_anchor_identity").notNull().default(""),
    observationKey: text("observation_key").notNull().default(""),
    snapshotJson: text("snapshot_json", { mode: "json" })
      .notNull()
      .default("{}"),
    createdAt: text("created_at").notNull(),
  },
  (table) => [
    index("idx_activity_observation_events_occurred_at_ms").on(
      table.occurredAtMs,
    ),
    index("idx_activity_observation_events_observation_id").on(
      table.observationId,
    ),
    index("idx_activity_observation_events_app_id").on(table.appId),
  ],
);

export const activityScreenshots = sqliteTable(
  "activity_screenshots",
  {
    id: text("id").primaryKey().notNull(),
    observationId: text("observation_id").notNull(),
    screenshotKind: text("screenshot_kind").notNull().default(""),
    scheduledAtMs: integer("scheduled_at_ms").notNull().default(0),
    capturedAtMs: integer("captured_at_ms").notNull().default(0),
    appName: text("app_name").notNull().default(""),
    windowTitle: text("window_title").notNull().default(""),
    mimeType: text("mime_type").notNull().default(""),
    width: integer("width").notNull().default(0),
    height: integer("height").notNull().default(0),
    sha256: text("sha256").notNull().default(""),
    imageBlob: blob("image_blob").notNull(),
    snapshotJson: text("snapshot_json", { mode: "json" })
      .notNull()
      .default("{}"),
    createdAt: text("created_at").notNull(),
  },
  (table) => [
    index("idx_activity_screenshots_captured_at_ms").on(table.capturedAtMs),
    index("idx_activity_screenshots_observation_id").on(table.observationId),
  ],
);

export const activityObservationAnalyses = sqliteTable(
  "activity_observation_analyses",
  {
    id: text("id").primaryKey().notNull(),
    observationId: text("observation_id").notNull(),
    screenshotId: text("screenshot_id").notNull(),
    screenshotKind: text("screenshot_kind").notNull().default(""),
    capturedAtMs: integer("captured_at_ms").notNull().default(0),
    modelName: text("model_name").notNull().default(""),
    promptVersion: text("prompt_version").notNull().default(""),
    appName: text("app_name").notNull().default(""),
    windowTitle: text("window_title").notNull().default(""),
    summary: text("summary").notNull().default(""),
    createdAt: text("created_at").notNull(),
  },
  (table) => [
    uniqueIndex("idx_activity_observation_analyses_dedup").on(
      table.screenshotId,
      table.modelName,
      table.promptVersion,
    ),
    index("idx_activity_observation_analyses_captured_at_ms").on(
      table.capturedAtMs,
    ),
    index("idx_activity_observation_analyses_observation_id").on(
      table.observationId,
    ),
  ],
);

export const promptOverrides = sqliteTable("prompt_overrides", {
  taskType: text("task_type").primaryKey(),
  content: text("content").notNull().default(""),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});

export const templates = sqliteTable("templates", {
  id: text("id").primaryKey(),
  title: text("title").notNull().default(""),
  description: text("description").notNull().default(""),
  pinned: integer("pinned", { mode: "boolean" }).notNull().default(false),
  pinOrder: integer("pin_order"),
  category: text("category"),
  targetsJson: text("targets_json", { mode: "json" }),
  sectionsJson: text("sections_json", { mode: "json" }).notNull().default("[]"),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});
