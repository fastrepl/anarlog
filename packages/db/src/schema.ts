import { sql } from "drizzle-orm";
import {
  index,
  integer,
  primaryKey,
  sqliteTable,
  text,
  uniqueIndex,
} from "drizzle-orm/sqlite-core";

const currentTimestamp = sql`(strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))`;

export const templates = sqliteTable("templates", {
  id: text("id").primaryKey(),
  title: text("title").notNull().default(""),
  description: text("description").notNull().default(""),
  pinned: integer("pinned", { mode: "boolean" }).notNull().default(false),
  pinOrder: integer("pin_order"),
  category: text("category"),
  iconJson: text("icon_json", { mode: "json" })
    .notNull()
    .default('{"type":"icon","value":"notebook-tabs","color":"#9ca3af"}'),
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
    deletedAt: text("deleted_at"),
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
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_events_calendar_id").on(table.calendarId),
    index("idx_events_started_at").on(table.startedAt),
  ],
);

export const workspaces = sqliteTable(
  "workspaces",
  {
    id: text("id").primaryKey().notNull(),
    ownerUserId: text("owner_user_id").notNull().default(""),
    kind: text("kind").notNull().default("personal"),
    name: text("name").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [index("idx_workspaces_owner_user_id").on(table.ownerUserId)],
);

export const workspaceMemberships = sqliteTable(
  "workspace_memberships",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    userId: text("user_id").notNull().default(""),
    role: text("role").notNull().default("member"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [index("idx_workspace_memberships_user_id").on(table.userId)],
);

export const organizations = sqliteTable("organizations", {
  id: text("id").primaryKey().notNull(),
  workspaceId: text("workspace_id").notNull().default(""),
  ownerUserId: text("owner_user_id").notNull().default(""),
  name: text("name").notNull().default(""),
  memo: text("memo").notNull().default(""),
  pinned: integer("pinned", { mode: "boolean" }).notNull().default(false),
  pinOrder: integer("pin_order"),
  metadataJson: text("metadata_json").notNull().default("{}"),
  createdAt: text("created_at").notNull().default(currentTimestamp),
  updatedAt: text("updated_at").notNull().default(currentTimestamp),
  deletedAt: text("deleted_at"),
});

export const humans = sqliteTable(
  "humans",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    organizationId: text("organization_id").notNull().default(""),
    name: text("name").notNull().default(""),
    email: text("email").notNull().default(""),
    phone: text("phone").notNull().default(""),
    jobTitle: text("job_title").notNull().default(""),
    linkedinUsername: text("linkedin_username").notNull().default(""),
    memo: text("memo").notNull().default(""),
    pinned: integer("pinned", { mode: "boolean" }).notNull().default(false),
    pinOrder: integer("pin_order"),
    metadataJson: text("metadata_json").notNull().default("{}"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_humans_organization_id").on(table.organizationId),
    index("idx_humans_email").on(table.email),
  ],
);

export const sessions = sqliteTable(
  "sessions",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    title: text("title").notNull().default(""),
    kind: text("kind").notNull().default("meeting"),
    status: text("status").notNull().default("active"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    startedAt: text("started_at").notNull().default(""),
    endedAt: text("ended_at").notNull().default(""),
    timezone: text("timezone").notNull().default(""),
    language: text("language").notNull().default(""),
    eventId: text("event_id").notNull().default(""),
    externalEventId: text("external_event_id").notNull().default(""),
    externalProvider: text("external_provider").notNull().default(""),
    seriesId: text("series_id").notNull().default(""),
    sourceAppsJson: text("source_apps_json").notNull().default("[]"),
    eventJson: text("event_json").notNull().default(""),
    folderPath: text("folder_path").notNull().default(""),
    slug: text("slug").notNull().default(""),
    metadataJson: text("metadata_json").notNull().default("{}"),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_sessions_created_at").on(table.createdAt),
    index("idx_sessions_folder_path").on(table.folderPath),
    index("idx_sessions_event_id").on(table.eventId),
  ],
);

export const sessionDocuments = sqliteTable(
  "session_documents",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    sessionId: text("session_id").notNull().default(""),
    kind: text("kind").notNull().default("note"),
    templateId: text("template_id").notNull().default(""),
    title: text("title").notNull().default(""),
    bodyFormat: text("body_format").notNull().default("prosemirror_json"),
    body: text("body").notNull().default(""),
    sourceHash: text("source_hash").notNull().default(""),
    generationMetadataJson: text("generation_metadata_json")
      .notNull()
      .default("{}"),
    sortOrder: integer("sort_order").notNull().default(0),
    createdBy: text("created_by").notNull().default(""),
    updatedBy: text("updated_by").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_session_documents_session_id").on(table.sessionId),
    index("idx_session_documents_kind").on(table.kind),
  ],
);

export const sharedSessionCache = sqliteTable(
  "shared_session_cache",
  {
    shareId: text("share_id").notNull(),
    viewerUserId: text("viewer_user_id").notNull(),
    workspaceId: text("workspace_id").notNull(),
    sessionId: text("session_id").notNull(),
    schemaVersion: integer("schema_version").notNull().default(1),
    contentRevision: integer("content_revision").notNull(),
    title: text("title").notNull().default(""),
    bodyJson: text("body_json", { mode: "json" }).notNull(),
    attachmentsJson: text("attachments_json", { mode: "json" })
      .notNull()
      .default("[]"),
    capability: text("capability", {
      enum: ["viewer", "commenter", "editor"],
    })
      .notNull()
      .default("viewer"),
    manageAccess: integer("manage_access", { mode: "boolean" })
      .notNull()
      .default(false),
    accessVersion: integer("access_version").notNull(),
    webEditable: integer("web_editable", { mode: "boolean" })
      .notNull()
      .default(false),
    webEditBaseContentRevision: integer("web_edit_base_content_revision"),
    webEditBaseTitle: text("web_edit_base_title"),
    webEditBaseBodyJson: text("web_edit_base_body_json", { mode: "json" }),
    publishedAt: text("published_at").notNull(),
    cachedAt: text("cached_at").notNull().default(currentTimestamp),
  },
  (table) => [
    primaryKey({ columns: [table.viewerUserId, table.shareId] }),
    index("idx_shared_session_cache_viewer_workspace").on(
      table.viewerUserId,
      table.workspaceId,
    ),
  ],
);

export const sessionShareSyncState = sqliteTable(
  "session_share_sync_state",
  {
    viewerUserId: text("viewer_user_id").notNull(),
    shareId: text("share_id").notNull(),
    sessionId: text("session_id").notNull(),
    acknowledgedContentRevision: integer(
      "acknowledged_content_revision",
    ).notNull(),
    baselineSourceHash: text("baseline_source_hash").notNull(),
    status: text("status", { enum: ["clean", "conflict"] })
      .notNull()
      .default("clean"),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
  },
  (table) => [
    primaryKey({ columns: [table.viewerUserId, table.shareId] }),
    index("idx_session_share_sync_state_session").on(
      table.viewerUserId,
      table.sessionId,
    ),
  ],
);

export const transcripts = sqliteTable(
  "transcripts",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    sessionId: text("session_id").notNull().default(""),
    source: text("source").notNull().default(""),
    provider: text("provider").notNull().default(""),
    model: text("model").notNull().default(""),
    language: text("language").notNull().default(""),
    startedAtMs: integer("started_at_ms").notNull().default(0),
    endedAtMs: integer("ended_at_ms"),
    audioAttachmentId: text("audio_attachment_id").notNull().default(""),
    memo: text("memo").notNull().default(""),
    wordsJson: text("words_json").notNull().default("[]"),
    speakerHintsJson: text("speaker_hints_json").notNull().default("[]"),
    metadataJson: text("metadata_json").notNull().default("{}"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [index("idx_transcripts_session_id").on(table.sessionId)],
);

export const sessionParticipants = sqliteTable(
  "session_participants",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    sessionId: text("session_id").notNull().default(""),
    humanId: text("human_id").notNull().default(""),
    displayName: text("display_name").notNull().default(""),
    email: text("email").notNull().default(""),
    role: text("role").notNull().default(""),
    source: text("source").notNull().default(""),
    metadataJson: text("metadata_json").notNull().default("{}"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_session_participants_session_id").on(table.sessionId),
    index("idx_session_participants_human_id").on(table.humanId),
  ],
);

export const actionItems = sqliteTable(
  "action_items",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    sessionId: text("session_id").notNull().default(""),
    sourceType: text("source_type").notNull().default(""),
    sourceId: text("source_id").notNull().default(""),
    sourceOrder: integer("source_order").notNull().default(0),
    assigneeHumanId: text("assignee_human_id").notNull().default(""),
    status: text("status").notNull().default("todo"),
    text: text("text").notNull().default(""),
    bodyJson: text("body_json").notNull().default("{}"),
    dueAt: text("due_at").notNull().default(""),
    completedAt: text("completed_at"),
    createdBy: text("created_by").notNull().default(""),
    updatedBy: text("updated_by").notNull().default(""),
    metadataJson: text("metadata_json").notNull().default("{}"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_action_items_session_id").on(table.sessionId),
    index("idx_action_items_source").on(table.sourceType, table.sourceId),
  ],
);

export const sessionAttachments = sqliteTable(
  "session_attachments",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    sessionId: text("session_id").notNull().default(""),
    filename: text("filename").notNull().default(""),
    relativePath: text("relative_path").notNull().default(""),
    contentType: text("content_type").notNull().default(""),
    sizeBytes: integer("size_bytes").notNull().default(0),
    sha256: text("sha256").notNull().default(""),
    storageKind: text("storage_kind").notNull().default("local_file"),
    cloudObjectKey: text("cloud_object_key").notNull().default(""),
    cloudSyncEnabled: integer("cloud_sync_enabled").notNull().default(0),
    sourceType: text("source_type").notNull().default(""),
    sourceId: text("source_id").notNull().default(""),
    metadataJson: text("metadata_json").notNull().default("{}"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [index("idx_session_attachments_session_id").on(table.sessionId)],
);

export const attachmentLocalState = sqliteTable(
  "attachment_local_state",
  {
    attachmentId: text("attachment_id").primaryKey().notNull(),
    sessionId: text("session_id").notNull().default(""),
    relativePath: text("relative_path").notNull().default(""),
    availability: text("availability").notNull().default("present"),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
  },
  (table) => [
    index("idx_attachment_local_state_session_id").on(table.sessionId),
  ],
);

export const attachmentTransferJobs = sqliteTable(
  "attachment_transfer_jobs",
  {
    id: text("id").primaryKey().notNull(),
    attachmentId: text("attachment_id").notNull(),
    sessionId: text("session_id").notNull(),
    workspaceId: text("workspace_id").notNull(),
    direction: text("direction").notNull(),
    expectedSha256: text("expected_sha256").notNull(),
    expectedSizeBytes: integer("expected_size_bytes").notNull().default(0),
    ciphertextSha256: text("ciphertext_sha256").notNull().default(""),
    ciphertextSizeBytes: integer("ciphertext_size_bytes").notNull().default(0),
    remoteObjectId: text("remote_object_id").notNull().default(""),
    objectKey: text("object_key").notNull().default(""),
    cacheId: text("cache_id").notNull().default(""),
    phase: text("phase").notNull().default("queued"),
    attemptCount: integer("attempt_count").notNull().default(0),
    nextAttemptAt: text("next_attempt_at").notNull().default(currentTimestamp),
    lastAttemptAt: text("last_attempt_at"),
    lastError: text("last_error").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    completedAt: text("completed_at"),
  },
  (table) => [
    uniqueIndex("idx_attachment_transfer_jobs_live_version")
      .on(table.attachmentId, table.expectedSha256, table.expectedSizeBytes)
      .where(
        sql`${table.direction} IN ('upload', 'download') AND ${table.phase} <> 'completed'`,
      ),
    uniqueIndex("idx_attachment_transfer_jobs_delete_object")
      .on(table.objectKey)
      .where(sql`${table.direction} = 'delete'`),
    uniqueIndex("idx_attachment_transfer_jobs_upload_object_id")
      .on(table.remoteObjectId)
      .where(
        sql`${table.direction} = 'upload' AND ${table.remoteObjectId} <> ''`,
      ),
    uniqueIndex("idx_attachment_transfer_jobs_delete_object_id")
      .on(table.remoteObjectId)
      .where(
        sql`${table.direction} = 'delete' AND ${table.remoteObjectId} <> ''`,
      ),
    uniqueIndex("idx_attachment_transfer_jobs_cache_id")
      .on(table.cacheId)
      .where(sql`${table.cacheId} <> ''`),
    index("idx_attachment_transfer_jobs_due").on(
      table.phase,
      table.nextAttemptAt,
      table.createdAt,
    ),
    index("idx_attachment_transfer_jobs_attachment_id").on(table.attachmentId),
    index("idx_attachment_transfer_jobs_session_id").on(table.sessionId),
  ],
);

export const sharedSessionAttachmentCache = sqliteTable(
  "shared_session_attachment_cache",
  {
    viewerUserId: text("viewer_user_id").notNull(),
    shareId: text("share_id").notNull(),
    attachmentId: text("attachment_id").notNull(),
    filename: text("filename").notNull().default(""),
    contentType: text("content_type")
      .notNull()
      .default("application/octet-stream"),
    sizeBytes: integer("size_bytes").notNull().default(0),
    sha256: text("sha256").notNull(),
    cacheId: text("cache_id").notNull().default(""),
    claimToken: text("claim_token").notNull().default(""),
    cacheGeneration: integer("cache_generation").notNull().default(0),
    availability: text("availability").notNull().default("pending"),
    accessVersion: integer("access_version").notNull().default(0),
    attemptCount: integer("attempt_count").notNull().default(0),
    nextAttemptAt: text("next_attempt_at").notNull().default(currentTimestamp),
    lastAttemptAt: text("last_attempt_at"),
    lastError: text("last_error").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
  },
  (table) => [
    primaryKey({
      columns: [table.viewerUserId, table.shareId, table.attachmentId],
    }),
    uniqueIndex("idx_shared_session_attachment_cache_cache_id")
      .on(table.cacheId)
      .where(sql`${table.cacheId} <> ''`),
    index("idx_shared_session_attachment_cache_cleanup")
      .on(table.availability, table.updatedAt)
      .where(
        sql`${table.availability} IN ('delete_pending', 'deleting', 'failed')`,
      ),
    index("idx_shared_session_attachment_cache_due")
      .on(table.availability, table.nextAttemptAt, table.updatedAt)
      .where(
        sql`${table.availability} IN ('pending', 'delete_pending', 'failed')`,
      ),
    index("idx_shared_session_attachment_cache_share").on(
      table.viewerUserId,
      table.shareId,
    ),
  ],
);

export const tags = sqliteTable(
  "tags",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    name: text("name").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [index("idx_tags_name").on(table.name)],
);

export const sessionTags = sqliteTable(
  "session_tags",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    sessionId: text("session_id").notNull().default(""),
    tagId: text("tag_id").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_session_tags_session_id").on(table.sessionId),
    index("idx_session_tags_tag_id").on(table.tagId),
  ],
);

export const entityMentions = sqliteTable(
  "entity_mentions",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    sourceType: text("source_type").notNull().default(""),
    sourceId: text("source_id").notNull().default(""),
    targetType: text("target_type").notNull().default(""),
    targetId: text("target_id").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_entity_mentions_source").on(table.sourceType, table.sourceId),
    index("idx_entity_mentions_target").on(table.targetType, table.targetId),
  ],
);

export const chatGroups = sqliteTable("chat_groups", {
  id: text("id").primaryKey().notNull(),
  workspaceId: text("workspace_id").notNull().default(""),
  ownerUserId: text("owner_user_id").notNull().default(""),
  title: text("title").notNull().default(""),
  createdAt: text("created_at").notNull().default(currentTimestamp),
  updatedAt: text("updated_at").notNull().default(currentTimestamp),
  deletedAt: text("deleted_at"),
});

export const chatMessages = sqliteTable(
  "chat_messages",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    chatGroupId: text("chat_group_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    role: text("role").notNull().default(""),
    content: text("content").notNull().default(""),
    metadataJson: text("metadata_json").notNull().default("{}"),
    partsJson: text("parts_json").notNull().default("[]"),
    status: text("status").notNull().default("ready"),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [
    index("idx_chat_messages_group_id").on(table.chatGroupId, table.createdAt),
  ],
);

export const dailyNotes = sqliteTable(
  "daily_notes",
  {
    id: text("id").primaryKey().notNull(),
    workspaceId: text("workspace_id").notNull().default(""),
    ownerUserId: text("owner_user_id").notNull().default(""),
    noteDate: text("note_date").notNull().default(""),
    bodyFormat: text("body_format").notNull().default("prosemirror_json"),
    body: text("body").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    updatedAt: text("updated_at").notNull().default(currentTimestamp),
    deletedAt: text("deleted_at"),
  },
  (table) => [index("idx_daily_notes_note_date").on(table.noteDate)],
);

export const appSettings = sqliteTable("app_settings", {
  id: text("id").primaryKey().notNull(),
  valueJson: text("value_json").notNull().default("null"),
  updatedAt: text("updated_at").notNull().default(currentTimestamp),
});

export const migrationImportRuns = sqliteTable("migration_import_runs", {
  id: text("id").primaryKey().notNull(),
  importerVersion: integer("importer_version").notNull().default(1),
  sourceRoot: text("source_root").notNull().default(""),
  dryRun: integer("dry_run", { mode: "boolean" }).notNull().default(false),
  status: text("status").notNull().default("running"),
  discoveredCount: integer("discovered_count").notNull().default(0),
  importedCount: integer("imported_count").notNull().default(0),
  matchedCount: integer("matched_count").notNull().default(0),
  skippedCount: integer("skipped_count").notNull().default(0),
  conflictCount: integer("conflict_count").notNull().default(0),
  errorCount: integer("error_count").notNull().default(0),
  startedAt: text("started_at").notNull().default(currentTimestamp),
  completedAt: text("completed_at"),
  error: text("error").notNull().default(""),
});

export const migrationImportItems = sqliteTable(
  "migration_import_items",
  {
    id: text("id").primaryKey().notNull(),
    runId: text("run_id").notNull().default(""),
    sourcePath: text("source_path").notNull().default(""),
    sourceKind: text("source_kind").notNull().default(""),
    sourceSha256: text("source_sha256").notNull().default(""),
    status: text("status").notNull().default("pending"),
    discoveredCount: integer("discovered_count").notNull().default(0),
    importedCount: integer("imported_count").notNull().default(0),
    matchedCount: integer("matched_count").notNull().default(0),
    skippedCount: integer("skipped_count").notNull().default(0),
    conflictCount: integer("conflict_count").notNull().default(0),
    error: text("error").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
    completedAt: text("completed_at"),
  },
  (table) => [
    index("idx_migration_import_items_run_id").on(table.runId),
    index("idx_migration_import_items_source").on(
      table.sourcePath,
      table.sourceSha256,
    ),
  ],
);

export const migrationImportTargets = sqliteTable(
  "migration_import_targets",
  {
    id: text("id").primaryKey().notNull(),
    runId: text("run_id").notNull().default(""),
    itemId: text("item_id").notNull().default(""),
    sourcePath: text("source_path").notNull().default(""),
    sourceKind: text("source_kind").notNull().default(""),
    tableName: text("table_name").notNull().default(""),
    targetId: text("target_id").notNull().default(""),
    status: text("status").notNull().default("pending"),
    error: text("error").notNull().default(""),
    createdAt: text("created_at").notNull().default(currentTimestamp),
  },
  (table) => [
    index("idx_migration_import_targets_run_id").on(table.runId),
    index("idx_migration_import_targets_table").on(
      table.runId,
      table.tableName,
      table.status,
    ),
    index("idx_migration_import_targets_source").on(
      table.sourcePath,
      table.targetId,
    ),
  ],
);

export const storageMigrationState = sqliteTable("storage_migration_state", {
  id: text("id").primaryKey().notNull(),
  importerVersion: integer("importer_version").notNull().default(1),
  phase: text("phase").notNull().default("shadow"),
  latestRunId: text("latest_run_id").notNull().default(""),
  parityVerified: integer("parity_verified", { mode: "boolean" })
    .notNull()
    .default(false),
  cutoverAt: text("cutover_at"),
  rollbackUntil: text("rollback_until"),
  lastError: text("last_error").notNull().default(""),
  updatedAt: text("updated_at").notNull().default(currentTimestamp),
});
