import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

type BoundaryFixture = {
  tables: Record<string, Record<string, Record<string, unknown>>>;
  results: Record<string, Record<string, Record<string, unknown>>>;
  slices: Record<string, Record<string, string[]>>;
};

const { fixture, resetFixture, setFixture } = vi.hoisted(() => {
  const fixture: BoundaryFixture = {
    tables: {},
    results: {},
    slices: {},
  };

  return {
    fixture,
    resetFixture: () => {
      fixture.tables = {};
      fixture.results = {};
      fixture.slices = {};
    },
    setFixture: (next: Partial<BoundaryFixture>) => {
      fixture.tables = next.tables ?? {};
      fixture.results = next.results ?? {};
      fixture.slices = next.slices ?? {};
    },
  };
});

vi.mock("~/store/tinybase/store/main", () => {
  const mockStore = {
    getRow: (table: string, rowId: string) =>
      fixture.tables[table]?.[rowId] ?? {},
    hasRow: (table: string, rowId: string) =>
      Boolean(fixture.tables[table]?.[rowId]),
    getCell: (table: string, rowId: string, cell: string) =>
      fixture.tables[table]?.[rowId]?.[cell],
    forEachRow: (
      table: string,
      cb: (rowId: string, forEachCell: unknown) => void,
    ) => {
      for (const rowId of Object.keys(fixture.tables[table] ?? {})) {
        cb(rowId, () => {});
      }
    },
  };

  return {
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
      useCell: vi.fn(
        (table: string, rowId: string, cell: string) =>
          fixture.tables[table]?.[rowId]?.[cell],
      ),
      useSliceRowIds: vi.fn(
        (index: string, sliceId: string) =>
          fixture.slices[index]?.[sliceId] ?? [],
      ),
      useTable: vi.fn((table: string) => fixture.tables[table] ?? {}),
      useResultTable: vi.fn((query: string) => fixture.results[query] ?? {}),
      useStore: vi.fn(() => mockStore),
      useIndexes: vi.fn(() => null),
      useValue: vi.fn(() => undefined),
      useRowIds: vi.fn((table: string) =>
        Object.keys(fixture.tables[table] ?? {}),
      ),
      useResultRow: vi.fn(() => ({})),
      useSetPartialRowCallback: vi.fn(),
      useSetCellCallback: vi.fn(),
      useQueries: vi.fn(() => null),
    },
  };
});

import {
  useSessionParticipantNames,
  useSearchableHumans,
} from "./participants";
import { useSessionSearchTimestampLookup } from "./search";
import { useSessionCell, useSessionCellOptional } from "./sessions";

describe("session hook boundaries", () => {
  beforeEach(() => {
    resetFixture();
  });

  it("preserves optional session cell semantics for missing values", () => {
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
    setFixture({
      tables: {
        mapping_session_participant: {
          "map-1": { source: "manual" },
          "map-2": { source: "excluded" },
        },
      },
      results: {
        sessionParticipantsWithDetails: {
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
        },
      },
      slices: {
        sessionParticipantsBySession: {
          "session-1": ["map-1", "map-2"],
        },
      },
    });

    const result = renderHook(() => useSessionParticipantNames("session-1"));
    expect(result.result.current).toEqual(["Alice"]);
  });

  it("filters humans by search text and excluded ids", () => {
    setFixture({
      tables: {
        humans: {
          "human-1": { name: "Alice", email: "alice@acme.com" },
          "human-2": { name: "Bob", email: "bob@acme.com" },
        },
      },
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
    setFixture({
      tables: {
        sessions: {
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
        },
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
