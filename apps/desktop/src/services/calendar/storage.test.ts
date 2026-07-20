import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(),
  id: vi.fn(),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/shared/utils", () => ({
  DEFAULT_USER_ID: "default-user",
  id: mocks.id,
}));

import { syncEvents } from "./process/events";
import {
  applyCalendarInventory,
  applyConnectionSync,
  loadEventsForSync,
  tombstoneCalendarConnection,
} from "./storage";

const calendar = {
  id: "cal-work",
  tracking_id_calendar: "primary",
  name: "Work",
  enabled: 1,
  provider: "google",
  source: "work@example.com",
  color: "#4285f4",
  connection_id: "conn-work",
  created_at: "2026-01-01T00:00:00.000Z",
  deleted_at: null,
};

describe("calendar SQLite storage", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.execute.mockResolvedValue([]);
    mocks.executeTransaction.mockResolvedValue([]);
    mocks.id.mockReturnValue("generated-id");
  });

  test("soft-deletes a disconnected calendar and its events atomically", async () => {
    mocks.execute.mockResolvedValue([calendar]);

    await applyCalendarInventory({
      provider: "google",
      requestedConnectionIds: [],
      successfulConnections: [],
    });

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE calendars");
    expect(statements[0].sql).toContain("deleted_at");
    expect(statements[0].params).toContain("cal-work");
    expect(statements[1].sql).toContain("UPDATE events");
    expect(statements[1].params).toContain("cal-work");
    expect(
      statements.every((statement) => !statement.sql.includes("DELETE")),
    ).toBe(true);
  });

  test("tombstones only the disconnected provider connection", async () => {
    await tombstoneCalendarConnection("google", "conn-personal");

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE events");
    expect(statements[0].sql).toContain("SELECT id");
    expect(statements[0].params.slice(2)).toEqual(["google", "conn-personal"]);
    expect(statements[1].sql).toContain("UPDATE calendars");
    expect(statements[1].params.slice(2)).toEqual(["google", "conn-personal"]);
  });

  test("preserves calendars when a requested connection fails to refresh", async () => {
    mocks.execute.mockResolvedValue([calendar]);

    await applyCalendarInventory({
      provider: "google",
      requestedConnectionIds: ["conn-work"],
      successfulConnections: [],
    });

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  test("resurrects a calendar with its durable id and disables it", async () => {
    mocks.execute.mockResolvedValue([
      { ...calendar, enabled: 1, deleted_at: "2026-06-01T00:00:00.000Z" },
    ]);

    await applyCalendarInventory({
      provider: "google",
      requestedConnectionIds: ["conn-work"],
      successfulConnections: [
        {
          connectionId: "conn-work",
          calendars: [
            {
              provider: "google",
              id: "primary",
              title: "Work restored",
              source: "work@example.com",
              color: null,
              is_primary: true,
              can_edit: true,
              raw: "{}",
            },
          ],
        },
      ],
    });

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("ON CONFLICT(id) DO UPDATE");
    expect(statement.sql).toContain("WHEN calendars.deleted_at IS NULL");
    expect(statement.params[0]).toBe("cal-work");
    expect(statement.params).toContain("Work restored");
  });

  test("loads tombstoned matching events for durable-id resurrection", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "event-1",
        tracking_id_event: "tracking-1",
        calendar_id: "cal-work",
        title: "Meeting",
        started_at: "2026-06-01T10:00:00.000Z",
        ended_at: "2026-06-01T11:00:00.000Z",
        location: "",
        meeting_link: "",
        description: "",
        note: "",
        recurrence_series_id: "",
        has_recurrence_rules: 0,
        is_all_day: 0,
        provider: "google",
        created_at: "2026-01-01T00:00:00.000Z",
        deleted_at: "2026-05-01T00:00:00.000Z",
      },
    ]);

    const rows = await loadEventsForSync(
      {
        provider: "google",
        connectionId: "conn-work",
        from: new Date("2026-06-01T00:00:00.000Z"),
        to: new Date("2026-06-02T00:00:00.000Z"),
        calendarIds: new Set(["cal-work"]),
        calendarTrackingIdToId: new Map([["primary", "cal-work"]]),
      },
      ["tracking-1"],
    );

    expect(rows[0]).toMatchObject({
      id: "event-1",
      has_recurrence_rules: false,
      is_all_day: false,
      deleted_at: "2026-05-01T00:00:00.000Z",
    });
    expect(mocks.execute.mock.calls[0][0]).toContain(
      "tracking_id_event IN (?)",
    );
  });

  test("commits event, session, human, and participant writes together", async () => {
    await applyConnectionSync({
      ctx: {
        provider: "google",
        connectionId: "conn-work",
        from: new Date("2026-06-01T00:00:00.000Z"),
        to: new Date("2026-06-02T00:00:00.000Z"),
        calendarIds: new Set(["cal-work"]),
        calendarTrackingIdToId: new Map([["primary", "cal-work"]]),
      },
      events: {
        toDelete: ["event-old"],
        toUpdate: [],
        toAdd: [
          {
            tracking_id_event: "tracking-1",
            tracking_id_calendar: "primary",
            title: "Meeting",
            has_recurrence_rules: false,
            is_all_day: false,
            participants: [{ email: "alice@example.com" }],
          },
        ],
      },
      sessionUpdates: [
        {
          sessionId: "session-1",
          trackingId: "tracking-1",
          calendarId: "cal-work",
          seriesId: "",
          eventJson: "{}",
        },
      ],
      participants: {
        humansToCreate: [
          {
            id: "human-1",
            ownerUserId: "",
            name: "Alice",
            email: "alice@example.com",
          },
        ],
        toDelete: ["mapping-old"],
        toAdd: [
          {
            sessionId: "session-1",
            humanId: "human-1",
            email: "alice@example.com",
          },
        ],
      },
    });

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
    }>;
    const sql = statements.map((statement) => statement.sql).join("\n");
    expect(sql).toContain("UPDATE events");
    expect(sql).toContain("INSERT INTO events");
    expect(sql).toContain("UPDATE sessions");
    expect(sql).toContain("INSERT INTO humans");
    expect(sql).toContain("cloudsync_workspace_binding");
    expect(sql).toContain("NULLIF((");
    expect(sql).not.toContain("COALESCE((");
    expect(sql).toContain("NULLIF(NULLIF(?, ''), 'default-user')");
    expect(sql).toContain("UPDATE session_participants");
    expect(sql).toContain("INSERT INTO session_participants");
    expect(sql).toContain("session.workspace_id");
    expect(sql).not.toContain("DELETE FROM");
  });

  test("normalizes missing optional fields when updating events", async () => {
    const ctx = {
      provider: "google" as const,
      connectionId: "conn-work",
      from: new Date("2026-06-01T00:00:00.000Z"),
      to: new Date("2026-06-02T00:00:00.000Z"),
      calendarIds: new Set(["cal-work"]),
      calendarTrackingIdToId: new Map([["primary", "cal-work"]]),
    };
    const events = syncEvents(ctx, {
      incoming: [
        {
          tracking_id_event: "tracking-1",
          tracking_id_calendar: "primary",
          title: "Updated meeting",
          started_at: "2026-06-01T10:00:00.000Z",
          ended_at: "2026-06-01T11:00:00.000Z",
          location: undefined,
          meeting_link: undefined,
          description: undefined,
          recurrence_series_id: undefined,
          has_recurrence_rules: false,
          is_all_day: false,
        },
      ],
      existing: [
        {
          id: "event-1",
          tracking_id_event: "tracking-1",
          calendar_id: "cal-work",
          title: "Meeting",
          started_at: "2026-06-01T10:00:00.000Z",
          ended_at: "2026-06-01T11:00:00.000Z",
          location: "Room 1",
          meeting_link: "https://meet.example.com/room",
          description: "Description",
          note: "",
          recurrence_series_id: "series-1",
          has_recurrence_rules: false,
          is_all_day: false,
          provider: "google",
          created_at: "2026-01-01T00:00:00.000Z",
          deleted_at: null,
        },
      ],
      incomingParticipants: new Map(),
    });

    await applyConnectionSync({
      ctx,
      events,
      sessionUpdates: [],
      participants: { humansToCreate: [], toDelete: [], toAdd: [] },
    });

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    const update = statements.find((statement) =>
      statement.sql.includes("UPDATE events"),
    );
    expect(update?.params.slice(5, 9)).toEqual(["", "", "", ""]);
  });
});
