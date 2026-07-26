import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(),
  liveRows: [] as Array<Record<string, unknown>>,
  liveQueryOptions: null as null | {
    sql: string;
    params?: unknown[];
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: (options: {
    sql: string;
    params?: unknown[];
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => {
    mocks.liveQueryOptions = options;
    return { data: options.mapRows(mocks.liveRows) };
  },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (
    _key: string,
    write: () => Promise<unknown>,
  ): Promise<unknown> => write(),
}));

import {
  getCalendarEventStartedAt,
  getNearbyCalendarEvents,
  searchCalendarEvents,
  setCalendarEnabled,
  useCalendarRows,
} from "./queries";

describe("calendar SQLite selection", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.liveRows = [];
    mocks.liveQueryOptions = null;
    mocks.execute.mockResolvedValue([]);
    mocks.executeTransaction.mockResolvedValue([]);
  });

  test("reads provider calendars from the canonical table", () => {
    mocks.liveRows = [
      {
        id: "calendar-1",
        tracking_id_calendar: "primary",
        name: "Work",
        enabled: 1,
        provider: "google",
        source: "work@example.com",
        color: "#4285f4",
        connection_id: "connection-1",
        created_at: "2026-07-10T00:00:00.000Z",
      },
    ];

    const { result } = renderHook(() => useCalendarRows("google"));

    expect(mocks.liveQueryOptions?.params).toEqual(["google", "google"]);
    expect(result.current).toEqual([
      {
        ...mocks.liveRows[0],
        enabled: true,
      },
    ]);
  });

  test("disabling a calendar tombstones its events in the same transaction", async () => {
    await setCalendarEnabled("calendar-1", false);

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE calendars");
    expect(statements[0].params[0]).toBe(0);
    expect(statements[1].sql).toContain("UPDATE events");
    expect(statements[1].sql).toContain("deleted_at");
    expect(statements[1].params).toContain("calendar-1");
  });

  test("reads an event start time from SQLite", async () => {
    mocks.execute.mockResolvedValue([
      { started_at: "2026-07-10T09:00:00.000Z" },
    ]);

    await expect(getCalendarEventStartedAt("event-1")).resolves.toBe(
      "2026-07-10T09:00:00.000Z",
    );
    expect(mocks.execute.mock.calls[0][1]).toEqual(["event-1"]);
  });

  test("maps event search results and linked sessions", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "event-1",
        title: "Planning",
        started_at: "2026-07-10T09:00:00.000Z",
        ended_at: "",
        location: "Room 1",
        meeting_link: "",
        description: "Weekly plan",
        participant_count: 2,
        linked_session_id: "session-1",
      },
    ]);

    await expect(searchCalendarEvents(" Plan ", 5)).resolves.toEqual([
      {
        id: "event-1",
        title: "Planning",
        startedAt: "2026-07-10T09:00:00.000Z",
        endedAt: null,
        location: "Room 1",
        meetingLink: null,
        description: "Weekly plan",
        participantCount: 2,
        linkedSessionId: "session-1",
      },
    ]);
    expect(mocks.execute.mock.calls[0][1]).toEqual(["plan", "plan", 5]);
  });

  test("returns nearby event participant names without the current user", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "event-1",
        title: "Planning",
        started_at: "2026-07-10T09:00:00.000Z",
        meeting_link: "https://meet.example.com/planning",
        location: "Room 1",
        description: "Weekly plan",
        participants_json: JSON.stringify([
          { name: "Alice", is_current_user: false },
          { name: "John", is_current_user: true },
          { name: "Alice", is_current_user: false },
        ]),
      },
    ]);

    await expect(getNearbyCalendarEvents(1000, 500)).resolves.toEqual([
      {
        id: "event-1",
        title: "Planning",
        meetingLink: "https://meet.example.com/planning",
        location: "Room 1",
        description: "Weekly plan",
        participantNames: ["Alice"],
      },
    ]);
    expect(mocks.execute.mock.calls[0][1]).toEqual([1000, 500, 1000]);
  });
});
