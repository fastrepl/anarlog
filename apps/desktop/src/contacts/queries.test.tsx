import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  rows: [] as Array<Record<string, unknown>>,
  loading: false,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: (options: {
    enabled?: boolean;
    mapRows?: (rows: Array<Record<string, unknown>>) => unknown;
  }) => ({
    data:
      options.enabled === false || mocks.loading
        ? undefined
        : options.mapRows
          ? options.mapRows(mocks.rows)
          : mocks.rows,
  }),
}));

import {
  loadHuman,
  loadHumansByIds,
  loadOrganization,
  searchContacts,
  useHumanDisplayRecordsByIds,
  useHumans,
  useOrganizationDisplayRecordsByIds,
  useOrganizations,
} from "./queries";

describe("contact SQLite queries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rows = [];
    mocks.loading = false;
    mocks.execute.mockResolvedValue([]);
  });

  it("maps canonical human rows", () => {
    mocks.rows = [
      {
        id: "human-1",
        owner_user_id: "user-1",
        created_at: "2026-07-10T12:00:00.000Z",
        organization_id: "organization-1",
        name: "Alice",
        email: "alice@example.com",
        phone: "",
        job_title: "Engineer",
        linkedin_username: "alice",
        memo: "",
        pinned: 1,
        pin_order: 2,
        avatar_data_url: "data:image/jpeg;base64,abc",
        contact_summary_json: JSON.stringify({
          facts: ["Fact one", "Fact two", "Fact three"],
          sourceHash: "source-1",
          generatedAt: "2026-08-12T12:00:00.000Z",
        }),
      },
    ];

    const { result } = renderHook(() => useHumans());

    expect(result.current).toEqual([
      {
        id: "human-1",
        userId: "user-1",
        createdAt: "2026-07-10T12:00:00.000Z",
        organizationId: "organization-1",
        name: "Alice",
        email: "alice@example.com",
        phone: "",
        jobTitle: "Engineer",
        linkedinUsername: "alice",
        memo: "",
        pinned: true,
        pinOrder: 2,
        avatarDataUrl: "data:image/jpeg;base64,abc",
        summary: {
          facts: ["Fact one", "Fact two", "Fact three"],
          sourceHash: "source-1",
          promptKey: "",
          generatedAt: "2026-08-12T12:00:00.000Z",
          sources: [],
        },
        additionalEmails: [],
      },
    ]);
  });

  it("maps canonical organization rows", () => {
    mocks.rows = [
      {
        id: "organization-1",
        owner_user_id: "user-1",
        created_at: "2026-07-10T12:00:00.000Z",
        name: "Example",
        memo: "Customer",
        pinned: 0,
        pin_order: null,
        avatar_data_url: null,
        team_workspace: 1,
      },
    ];

    const { result } = renderHook(() => useOrganizations());

    expect(result.current).toEqual([
      {
        id: "organization-1",
        userId: "user-1",
        createdAt: "2026-07-10T12:00:00.000Z",
        name: "Example",
        memo: "Customer",
        pinned: false,
        pinOrder: null,
        avatarDataUrl: null,
        teamWorkspace: true,
      },
    ]);
  });

  it("loads only lightweight display fields for referenced contacts", () => {
    mocks.rows = [
      {
        id: "human-1",
        organization_id: "organization-1",
        name: "Alice",
        email: "alice@example.com",
        avatar_data_url: "data:image/jpeg;base64,unused",
      },
    ];

    const { result: humanResult } = renderHook(() =>
      useHumanDisplayRecordsByIds(["human-1", "human-1", ""]),
    );

    expect(humanResult.current).toEqual([
      {
        id: "human-1",
        organizationId: "organization-1",
        name: "Alice",
        email: "alice@example.com",
      },
    ]);

    mocks.rows = [{ id: "organization-1", name: "Acme" }];
    const { result: organizationResult } = renderHook(() =>
      useOrganizationDisplayRecordsByIds(["organization-1"]),
    );

    expect(organizationResult.current).toEqual([
      { id: "organization-1", name: "Acme" },
    ]);
  });

  it("keeps the last resolved display records while a by-id query is loading", () => {
    mocks.rows = [
      {
        id: "human-1",
        organization_id: "organization-1",
        name: "Alice",
        email: "alice@example.com",
      },
    ];

    const { result: humanResult, rerender: rerenderHumans } = renderHook(
      ({ ids }) => useHumanDisplayRecordsByIds(ids),
      { initialProps: { ids: ["human-1"] } },
    );

    expect(humanResult.current).toEqual([
      {
        id: "human-1",
        organizationId: "organization-1",
        name: "Alice",
        email: "alice@example.com",
      },
    ]);

    mocks.loading = true;
    rerenderHumans({ ids: ["human-1", "human-2"] });
    expect(humanResult.current).toEqual([
      {
        id: "human-1",
        organizationId: "organization-1",
        name: "Alice",
        email: "alice@example.com",
      },
    ]);

    mocks.loading = false;
    mocks.rows = [{ id: "organization-1", name: "Acme" }];
    const { result: organizationResult, rerender: rerenderOrgs } = renderHook(
      ({ ids }) => useOrganizationDisplayRecordsByIds(ids),
      { initialProps: { ids: ["organization-1"] } },
    );

    expect(organizationResult.current).toEqual([
      { id: "organization-1", name: "Acme" },
    ]);

    mocks.loading = true;
    rerenderOrgs({ ids: ["organization-1", "organization-2"] });
    expect(organizationResult.current).toEqual([
      { id: "organization-1", name: "Acme" },
    ]);
  });

  it("drops held display records when no ids are referenced", () => {
    mocks.rows = [
      {
        id: "human-1",
        organization_id: "organization-1",
        name: "Alice",
        email: "alice@example.com",
      },
    ];

    const { result, rerender } = renderHook(
      ({ ids }) => useHumanDisplayRecordsByIds(ids),
      { initialProps: { ids: ["human-1"] } },
    );

    expect(result.current).toHaveLength(1);

    mocks.loading = true;
    rerender({ ids: [] });
    expect(result.current).toEqual([]);
  });

  it("loads deduplicated human records directly from SQLite", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "human-1",
        owner_user_id: "user-1",
        created_at: "2026-07-10T12:00:00.000Z",
        organization_id: "organization-1",
        name: "Alice",
        email: "alice@example.com",
        phone: "",
        job_title: "Engineer",
        linkedin_username: "alice",
        memo: "Lead",
        pinned: 0,
        pin_order: null,
      },
    ]);

    await expect(loadHumansByIds(["human-1", "human-1", ""])).resolves.toEqual([
      expect.objectContaining({
        id: "human-1",
        organizationId: "organization-1",
        jobTitle: "Engineer",
      }),
    ]);
    expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), ["human-1"]);

    await expect(loadHuman("")).resolves.toBeNull();
  });

  it("loads one active organization directly from SQLite", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "organization-1",
        owner_user_id: "user-1",
        created_at: "2026-07-10T12:00:00.000Z",
        name: "Example",
        memo: "Customer",
        pinned: 0,
        pin_order: null,
      },
    ]);

    await expect(loadOrganization("organization-1")).resolves.toEqual(
      expect.objectContaining({ id: "organization-1", name: "Example" }),
    );
    expect(mocks.execute.mock.calls[0][0]).toContain("deleted_at IS NULL");
  });

  it("searches canonical contacts with organization details", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "human-1",
        name: "Alice",
        email: "alice@example.com",
        phone: "",
        job_title: "Engineer",
        organization_name: "Example",
        memo: "Customer lead",
      },
    ]);

    await expect(searchContacts("  ALICE ", 5)).resolves.toEqual([
      {
        id: "human-1",
        name: "Alice",
        email: "alice@example.com",
        phone: null,
        jobTitle: "Engineer",
        organization: "Example",
        memo: "Customer lead",
      },
    ]);
    expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
      "alice",
      "alice",
      5,
    ]);
  });
});
