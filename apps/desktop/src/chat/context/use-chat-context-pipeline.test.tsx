import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const queryMocks = vi.hoisted(() => ({
  sessionIds: vi.fn(),
  humanIds: vi.fn(),
  organizationIds: vi.fn(),
  emptyHumans: false,
}));

vi.mock("~/session/queries", () => ({
  useSessionSummariesByIds: (ids: string[]) => {
    queryMocks.sessionIds(ids);
    return ids.includes("session-1")
      ? [
          {
            id: "session-1",
            title: "Planning",
            created_at: "2026-07-10T09:00:00.000Z",
          },
        ]
      : [];
  },
}));

vi.mock("~/contacts/queries", () => ({
  useHumanDisplayRecordsByIds: (ids: string[]) => {
    queryMocks.humanIds(ids);
    return queryMocks.emptyHumans || !ids.includes("human-1")
      ? []
      : [
          {
            id: "human-1",
            name: "Alice",
            email: "alice@example.com",
            organizationId: "organization-1",
          },
        ];
  },
  useOrganizationDisplayRecordsByIds: (ids: string[]) => {
    queryMocks.organizationIds(ids);
    return ids.includes("organization-1")
      ? [{ id: "organization-1", name: "Acme" }]
      : [];
  },
}));

import { useChatContextPipeline } from "./use-chat-context-pipeline";

describe("chat context display pipeline", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryMocks.emptyHumans = false;
  });

  it("resolves pending entity labels from live SQLite records", () => {
    const { result } = renderHook(() =>
      useChatContextPipeline({
        messages: [],
        currentSessionId: "session-1",
        pendingManualRefs: [
          {
            kind: "human",
            key: "human:manual:human-1",
            source: "manual",
            humanId: "human-1",
          },
          {
            kind: "organization",
            key: "organization:manual:organization-1",
            source: "manual",
            organizationId: "organization-1",
          },
        ],
      }),
    );

    expect(result.current.contextEntities).toEqual([
      expect.objectContaining({
        kind: "session",
        title: "Planning",
        date: "2026-07-10T09:00:00.000Z",
      }),
      expect.objectContaining({
        kind: "human",
        name: "Alice",
        email: "alice@example.com",
        organizationName: "Acme",
        removable: true,
      }),
      expect.objectContaining({
        kind: "organization",
        name: "Acme",
        removable: true,
      }),
    ]);
    expect(queryMocks.sessionIds).toHaveBeenCalledWith(["session-1"]);
    expect(queryMocks.humanIds).toHaveBeenCalledWith(["human-1"]);
    expect(queryMocks.organizationIds).toHaveBeenCalledWith(["organization-1"]);
  });

  it("attaches the active folder as pending context", () => {
    const { result } = renderHook(() =>
      useChatContextPipeline({
        messages: [],
        folderId: "CS 101",
        pendingManualRefs: [],
      }),
    );

    expect(result.current.pendingRefs).toEqual([
      {
        kind: "folder",
        key: "folder:auto:CS 101",
        source: "auto-current",
        folderId: "CS 101",
      },
    ]);
    expect(result.current.contextEntities).toEqual([
      expect.objectContaining({
        kind: "folder",
        title: "CS 101",
        removable: false,
        pending: true,
      }),
    ]);
    expect(queryMocks.sessionIds).toHaveBeenCalledWith([]);
  });

  it("does not request unreferenced context catalogs", () => {
    const { result } = renderHook(() =>
      useChatContextPipeline({
        messages: [],
        pendingManualRefs: [],
      }),
    );

    expect(result.current.contextEntities).toEqual([]);
    expect(queryMocks.sessionIds).toHaveBeenCalledWith([]);
    expect(queryMocks.humanIds).toHaveBeenCalledWith([]);
    expect(queryMocks.organizationIds).toHaveBeenCalledWith([]);
  });

  it("drops derived organization lookups when humans resolve empty", () => {
    const pendingHuman = {
      kind: "human" as const,
      key: "human:manual:human-1",
      source: "manual" as const,
      humanId: "human-1",
    };
    const { rerender } = renderHook(
      ({ pendingManualRefs }) =>
        useChatContextPipeline({
          messages: [],
          pendingManualRefs,
        }),
      { initialProps: { pendingManualRefs: [pendingHuman] } },
    );

    expect(queryMocks.organizationIds).toHaveBeenLastCalledWith([
      "organization-1",
    ]);

    queryMocks.emptyHumans = true;
    rerender({ pendingManualRefs: [pendingHuman] });

    expect(queryMocks.humanIds).toHaveBeenLastCalledWith(["human-1"]);
    expect(queryMocks.organizationIds).toHaveBeenLastCalledWith([]);
  });
});
