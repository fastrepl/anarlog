import { act, renderHook, waitFor } from "@testing-library/react";
import { createRequire } from "node:module";
import { expect, it, vi } from "vitest";

import { useSessionEvent } from "./useSessionEvent";

import { useTimelineEventsTable } from "~/calendar/queries";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  subscribe: vi.fn(),
}));

vi.mock("~/db", async () => {
  const { createUseLiveQuery } = await import("@anlg/db-react");
  return {
    executeTransaction: vi.fn(),
    liveQueryClient: mocks,
    useLiveQuery: createUseLiveQuery(mocks),
  };
});

vi.mock("~/session/queries", async () => import("../queries/sessions"));

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");

it("refreshes the open note and sidebar together when a calendar event is rescheduled", async () => {
  const database = new DatabaseSync(":memory:");
  const listeners = new Set<() => void>();
  const savedEvent = {
    tracking_id: "external-event-1",
    calendar_id: "calendar-1",
    title: "Planning",
    started_at: "2026-10-02T09:00:00.000Z",
    ended_at: "2026-10-02T10:00:00.000Z",
    is_all_day: false,
    has_recurrence_rules: false,
  };

  database.exec(`
    CREATE TABLE sessions (
      id TEXT, owner_user_id TEXT, created_at TEXT, folder_path TEXT,
      event_id TEXT, event_json TEXT, title TEXT, locked INTEGER, deleted_at TEXT
    );
    CREATE TABLE session_documents (
      id TEXT, kind TEXT, body TEXT, body_format TEXT, template_id TEXT, deleted_at TEXT
    );
    CREATE TABLE calendars (id TEXT, color TEXT, deleted_at TEXT);
    CREATE TABLE events (
      id TEXT, tracking_id_event TEXT, calendar_id TEXT, title TEXT,
      started_at TEXT, ended_at TEXT, is_all_day INTEGER, has_recurrence_rules INTEGER,
      recurrence_series_id TEXT, location TEXT, meeting_link TEXT, description TEXT,
      participants_json TEXT, attendance_json TEXT, deleted_at TEXT
    );
    INSERT INTO events VALUES (
      'event-1', 'external-event-1', 'calendar-1', 'Planning',
      '2026-10-02T09:00:00.000Z', '2026-10-02T10:00:00.000Z', 0, 0,
      '', '', '', '', '[]', '{}', NULL
    );
  `);
  database
    .prepare("INSERT INTO sessions VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)")
    .run(
      "session-1",
      "user-1",
      "2026-10-01",
      "",
      "event-1",
      JSON.stringify(savedEvent),
      "My note",
      0,
    );

  mocks.subscribe.mockImplementation(
    async (
      sql: string,
      params: string[],
      callbacks: { onData: (rows: unknown[]) => void },
    ) => {
      const emit = () => callbacks.onData(database.prepare(sql).all(...params));
      if (sql.includes("FROM events") || sql.includes("JOIN events")) {
        listeners.add(emit);
      }
      emit();
      return async () => {
        listeners.delete(emit);
      };
    },
  );

  const { result, unmount } = renderHook(() => ({
    event: useSessionEvent("session-1"),
    sidebar: useTimelineEventsTable(),
  }));

  try {
    await waitFor(() =>
      expect(result.current.event?.started_at).toBe(savedEvent.started_at),
    );
    database.exec(`
      UPDATE events SET started_at = '2026-10-02T11:00:00.000Z',
        ended_at = '2026-10-02T12:00:00.000Z' WHERE id = 'event-1';
    `);
    act(() => listeners.forEach((emit) => emit()));

    expect(result.current.sidebar["event-1"].started_at).toBe(
      "2026-10-02T11:00:00.000Z",
    );
    expect(result.current.event).toMatchObject({
      ...savedEvent,
      started_at: "2026-10-02T11:00:00.000Z",
      ended_at: "2026-10-02T12:00:00.000Z",
    });

    database.exec(
      "UPDATE events SET deleted_at = '2026-10-02' WHERE id = 'event-1'",
    );
    act(() => listeners.forEach((emit) => emit()));
    expect(result.current.event).toEqual(savedEvent);
  } finally {
    unmount();
    database.close();
  }
});
