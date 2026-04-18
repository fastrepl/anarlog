import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { useTableMock, useSliceRowIdsMock } = vi.hoisted(() => ({
  useTableMock: vi.fn(() => ({})),
  useSliceRowIdsMock: vi.fn<() => string[]>(() => []),
}));

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
    useTable: useTableMock,
    useResultTable: vi.fn(() => ({})),
    useResultSortedRowIds: vi.fn(() => []),
    useSliceRowIds: useSliceRowIdsMock,
    useStore: vi.fn(() => null),
    useValue: vi.fn(() => undefined),
    useSetRowCallback: vi.fn(),
    useDelRowCallback: vi.fn(),
  },
}));

import { useHumansByIds, useOrganizationMembers } from "./hooks";

describe("contacts boundary hooks", () => {
  beforeEach(() => {
    useTableMock.mockReset();
    useSliceRowIdsMock.mockReset();
  });

  it("returns only requested humans for id-scoped reads", () => {
    useTableMock.mockReturnValue({
      "human-1": { name: "Alice", email: "a@example.com" },
      "human-2": { name: "Bob", email: "b@example.com" },
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
    useSliceRowIdsMock.mockReturnValue(["human-2"]);
    useTableMock.mockReturnValue({
      "human-1": { name: "Alice", email: "a@example.com", org_id: "org-1" },
      "human-2": { name: "Bob", email: "b@example.com", org_id: "org-1" },
    });

    const result = renderHook(() => useOrganizationMembers("org-1"));
    expect(result.result.current).toHaveLength(1);
    expect(result.result.current[0]).toEqual(
      expect.objectContaining({ id: "human-2", name: "Bob" }),
    );
  });
});
