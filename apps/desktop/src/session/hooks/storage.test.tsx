import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { useCellMock, useSliceRowIdsMock, useTableMock, useResultTableMock } =
  vi.hoisted(() => ({
    useCellMock: vi.fn(),
    useSliceRowIdsMock: vi.fn<() => string[]>(() => []),
    useTableMock: vi.fn(() => ({})),
    useResultTableMock: vi.fn(() => ({})),
  }));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    transcriptBySession: "transcriptBySession",
    enhancedNotesBySession: "enhancedNotesBySession",
    sessionParticipantsBySession: "sessionParticipantsBySession",
  },
  QUERIES: {
    sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  },
  UI: {
    useCell: useCellMock,
    useSliceRowIds: useSliceRowIdsMock,
    useTable: useTableMock,
    useResultTable: useResultTableMock,
    useStore: vi.fn(() => null),
    useIndexes: vi.fn(() => null),
    useValue: vi.fn(() => undefined),
    useRowIds: vi.fn(() => []),
    useResultRow: vi.fn(() => ({})),
    useSetPartialRowCallback: vi.fn(),
    useSetCellCallback: vi.fn(),
    useQueries: vi.fn(() => null),
  },
}));

import {
  useSearchableHumans,
  useSessionCell,
  useSessionCellOptional,
  useSessionParticipantNames,
  useSessionSearchTimestampLookup,
} from "./storage";

describe("session storage boundary hooks", () => {
  beforeEach(() => {
    useCellMock.mockReset();
    useSliceRowIdsMock.mockReset();
    useTableMock.mockReset();
    useResultTableMock.mockReset();
  });

  it("preserves optional session cell semantics for missing values", () => {
    useCellMock.mockReturnValue(undefined);

    const optionalResult = renderHook(() =>
      useSessionCellOptional("session-1", "created_at"),
    );
    const normalizedResult = renderHook(() =>
      useSessionCell("session-1", "created_at"),
    );

    expect(optionalResult.result.current).toBeUndefined();
    expect(normalizedResult.result.current).toBe("");
  });

  it("returns participant names for non-excluded mappings", () => {
    useSliceRowIdsMock.mockReturnValue(["map-1", "map-2"]);
    useTableMock.mockReturnValue({
      "map-1": { source: "manual" },
      "map-2": { source: "excluded" },
    });
    useResultTableMock.mockReturnValue({
      "map-1": {
        human_id: "human-1",
        session_id: "session-1",
        human_name: "Alice",
      },
      "map-2": {
        human_id: "human-2",
        session_id: "session-1",
        human_name: "Bob",
      },
    });

    const result = renderHook(() => useSessionParticipantNames("session-1"));
    expect(result.result.current).toEqual(["Alice"]);
  });

  it("filters humans by search text and excluded ids", () => {
    useTableMock.mockReturnValue({
      "human-1": { name: "Alice", email: "alice@acme.com" },
      "human-2": { name: "Bob", email: "bob@acme.com" },
    });

    const excluded = new Set<string>(["human-2"]);
    const result = renderHook(() => useSearchableHumans("ali", excluded));

    expect(result.result.current).toEqual([
      {
        id: "human-1",
        name: "Alice",
        email: "alice@acme.com",
        orgId: undefined,
        jobTitle: undefined,
      },
    ]);
  });

  it("prefers event start timestamps and falls back to created_at", () => {
    useTableMock.mockReturnValue({
      "session-event": {
        created_at: "2024-01-01T00:00:00Z",
        event_json: JSON.stringify({
          started_at: "2024-01-15T10:00:00Z",
        }),
      },
      "session-plain": {
        created_at: "2024-02-01T00:00:00Z",
      },
      "session-invalid-event": {
        created_at: "2024-03-01T00:00:00Z",
        event_json: JSON.stringify({
          started_at: "not-a-date",
        }),
      },
    });

    const result = renderHook(() => useSessionSearchTimestampLookup());

    expect(result.result.current("session-event")).toBe(
      Date.parse("2024-01-15T10:00:00Z"),
    );
    expect(result.result.current("session-plain")).toBe(
      Date.parse("2024-02-01T00:00:00Z"),
    );
    expect(result.result.current("session-invalid-event")).toBe(
      Date.parse("2024-03-01T00:00:00Z"),
    );
  });
});
