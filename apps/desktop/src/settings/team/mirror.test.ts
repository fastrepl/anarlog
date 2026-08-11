import { beforeEach, describe, expect, it, vi } from "vitest";

type Statement = { sql: string; params: unknown[] };

const executeTransaction = vi.fn((_statements: Statement[]) =>
  Promise.resolve(),
);

vi.mock("~/db", () => ({ executeTransaction }));

const { mirrorSharedWorkspaces } = await import("./mirror");
const { TeamError } = await import("./client");

import type { TeamContext } from "./client";

const USER_ID = "22222222-2222-4222-8222-222222222222";
const WORKSPACE_ID = "11111111-1111-4111-8111-111111111111";

const context = {
  supabase: {},
  session: { user: { id: USER_ID } },
} as unknown as TeamContext;

function statements(): Statement[] {
  const call = executeTransaction.mock.calls[0];
  if (!call) throw new Error("executeTransaction was not called");
  return call[0];
}

describe("mirrorSharedWorkspaces", () => {
  beforeEach(() => executeTransaction.mockClear());

  it("upserts each workspace with the caller's own membership", async () => {
    await mirrorSharedWorkspaces(context, [
      {
        workspaceId: WORKSPACE_ID,
        name: "Acme",
        ownerUserId: USER_ID,
        role: "owner",
      },
    ]);

    const written = statements();
    expect(written[0].sql).toContain("INSERT INTO workspaces");
    expect(written[0].params).toContain("Acme");
    expect(written[1].sql).toContain("INSERT INTO workspace_memberships");
    expect(written[1].params).toContain("owner");
    expect(written[1].params).toContain(USER_ID);
  });

  it("revives a workspace that was previously tombstoned", async () => {
    await mirrorSharedWorkspaces(context, [
      {
        workspaceId: WORKSPACE_ID,
        name: "Acme",
        ownerUserId: USER_ID,
        role: "member",
      },
    ]);

    const written = statements();
    expect(written[0].sql).toContain("deleted_at = NULL");
    expect(written[1].sql).toContain("deleted_at = NULL");
  });

  it("tombstones access the server no longer lists", async () => {
    await mirrorSharedWorkspaces(context, [
      {
        workspaceId: WORKSPACE_ID,
        name: "Acme",
        ownerUserId: USER_ID,
        role: "member",
      },
    ]);

    const written = statements();
    const tombstone = written[written.length - 1];
    expect(tombstone.sql).toContain("SET deleted_at");
    expect(tombstone.sql).toContain("NOT IN");
    expect(tombstone.params).toContain(WORKSPACE_ID);
  });

  it("tombstones every shared workspace when access is gone entirely", async () => {
    await mirrorSharedWorkspaces(context, []);

    const written = statements();
    expect(written).toHaveLength(1);
    // No survivors means no exclusion list, otherwise the IN () would be
    // invalid SQL and nothing would be revoked locally.
    expect(written[0].sql).not.toContain("NOT IN");
    expect(written[0].sql).toContain("SET deleted_at");
  });

  it("refuses to mirror without an account id", async () => {
    await expect(
      mirrorSharedWorkspaces(
        {
          supabase: {},
          session: { user: { id: "" } },
        } as unknown as TeamContext,
        [],
      ),
    ).rejects.toThrow(TeamError);
  });
});
