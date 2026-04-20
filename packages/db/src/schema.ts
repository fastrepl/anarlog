import { index, integer, sqliteTable, text } from "drizzle-orm/sqlite-core";

export const sessions = sqliteTable(
  "sessions",
  {
    id: text("id").primaryKey().notNull(),
    title: text("title").notNull().default(""),
    rawMd: text("raw_md").notNull().default(""),
    folderId: text("folder_id").notNull().default(""),
    eventJson: text("event_json").notNull().default(""),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [index("idx_sessions_updated_at").on(table.updatedAt)],
);

export const dailyNotes = sqliteTable(
  "daily_notes",
  {
    date: text("date").primaryKey().notNull(),
    content: text("content").notNull().default(""),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [index("idx_daily_notes_updated_at").on(table.updatedAt)],
);

export const tasks = sqliteTable(
  "tasks",
  {
    id: text("id").primaryKey().notNull(),
    sourceType: text("source_type").notNull().default(""),
    sourceId: text("source_id").notNull().default(""),
    sourceOrder: integer("source_order").notNull().default(0),
    status: text("status").notNull().default("todo"),
    textPreview: text("text_preview").notNull().default(""),
    bodyJson: text("body_json").notNull().default("[]"),
    dueDate: text("due_date"),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [
    index("idx_tasks_source_order").on(
      table.sourceType,
      table.sourceId,
      table.sourceOrder,
    ),
    index("idx_tasks_status").on(table.status),
  ],
);

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

export const calendars = sqliteTable(
  "calendars",
  {
    id: text("id").primaryKey().notNull(),
    trackingIdCalendar: text("tracking_id_calendar").notNull().default(""),
    name: text("name").notNull().default(""),
    enabled: integer("enabled", { mode: "boolean" }).notNull().default(false),
    provider: text("provider").notNull().default(""),
    source: text("source").notNull().default(""),
    color: text("color").notNull().default("#888"),
    connectionId: text("connection_id").notNull().default(""),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [index("idx_calendars_provider").on(table.provider)],
);

export const events = sqliteTable(
  "events",
  {
    id: text("id").primaryKey().notNull(),
    trackingIdEvent: text("tracking_id_event").notNull().default(""),
    calendarId: text("calendar_id").notNull().default(""),
    title: text("title").notNull().default(""),
    startedAt: text("started_at").notNull().default(""),
    endedAt: text("ended_at").notNull().default(""),
    location: text("location").notNull().default(""),
    meetingLink: text("meeting_link").notNull().default(""),
    description: text("description").notNull().default(""),
    note: text("note").notNull().default(""),
    recurrenceSeriesId: text("recurrence_series_id").notNull().default(""),
    hasRecurrenceRules: integer("has_recurrence_rules", {
      mode: "boolean",
    })
      .notNull()
      .default(false),
    isAllDay: integer("is_all_day", {
      mode: "boolean",
    })
      .notNull()
      .default(false),
    provider: text("provider").notNull().default(""),
    participantsJson: text("participants_json", { mode: "json" }),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => [
    index("idx_events_calendar_id").on(table.calendarId),
    index("idx_events_started_at").on(table.startedAt),
  ],
);
