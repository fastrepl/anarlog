import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  rows: [] as Array<Record<string, unknown>>,
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  useLiveQuery: (options: {
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => ({ data: options.mapRows(mocks.rows) }),
}));

vi.mock("~/shared/utils", () => ({ id: () => "human-new" }));

import { applyContactEnhancement, createHuman, useHumans } from "./queries";

describe("contact SQLite queries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rows = [];
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
