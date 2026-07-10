import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  rows: [] as Array<Record<string, unknown>>,
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: (options: {
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => ({ data: options.mapRows(mocks.rows) }),
}));

vi.mock("~/shared/utils", () => ({
  DEFAULT_USER_ID: "00000000-0000-0000-0000-000000000000",
  id: () => "human-new",
}));

import {
  applyContactEnhancement,
  createHuman,
  deleteHuman,
  loadHuman,
  loadHumansByIds,
  loadOrganization,
  mergeHumans,
  reorderPinnedContacts,
  searchContacts,
  toggleContactPin,
  updateHuman,
  useHumans,
  useOrganizations,
} from "./queries";

describe("contact SQLite queries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rows = [];
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
      },
    ]);
  });

  it("returns the durable id after inserting a human", async () => {
    await expect(
      createHuman({
        ownerUserId: "user-1",
        name: "Alice",
        email: "alice@example.com",
      }),
    ).resolves.toBe("human-new");

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("INSERT INTO humans");
    expect(statement.params).toContain("human-new");
    expect(statement.params).toContain("alice@example.com");
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
      },
    ]);
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

  it("updates only whitelisted human fields", async () => {
    await updateHuman("human-1", {
      name: "Alice Kim",
      jobTitle: "Staff Engineer",
    });

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("name = ?");
    expect(statement.sql).toContain("job_title = ?");
    expect(statement.params.slice(0, 2)).toEqual([
      "Alice Kim",
      "Staff Engineer",
    ]);
    expect(statement.params[statement.params.length - 1]).toBe("human-1");
  });

  it("soft-deletes contacts without removing their rows", async () => {
    await deleteHuman("human-1");

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("UPDATE humans");
    expect(statement.sql).toContain("SET deleted_at = ?");
    expect(statement.params[statement.params.length - 1]).toBe("human-1");
  });

  it("computes pin order across humans and organizations", async () => {
    await toggleContactPin("human", "human-1");

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("UPDATE humans");
    expect(statement.sql).toContain("SELECT pin_order FROM organizations");
    expect(statement.sql).toContain(
      "pinned = CASE WHEN pinned = 1 THEN 0 ELSE 1 END",
    );
  });

  it("reorders mixed pinned contacts atomically", async () => {
    await reorderPinnedContacts([
      { type: "organization", id: "organization-1" },
      { type: "human", id: "human-1" },
    ]);

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE organizations");
    expect(statements[0].params[0]).toBe(0);
    expect(statements[1].sql).toContain("UPDATE humans");
    expect(statements[1].params[0]).toBe(1);
  });

  it("merges participant mappings and tombstones the duplicate atomically", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "human-primary",
        owner_user_id: "user-1",
        created_at: "first",
        organization_id: "",
        name: "Alice",
        email: "alice@example.com",
        phone: "111",
        job_title: "Engineer",
        linkedin_username: "alice",
        memo: "Primary",
        pinned: 0,
        pin_order: null,
      },
      {
        id: "human-duplicate",
        owner_user_id: "user-1",
        created_at: "second",
        organization_id: "organization-1",
        name: "Alice",
        email: "alice@example.com",
        phone: "222",
        job_title: "Founder",
        linkedin_username: "alice-two",
        memo: "Duplicate",
        pinned: 0,
        pin_order: null,
      },
    ]);

    await mergeHumans("human-primary", "human-duplicate");

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(4);
    expect(statements[0].sql).toContain("UPDATE session_participants");
    expect(statements[1].params).toContain("human-primary");
    expect(statements[2].params).toContain("Engineer, Founder");
    expect(statements[2].params).toContain("organization-1");
    expect(statements[3].sql).toContain("SET deleted_at = ?");
    expect(statements[3].params[statements[3].params.length - 1]).toBe(
      "human-duplicate",
    );
  });

  it("creates an organization and updates the human atomically", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1, 1]);

    await applyContactEnhancement({
      humanId: "human-1",
      ownerUserId: "user-1",
      changes: {
        name: "Alice Kim",
        email: "alice@example.com",
        companyName: "Example",
      },
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0]?.sql).toContain("INSERT INTO organizations");
    expect(statements[0]?.sql).toContain("NOT EXISTS");
    expect(statements[1]?.sql).toContain("UPDATE humans");
    expect(statements[1]?.sql).toContain("organization_id = CASE");
    expect(statements[1]?.params).toContain("human-1");
  });
});
