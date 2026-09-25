import { beforeEach, describe, expect, it, vi } from "vitest";

type Statement = { sql: string; params: unknown[] };

const executeTransaction = vi.fn((_statements: Statement[]) =>
  Promise.resolve(),
);
type DbRow = Record<string, unknown>;

const execute = vi.fn(
  (_sql: string, _params?: unknown[]): Promise<DbRow[]> => Promise.resolve([]),
);

vi.mock("~/db", () => ({
  executeTransaction,
  liveQueryClient: { execute },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, write: () => Promise<unknown>) =>
    write(),
}));

const { mirrorWorkspaceContacts } = await import("./contacts-mirror");

const WORKSPACE_ID = "11111111-1111-4111-8111-111111111111";
const OTHER_WORKSPACE_ID = "33333333-3333-4333-8333-333333333333";
const MEMBER_ID = "44444444-4444-4444-8444-444444444444";

const MEMBER = {
  userId: MEMBER_ID,
  email: "teammate@fastrepl.com",
  name: "Teammate",
  avatarUrl: "https://example.com/avatar.png",
};

function workspace(overrides = {}) {
  return {
    workspaceId: WORKSPACE_ID,
    name: "Fastrepl",
    logoDataUrl: "data:image/png;base64,AAAA",
    members: [MEMBER],
    ...overrides,
  };
}

function statements(): Statement[] {
  return executeTransaction.mock.calls.map((call) => call[0][0]);
}

function selects(sql: string): unknown[][] {
  return execute.mock.calls
    .filter((call) => (call[0] as string).includes(sql))
    .map((call) => (call[1] ?? []) as unknown[]);
}

describe("mirrorWorkspaceContacts", () => {
  beforeEach(() => {
    executeTransaction.mockClear();
    execute.mockClear();
    execute.mockImplementation(() => Promise.resolve([]));
  });

  it("creates a tracked organization and a linked human per member", async () => {
    await mirrorWorkspaceContacts([workspace()]);

    const written = statements();
    const orgInsert = written.find((s) =>
      s.sql.includes("INSERT INTO organizations"),
    );
    const humanInsert = written.find((s) =>
      s.sql.includes("INSERT INTO humans"),
    );
    expect(orgInsert).toBeDefined();
    expect(humanInsert).toBeDefined();
    expect(orgInsert!.params).toContain(WORKSPACE_ID);
    expect(orgInsert!.params).toContain("Fastrepl");
    const orgMeta = JSON.parse(orgInsert!.params[3] as string);
    expect(orgMeta.teamWorkspace).toBe(true);
    expect(orgMeta.teamLogoDataUrl).toBe("data:image/png;base64,AAAA");
    expect(humanInsert!.params).toContain(MEMBER_ID);
    expect(humanInsert!.params).toContain(WORKSPACE_ID);
    const humanMeta = JSON.parse(humanInsert!.params[5] as string);
    expect(humanMeta.teamWorkspaceId).toBe(WORKSPACE_ID);
    expect(humanMeta.avatarDataUrl).toBe(MEMBER.avatarUrl);
  });

  it("follows server renames while the organization is untouched", async () => {
    execute.mockImplementation((sql: string) => {
      if (sql.includes("FROM organizations")) {
        return Promise.resolve([
          {
            id: WORKSPACE_ID,
            name: "Old name",
            metadata_json: JSON.stringify({
              teamWorkspace: true,
              teamName: "Old name",
              teamLogoDataUrl: null,
              avatarDataUrl: null,
            }),
            deleted_at: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    const orgUpdate = statements().find(
      (s) =>
        s.sql.includes("UPDATE organizations") &&
        s.params.includes(WORKSPACE_ID),
    );
    expect(orgUpdate).toBeDefined();
    expect(orgUpdate!.params[0]).toBe("Fastrepl");
    const meta = JSON.parse(orgUpdate!.params[1] as string);
    expect(meta.teamName).toBe("Fastrepl");
    expect(meta.avatarDataUrl).toBe("data:image/png;base64,AAAA");
  });

  it("keeps a user-renamed organization name and a user avatar", async () => {
    execute.mockImplementation((sql: string) => {
      if (sql.includes("FROM organizations")) {
        return Promise.resolve([
          {
            id: WORKSPACE_ID,
            name: "Custom name",
            metadata_json: JSON.stringify({
              teamWorkspace: true,
              teamName: "Old name",
              teamLogoDataUrl: null,
              avatarDataUrl: "data:image/png;base64,CUSTOM",
            }),
            deleted_at: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    const orgUpdate = statements().find(
      (s) =>
        s.sql.includes("UPDATE organizations") &&
        s.params.includes(WORKSPACE_ID),
    );
    expect(orgUpdate).toBeDefined();
    expect(orgUpdate!.params[0]).toBe("Custom name");
    const meta = JSON.parse(orgUpdate!.params[1] as string);
    expect(meta.teamName).toBe("Fastrepl");
    expect(meta.teamLogoDataUrl).toBe("data:image/png;base64,AAAA");
    expect(meta.avatarDataUrl).toBe("data:image/png;base64,CUSTOM");
  });

  it("writes nothing when the mirror is already in sync", async () => {
    const meta = {
      teamWorkspace: true,
      teamName: "Fastrepl",
      teamLogoDataUrl: "data:image/png;base64,AAAA",
      avatarDataUrl: "data:image/png;base64,AAAA",
    };
    execute.mockImplementation((sql: string) => {
      if (sql.includes("FROM organizations")) {
        return Promise.resolve([
          {
            id: WORKSPACE_ID,
            name: "Fastrepl",
            metadata_json: JSON.stringify(meta),
            deleted_at: null,
          },
        ]);
      }
      if (sql.includes("FROM humans WHERE id = ?")) {
        return Promise.resolve([
          {
            id: MEMBER_ID,
            name: "Teammate",
            email: "teammate@fastrepl.com",
            organization_id: WORKSPACE_ID,
            metadata_json: JSON.stringify({
              teamWorkspaceId: WORKSPACE_ID,
              teamAvatarUrl: MEMBER.avatarUrl,
              avatarDataUrl: MEMBER.avatarUrl,
            }),
            deleted_at: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    expect(statements()).toHaveLength(0);
  });

  it("does not steal a user-managed organization link on a member contact", async () => {
    execute.mockImplementation((sql: string) => {
      if (sql.includes("FROM humans WHERE id = ?")) {
        return Promise.resolve([
          {
            id: MEMBER_ID,
            name: "Known contact",
            email: "teammate@fastrepl.com",
            organization_id: "user-picked-org",
            metadata_json: "{}",
            deleted_at: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    const humanUpdate = statements().find((s) =>
      s.sql.includes("UPDATE humans"),
    );
    expect(humanUpdate).toBeDefined();
    // organization_id is the first param: the user's own link is preserved.
    expect(humanUpdate!.params[0]).toBe("user-picked-org");
    expect(humanUpdate!.params[1]).toBe("Known contact");
    const meta = JSON.parse(humanUpdate!.params[3] as string);
    expect(meta.teamWorkspaceId).toBe(WORKSPACE_ID);
  });

  it("leaves a contact the user deleted alone", async () => {
    execute.mockImplementation((sql: string) => {
      if (sql.includes("FROM humans WHERE id = ?")) {
        return Promise.resolve([
          {
            id: MEMBER_ID,
            name: "Teammate",
            email: "teammate@fastrepl.com",
            organization_id: "",
            metadata_json: "{}",
            deleted_at: "2026-09-01T00:00:00Z",
          },
        ]);
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    expect(
      statements().find((s) => s.sql.includes("UPDATE humans")),
    ).toBeUndefined();
  });

  it("unlinks contacts of members who left the roster", async () => {
    execute.mockImplementation((sql: string, params?: unknown[]) => {
      if (sql.includes("teamWorkspaceId") && sql.includes("NOT IN")) {
        return Promise.resolve([
          {
            id: "departed",
            name: "Gone",
            email: "gone@x.com",
            organization_id: WORKSPACE_ID,
            metadata_json: JSON.stringify({ teamWorkspaceId: WORKSPACE_ID }),
            deleted_at: null,
          },
        ]);
      }
      if (sql.includes("FROM humans WHERE id = ?")) {
        const id = (params ?? [])[0];
        if (id === "departed") {
          return Promise.resolve([
            {
              id: "departed",
              name: "Gone",
              email: "gone@x.com",
              organization_id: WORKSPACE_ID,
              metadata_json: JSON.stringify({
                teamWorkspaceId: WORKSPACE_ID,
                teamAvatarUrl: "https://example.com/gone.png",
                avatarDataUrl: "https://example.com/gone.png",
              }),
              deleted_at: null,
            },
          ]);
        }
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    const unlink = statements().find(
      (s) => s.sql.includes("UPDATE humans") && s.params.includes("departed"),
    );
    expect(unlink).toBeDefined();
    expect(unlink!.params[0]).toBe("");
    const meta = JSON.parse(unlink!.params[1] as string);
    expect(meta.teamWorkspaceId).toBeUndefined();
    expect(meta.avatarDataUrl).toBe("https://example.com/gone.png");
  });

  it("unlinks contacts of workspaces the account left entirely", async () => {
    const staleRow = {
      id: "stale-user",
      name: "Former",
      email: "former@x.com",
      organization_id: OTHER_WORKSPACE_ID,
      metadata_json: JSON.stringify({
        teamWorkspaceId: OTHER_WORKSPACE_ID,
      }),
      deleted_at: null,
    };
    execute.mockImplementation((sql: string, params?: unknown[]) => {
      if (sql.includes("teamWorkspaceId") && sql.includes("NOT IN")) {
        return Promise.resolve([staleRow]);
      }
      if (sql.includes("FROM humans WHERE id = ?")) {
        if ((params ?? [])[0] === "stale-user") {
          return Promise.resolve([staleRow]);
        }
      }
      return Promise.resolve([]);
    });

    await mirrorWorkspaceContacts([workspace()]);

    const unlink = statements().find(
      (s) => s.sql.includes("UPDATE humans") && s.params.includes("stale-user"),
    );
    expect(unlink).toBeDefined();
    expect(unlink!.params[0]).toBe("");
  });

  it("skips member mirroring when the roster could not be listed", async () => {
    await mirrorWorkspaceContacts([workspace({ members: undefined })]);

    expect(selects("FROM humans WHERE id = ?").flat()).not.toContain(MEMBER_ID);
    expect(
      statements().find((s) => s.sql.includes("INSERT INTO humans")),
    ).toBeUndefined();
    // The organization still mirrors.
    expect(
      statements().find((s) => s.sql.includes("INSERT INTO organizations")),
    ).toBeDefined();
  });
});
