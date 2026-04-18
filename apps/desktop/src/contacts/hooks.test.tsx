import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

type ContactsFixture = {
  tables: Record<string, Record<string, Record<string, unknown>>>;
  slices: Record<string, Record<string, string[]>>;
};

const { fixture, resetFixture, setFixture } = vi.hoisted(() => {
  const fixture: ContactsFixture = {
    tables: {},
    slices: {},
  };

  return {
    fixture,
    resetFixture: () => {
      fixture.tables = {};
      fixture.slices = {};
    },
    setFixture: (next: Partial<ContactsFixture>) => {
      fixture.tables = next.tables ?? {};
      fixture.slices = next.slices ?? {};
    },
  };
});

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    humansByOrg: "humansByOrg",
  },
  QUERIES: {
    visibleOrganizations: "visibleOrganizations",
    visibleHumans: "visibleHumans",
  },
  UI: {
    useRow: vi.fn(() => ({})),
    useCell: vi.fn(() => undefined),
    useTable: vi.fn((table: string) => fixture.tables[table] ?? {}),
    useResultTable: vi.fn(() => ({})),
    useResultSortedRowIds: vi.fn(() => []),
    useSliceRowIds: vi.fn(
      (index: string, sliceId: string) =>
        fixture.slices[index]?.[sliceId] ?? [],
    ),
    useStore: vi.fn(() => null),
    useValue: vi.fn(() => undefined),
    useSetRowCallback: vi.fn(),
    useDelRowCallback: vi.fn(),
  },
}));

import { useHumansByIds, useOrganizationMembers } from "./hooks";

describe("contacts boundary hooks", () => {
  beforeEach(() => {
    resetFixture();
  });

  it("returns only requested humans for id-scoped reads", () => {
    setFixture({
      tables: {
        humans: {
          "human-1": { name: "Alice", email: "a@example.com" },
          "human-2": { name: "Bob", email: "b@example.com" },
        },
      },
    });

    const result = renderHook(() => useHumansByIds(["human-2"]));
    expect(result.result.current).toEqual({
      "human-2": expect.objectContaining({
        id: "human-2",
        name: "Bob",
      }),
    });
  });

  it("builds organization members from org-specific ids", () => {
    setFixture({
      tables: {
        humans: {
          "human-1": { name: "Alice", email: "a@example.com", org_id: "org-1" },
          "human-2": { name: "Bob", email: "b@example.com", org_id: "org-1" },
        },
      },
      slices: {
        humansByOrg: {
          "org-1": ["human-2"],
        },
      },
    });

    const result = renderHook(() => useOrganizationMembers("org-1"));
    expect(result.result.current).toHaveLength(1);
    expect(result.result.current[0]).toEqual(
      expect.objectContaining({ id: "human-2", name: "Bob" }),
    );
  });
});
