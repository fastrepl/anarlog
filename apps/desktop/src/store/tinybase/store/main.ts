import { createBroadcastChannelSynchronizer } from "tinybase/synchronizers/synchronizer-broadcast-channel/with-schemas";
import * as _UI from "tinybase/ui-react/with-schemas";
import {
  createCheckpoints,
  createIndexes,
  createMergeableStore,
  createMetrics,
  createQueries,
  createRelationships,
  type MergeableStore,
} from "tinybase/with-schemas";

import { SCHEMA, type Schemas } from "@hypr/store";
import { format } from "@hypr/utils";

import { useMainPersisters } from "./persisters";

import { getSessionEvent } from "~/session/utils";

export const STORE_ID = "main";

export const TABLES = Object.keys(
  SCHEMA.table,
) as (keyof typeof SCHEMA.table)[];

const {
  useCreateMergeableStore,
  useCreateSynchronizer,
  useCreateRelationships,
  useCreateQueries,
  useProvideStore,
  useProvideIndexes,
  useProvideRelationships,
  useProvideMetrics,
  useCreateIndexes,
  useCreateMetrics,
  useProvideQueries,
  useProvideSynchronizer,
  useCreateCheckpoints,
  useProvideCheckpoints,
} = _UI as _UI.WithSchemas<Schemas>;

export const UI = _UI as TypedUI;
export type Store = MergeableStore<Schemas>;
export type { Schemas };

export const testUtils = {
  useCreateMergeableStore,
  useProvideStore,
  useProvideIndexes,
  useProvideRelationships,
  useProvideQueries,
  useCreateIndexes,
  useCreateRelationships,
  useCreateQueries,
  createMergeableStore,
  createIndexes,
  createQueries,
  createRelationships,
  SCHEMA,
};

export const StoreComponent = () => {
  const store = useCreateMergeableStore(() =>
    createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value),
  );

  useMainPersisters(store as Store);

  const synchronizer = useCreateSynchronizer(store, async (store) =>
    createBroadcastChannelSynchronizer(
      store,
      "hypr-sync-persisted",
    ).startSync(),
  );

  const relationships = useCreateRelationships(
    store,
    (store) =>
      createRelationships(store).setRelationshipDefinition(
        RELATIONSHIPS.enhancedNoteToSession,
        "enhanced_notes",
        "sessions",
        "session_id",
      ),
    [],
  )!;

  const queries = useCreateQueries(
    store,
    (store) =>
      createQueries(store)
        .setQueryDefinition(QUERIES.timelineEvents, "events", ({ select }) => {
          select("title");
          select("started_at");
          select("ended_at");
          select("calendar_id");
          select("tracking_id_event");
          select("has_recurrence_rules");
          select("recurrence_series_id");
          select("is_all_day");
        })
        .setQueryDefinition(
          QUERIES.timelineSessions,
          "sessions",
          ({ select }) => {
            select("title");
            select("created_at");
            select("event_json");
            select("folder_id");
          },
        )
        .setQueryDefinition(QUERIES.visibleHumans, "humans", ({ select }) => {
          select("created_at");
          select("name");
          select("email");
          select("org_id");
          select("job_title");
          select("linkedin_username");
          select("pinned");
          select("pin_order");
        })
        .setQueryDefinition(
          QUERIES.visibleOrganizations,
          "organizations",
          ({ select }) => {
            select("created_at");
            select("name");
            select("pinned");
            select("pin_order");
          },
        )
        .setQueryDefinition(
          QUERIES.visibleTemplates,
          "templates",
          ({ select }) => {
            select("title");
            select("description");
            select("sections");
          },
        )
        .setQueryDefinition(
          QUERIES.visibleChatShortcuts,
          "chat_shortcuts",
          ({ select }) => {
            select("user_id");
            select("title");
            select("content");
          },
        )
        .setQueryDefinition(
          QUERIES.sessionParticipantsWithDetails,
          "mapping_session_participant",
          ({ select, join }) => {
            select("session_id");
            select("human_id");

            join("humans", "human_id").as("human");
            select("human", "name").as("human_name");
            select("human", "email").as("human_email");
            select("human", "job_title").as("human_job_title");
            select("human", "linkedin_username").as("human_linkedin_username");
            select("human", "org_id").as("org_id");

            join("organizations", "human", "org_id").as("org");
            select("org", "name").as("org_name");
          },
        )
        .setQueryDefinition(
          QUERIES.sessionRecordingTimes,
          "transcripts",
          ({ select, group }) => {
            select("session_id");
            select("started_at");
            select("ended_at");

            group("started_at", "min").as("min_started_at");
            group("ended_at", "max").as("max_ended_at");
          },
        )
        .setQueryDefinition(
          QUERIES.enabledCalendars,
          "calendars",
          ({ select, where }) => {
            select("provider");
            where("enabled", true);
          },
        )
        .setQueryDefinition(
          QUERIES.userTemplates,
          "templates",
          ({ select, where, param }) => {
            select("title");
            select("description");
            select("sections");
            select("user_id");
            where("user_id", (param("user_id") as string) ?? "");
          },
          { user_id: "" },
        )
        .setQueryDefinition(
          QUERIES.visibleVocabs,
          "memories",
          ({ select, where }) => {
            select("text");
            select("type");
            select("created_at");
            where("type", "vocab");
          },
        ),
    [],
  )!;

  const indexes = useCreateIndexes(store, (store) =>
    createIndexes(store)
      .setIndexDefinition(INDEXES.humansByOrg, "humans", "org_id", "name")
      .setIndexDefinition(INDEXES.humansByEmail, "humans", "email")
      .setIndexDefinition(
        INDEXES.sessionParticipantsBySession,
        "mapping_session_participant",
        "session_id",
      )
      .setIndexDefinition(
        INDEXES.sessionsByHuman,
        "mapping_session_participant",
        "human_id",
      )
      .setIndexDefinition(
        INDEXES.sessionsByFolder,
        "sessions",
        "folder_id",
        "created_at",
      )
      .setIndexDefinition(
        INDEXES.transcriptBySession,
        "transcripts",
        "session_id",
        "created_at",
      )
      .setIndexDefinition(
        INDEXES.eventsByDate,
        "events",
        (getCell) => {
          const cell = getCell("started_at");
          if (!cell) {
            return "";
          }

          const d = new Date(cell);
          if (isNaN(d.getTime())) {
            return "";
          }

          return format(d, "yyyy-MM-dd");
        },
        "started_at",
        (a, b) => a.localeCompare(b),
        (a, b) => String(a).localeCompare(String(b)),
      )
      .setIndexDefinition(
        INDEXES.sessionByDateWithoutEvent,
        "sessions",
        (getCell) => {
          if (getCell("event_json")) {
            return "";
          }

          const cell = getCell("created_at");
          if (!cell) {
            return "";
          }

          const d = new Date(cell);
          if (isNaN(d.getTime())) {
            return "";
          }

          return format(d, "yyyy-MM-dd");
        },
        "created_at",
        (a, b) => a.localeCompare(b),
        (a, b) => String(a).localeCompare(String(b)),
      )
      .setIndexDefinition(
        INDEXES.sessionsByEventTrackingId,
        "sessions",
        (getCell) => {
          const eventJson = getCell("event_json") as string | undefined;
          if (!eventJson) return "";
          return getSessionEvent({ event_json: eventJson })?.tracking_id || "";
        },
      )
      .setIndexDefinition(
        INDEXES.tagSessionsBySession,
        "mapping_tag_session",
        "session_id",
      )
      .setIndexDefinition(
        INDEXES.chatMessagesByGroup,
        "chat_messages",
        "chat_group_id",
        "created_at",
      )
      .setIndexDefinition(
        INDEXES.enhancedNotesBySession,
        "enhanced_notes",
        "session_id",
        "position",
      )
      .setIndexDefinition(
        INDEXES.enhancedNotesByTemplate,
        "enhanced_notes",
        "template_id",
        "position",
      )
      .setIndexDefinition(
        INDEXES.mentionsBySource,
        "mapping_mention",
        "source_id",
      )
      .setIndexDefinition(
        INDEXES.mentionsByTarget,
        "mapping_mention",
        "target_id",
      ),
  );

  const metrics = useCreateMetrics(store, (store) =>
    createMetrics(store)
      .setMetricDefinition(METRICS.totalHumans, "humans", "sum", () => 1)
      .setMetricDefinition(
        METRICS.totalOrganizations,
        "organizations",
        "sum",
        () => 1,
      ),
  );

  const checkpoints = useCreateCheckpoints(store, (store) =>
    createCheckpoints(store),
  );

  useProvideStore(STORE_ID, store);
  useProvideRelationships(STORE_ID, relationships);
  useProvideQueries(STORE_ID, queries!);
  useProvideIndexes(STORE_ID, indexes!);
  useProvideMetrics(STORE_ID, metrics!);
  useProvideSynchronizer(STORE_ID, synchronizer);
  useProvideCheckpoints(STORE_ID, checkpoints!);

  return null;
};

export const rowIdOfChange = (table: string, row: string) => `${table}:${row}`;

export const QUERIES = {
  timelineEvents: "timelineEvents",
  timelineSessions: "timelineSessions",
  visibleOrganizations: "visibleOrganizations",
  visibleHumans: "visibleHumans",
  visibleTemplates: "visibleTemplates",
  visibleChatShortcuts: "visibleChatShortcuts",
  visibleVocabs: "visibleVocabs",
  sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  sessionRecordingTimes: "sessionRecordingTimes",
  enabledCalendars: "enabledCalendars",
  userTemplates: "userTemplates",
} as const;

export const METRICS = {
  totalHumans: "totalHumans",
  totalOrganizations: "totalOrganizations",
} as const;

export const INDEXES = {
  humansByOrg: "humansByOrg",
  humansByEmail: "humansByEmail",
  sessionParticipantsBySession: "sessionParticipantsBySession",
  sessionsByFolder: "sessionsByFolder",
  transcriptBySession: "transcriptBySession",
  eventsByDate: "eventsByDate",
  sessionByDateWithoutEvent: "sessionByDateWithoutEvent",
  sessionsByEventTrackingId: "sessionsByEventTrackingId",
  tagSessionsBySession: "tagSessionsBySession",
  chatMessagesByGroup: "chatMessagesByGroup",
  sessionsByHuman: "sessionsByHuman",
  enhancedNotesBySession: "enhancedNotesBySession",
  enhancedNotesByTemplate: "enhancedNotesByTemplate",
  mentionsBySource: "mentionsBySource",
  mentionsByTarget: "mentionsByTarget",
} as const;

export const RELATIONSHIPS = {
  enhancedNoteToSession: "enhancedNoteToSession",
} as const;

type QueryId = (typeof QUERIES)[keyof typeof QUERIES];

interface _QueryResultRows {
  timelineEvents: {
    title: string;
    started_at: string;
    ended_at: string;
    calendar_id: string;
    tracking_id_event: string;
    has_recurrence_rules: boolean;
    recurrence_series_id: string;
    is_all_day: boolean;
  };
  timelineSessions: {
    title: string;
    created_at: string;
    event_json: string;
    folder_id: string;
  };
  visibleHumans: {
    created_at: string;
    name: string;
    email: string;
    org_id: string;
    job_title: string;
    linkedin_username: string;
    pinned: boolean;
    pin_order: number;
  };
  visibleOrganizations: {
    created_at: string;
    name: string;
    pinned: boolean;
    pin_order: number;
  };
  visibleTemplates: {
    title: string;
    description: string;
    sections: string;
  };
  visibleChatShortcuts: {
    user_id: string;
    title: string;
    content: string;
  };
  sessionParticipantsWithDetails: {
    session_id: string;
    human_id: string;
    human_name?: string;
    human_email?: string;
    human_job_title?: string;
    human_linkedin_username?: string;
    org_id?: string;
    org_name?: string;
  };
  sessionRecordingTimes: {
    session_id: string;
    min_started_at: number;
    max_ended_at: number;
  };
  enabledCalendars: {
    provider: string;
  };
  userTemplates: {
    title: string;
    description: string;
    sections: string;
    user_id: string;
  };
  visibleVocabs: {
    text: string;
    type: string;
    created_at: string;
  };
}

export type QueryResultRowMap = { [K in QueryId]: _QueryResultRows[K] };

type QueriesOrQueriesId = _UI.WithSchemas<Schemas>["QueriesOrQueriesId"];

type TypedUI = Omit<
  _UI.WithSchemas<Schemas>,
  "useResultTable" | "useResultRow"
> & {
  useResultTable: <Q extends QueryId>(
    queryId: Q,
    queriesOrQueriesId?: QueriesOrQueriesId,
  ) => Record<string, QueryResultRowMap[Q]>;
  useResultRow: <Q extends QueryId>(
    queryId: Q,
    rowId: string,
    queriesOrQueriesId?: QueriesOrQueriesId,
  ) => QueryResultRowMap[Q];
};
