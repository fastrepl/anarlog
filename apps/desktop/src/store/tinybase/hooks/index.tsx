import { type ReactNode, useCallback, useMemo } from "react";

import type {
  EnhancedNoteStorage,
  HumanStorage,
  IgnoredEvent,
  IgnoredRecurringSeries,
  OrganizationStorage,
  SessionEvent,
  SessionStorage,
  TemplateStorage,
} from "@hypr/store";

import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";

export function useSession(sessionId: string) {
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  const folderId = main.UI.useCell(
    "sessions",
    sessionId,
    "folder_id",
    main.STORE_ID,
  );

  const event = useMemo(
    () => getSessionEvent({ event_json: eventJson }),
    [eventJson],
  );

  return useMemo(
    () => ({ title, rawMd, createdAt, event, folderId }),
    [title, rawMd, createdAt, event, folderId],
  );
}

export function useSessionEvent(sessionId: string): SessionEvent | null {
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  return useMemo(() => getSessionEvent({ event_json: eventJson }), [eventJson]);
}

export function useSetSessionTitle() {
  const store = main.UI.useStore(main.STORE_ID);

  return useCallback(
    (sessionId: string, title: string) => {
      if (!store) return;
      store.setPartialRow("sessions", sessionId, { title });
    },
    [store],
  );
}

export function useSetSessionRawMd() {
  const store = main.UI.useStore(main.STORE_ID);

  return useCallback(
    (sessionId: string, rawMd: string) => {
      if (!store) return;
      store.setPartialRow("sessions", sessionId, { raw_md: rawMd });
    },
    [store],
  );
}

export function useHuman(humanId: string) {
  const name = main.UI.useCell("humans", humanId, "name", main.STORE_ID);
  const email = main.UI.useCell("humans", humanId, "email", main.STORE_ID);
  const orgId = main.UI.useCell("humans", humanId, "org_id", main.STORE_ID);
  const jobTitle = main.UI.useCell(
    "humans",
    humanId,
    "job_title",
    main.STORE_ID,
  );
  const linkedinUsername = main.UI.useCell(
    "humans",
    humanId,
    "linkedin_username",
    main.STORE_ID,
  );

  return useMemo(
    () => ({ name, email, orgId, jobTitle, linkedinUsername }),
    [name, email, orgId, jobTitle, linkedinUsername],
  );
}

export function useOrganization(orgId: string) {
  const name = main.UI.useCell("organizations", orgId, "name", main.STORE_ID);

  return useMemo(() => ({ name }), [name]);
}

export function useEvent(eventId: string | undefined) {
  const title = main.UI.useCell(
    "events",
    eventId ?? "",
    "title",
    main.STORE_ID,
  );
  const startedAt = main.UI.useCell(
    "events",
    eventId ?? "",
    "started_at",
    main.STORE_ID,
  );
  const endedAt = main.UI.useCell(
    "events",
    eventId ?? "",
    "ended_at",
    main.STORE_ID,
  );
  const location = main.UI.useCell(
    "events",
    eventId ?? "",
    "location",
    main.STORE_ID,
  );
  const meetingLink = main.UI.useCell(
    "events",
    eventId ?? "",
    "meeting_link",
    main.STORE_ID,
  );
  const description = main.UI.useCell(
    "events",
    eventId ?? "",
    "description",
    main.STORE_ID,
  );
  const calendarId = main.UI.useCell(
    "events",
    eventId ?? "",
    "calendar_id",
    main.STORE_ID,
  );

  return useMemo(
    () =>
      eventId
        ? {
            title,
            startedAt,
            endedAt,
            location,
            meetingLink,
            description,
            calendarId,
          }
        : null,
    [
      eventId,
      title,
      startedAt,
      endedAt,
      location,
      meetingLink,
      description,
      calendarId,
    ],
  );
}

function parseIgnoredEvents(raw: string | undefined): IgnoredEvent[] {
  if (!raw) return [];
  try {
    return JSON.parse(raw) as IgnoredEvent[];
  } catch {
    return [];
  }
}

function parseIgnoredSeries(raw: string | undefined): IgnoredRecurringSeries[] {
  if (!raw) return [];
  try {
    return JSON.parse(raw) as IgnoredRecurringSeries[];
  } catch {
    return [];
  }
}

export function useIgnoredEvents() {
  const store = main.UI.useStore(main.STORE_ID);

  const ignoredEventsRaw = main.UI.useValue("ignored_events", main.STORE_ID) as
    | string
    | undefined;
  const ignoredSeriesRaw = main.UI.useValue(
    "ignored_recurring_series",
    main.STORE_ID,
  ) as string | undefined;

  const ignoredIds = useMemo(() => {
    const list = parseIgnoredEvents(ignoredEventsRaw);
    return new Set(list.map((e) => e.tracking_id));
  }, [ignoredEventsRaw]);

  const ignoredSeriesIds = useMemo(() => {
    const list = parseIgnoredSeries(ignoredSeriesRaw);
    return new Set(list.map((e) => e.id));
  }, [ignoredSeriesRaw]);

  const isIgnored = useCallback(
    (
      trackingId: string | null | undefined,
      recurrenceSeriesId: string | null | undefined,
    ) => {
      if (!trackingId) return false;
      if (ignoredIds.has(trackingId)) return true;
      if (recurrenceSeriesId && ignoredSeriesIds.has(recurrenceSeriesId))
        return true;
      return false;
    },
    [ignoredIds, ignoredSeriesIds],
  );

  const ignoreEvent = useCallback(
    (trackingId: string) => {
      if (!store) return;
      const list = parseIgnoredEvents(
        store.getValue("ignored_events") as string | undefined,
      );
      const now = new Date().toISOString();
      list.push({
        tracking_id: trackingId,
        last_seen: now,
      });
      store.setValue("ignored_events", JSON.stringify(list));
    },
    [store],
  );

  const unignoreEvent = useCallback(
    (trackingId: string) => {
      if (!store) return;
      const list = parseIgnoredEvents(
        store.getValue("ignored_events") as string | undefined,
      );
      const filtered = list.filter((e) => e.tracking_id !== trackingId);
      store.setValue("ignored_events", JSON.stringify(filtered));
    },
    [store],
  );

  const ignoreSeries = useCallback(
    (seriesId: string) => {
      if (!store) return;
      const list = parseIgnoredSeries(
        store.getValue("ignored_recurring_series") as string | undefined,
      );
      if (!list.some((e) => e.id === seriesId)) {
        list.push({ id: seriesId, last_seen: new Date().toISOString() });
        store.setValue("ignored_recurring_series", JSON.stringify(list));
      }
    },
    [store],
  );

  const unignoreSeries = useCallback(
    (seriesId: string) => {
      if (!store) return;
      const list = parseIgnoredSeries(
        store.getValue("ignored_recurring_series") as string | undefined,
      );
      store.setValue(
        "ignored_recurring_series",
        JSON.stringify(list.filter((e) => e.id !== seriesId)),
      );
    },
    [store],
  );

  return {
    isIgnored,
    ignoreEvent,
    unignoreEvent,
    ignoreSeries,
    unignoreSeries,
  };
}

export function useTemplate(templateId: string) {
  const title = main.UI.useCell(
    "templates",
    templateId,
    "title",
    main.STORE_ID,
  );
  const description = main.UI.useCell(
    "templates",
    templateId,
    "description",
    main.STORE_ID,
  );
  const sections = main.UI.useCell(
    "templates",
    templateId,
    "sections",
    main.STORE_ID,
  );
  return useMemo(
    () => ({ title, description, sections }),
    [title, description, sections],
  );
}

interface TinyBaseTestWrapperProps {
  children: ReactNode;
  initialData?: {
    sessions?: Record<string, Partial<SessionStorage>>;
    humans?: Record<string, Partial<HumanStorage>>;
    organizations?: Record<string, Partial<OrganizationStorage>>;
    templates?: Record<string, Partial<TemplateStorage>>;
    enhanced_notes?: Record<string, Partial<EnhancedNoteStorage>>;
  };
  initialValues?: {
    user_id?: string;
  };
}

export function TinyBaseTestWrapper({
  children,
  initialData,
  initialValues,
}: TinyBaseTestWrapperProps) {
  const {
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
  } = main.testUtils;

  const store = useCreateMergeableStore(() => {
    const s = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);

    if (initialValues?.user_id) {
      s.setValue("user_id", initialValues.user_id);
    }

    if (initialData?.sessions) {
      Object.entries(initialData.sessions).forEach(([id, data]) => {
        s.setRow("sessions", id, data as Record<string, unknown>);
      });
    }
    if (initialData?.humans) {
      Object.entries(initialData.humans).forEach(([id, data]) => {
        s.setRow("humans", id, data as Record<string, unknown>);
      });
    }
    if (initialData?.organizations) {
      Object.entries(initialData.organizations).forEach(([id, data]) => {
        s.setRow("organizations", id, data as Record<string, unknown>);
      });
    }
    if (initialData?.templates) {
      Object.entries(initialData.templates).forEach(([id, data]) => {
        s.setRow("templates", id, data as Record<string, unknown>);
      });
    }
    if (initialData?.enhanced_notes) {
      Object.entries(initialData.enhanced_notes).forEach(([id, data]) => {
        s.setRow("enhanced_notes", id, data as Record<string, unknown>);
      });
    }

    return s;
  });

  const indexes = useCreateIndexes(store, (store) =>
    createIndexes(store)
      .setIndexDefinition(
        main.INDEXES.sessionParticipantsBySession,
        "mapping_session_participant",
        "session_id",
      )
      .setIndexDefinition(
        main.INDEXES.sessionsByFolder,
        "sessions",
        "folder_id",
        "created_at",
      )
      .setIndexDefinition(
        main.INDEXES.transcriptBySession,
        "transcripts",
        "session_id",
        "created_at",
      )
      .setIndexDefinition(
        main.INDEXES.enhancedNotesBySession,
        "enhanced_notes",
        "session_id",
        "position",
      ),
  );

  const relationships = useCreateRelationships(store, (store) =>
    createRelationships(store).setRelationshipDefinition(
      main.RELATIONSHIPS.enhancedNoteToSession,
      "enhanced_notes",
      "sessions",
      "session_id",
    ),
  );

  const queries = useCreateQueries(store, (store) =>
    createQueries(store)
      .setQueryDefinition(
        main.QUERIES.visibleHumans,
        "humans",
        ({ select }) => {
          select("name");
          select("email");
          select("org_id");
          select("job_title");
          select("linkedin_username");
        },
      )
      .setQueryDefinition(
        main.QUERIES.visibleOrganizations,
        "organizations",
        ({ select }) => {
          select("name");
        },
      )
      .setQueryDefinition(
        main.QUERIES.visibleTemplates,
        "templates",
        ({ select }) => {
          select("title");
          select("description");
          select("sections");
        },
      ),
  );

  useProvideStore(main.STORE_ID, store);
  useProvideIndexes(main.STORE_ID, indexes!);
  useProvideRelationships(main.STORE_ID, relationships!);
  useProvideQueries(main.STORE_ID, queries!);

  return <>{children}</>;
}
