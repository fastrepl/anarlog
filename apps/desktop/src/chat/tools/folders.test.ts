import { createRequire } from "node:module";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { z } from "zod";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(),
  createFolder: vi.fn(),
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  executeTransaction: mocks.executeTransaction,
  useLiveQuery: vi.fn(),
}));
vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: { createFolder: mocks.createFolder },
}));

import {
  buildCreateFolderTool,
  buildListFoldersTool,
  buildMoveMeetingsToFolderTool,
} from "./folders";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");

let database: InstanceType<typeof DatabaseSync>;
const options = { toolCallId: "folder-test", messages: [] };
const listTool = buildListFoldersTool();
const createTool = buildCreateFolderTool();
const moveTool = buildMoveMeetingsToFolderTool();

function addMeeting(id: string, folder = "", deletedAt: string | null = null) {
  database
    .prepare(
      "INSERT INTO sessions (id, title, folder_path, deleted_at) VALUES (?, ?, ?, ?)",
    )
    .run(id, "DEFCON 1", folder, deletedAt);
}

function folderFor(id: string) {
  return database
    .prepare("SELECT folder_path FROM sessions WHERE id = ?")
    .get(id)?.folder_path;
}

beforeEach(() => {
  vi.resetAllMocks();
  database = new DatabaseSync(":memory:");
  database.exec(`
    CREATE TABLE sessions (
      id TEXT PRIMARY KEY, title TEXT, folder_path TEXT DEFAULT '',
      workspace_id TEXT DEFAULT '', updated_at TEXT DEFAULT '', deleted_at TEXT
    );
    CREATE TABLE folders (
      id TEXT PRIMARY KEY, path TEXT, workspace_id TEXT DEFAULT '',
      updated_at TEXT DEFAULT '', deleted_at TEXT
    );
    CREATE TABLE folder_attachments (folder_path TEXT, deleted_at TEXT);
    CREATE TABLE session_documents (id TEXT, session_id TEXT, body TEXT);
  `);
  mocks.execute.mockImplementation(async (sql: string, params: never[] = []) =>
    database.prepare(sql).all(...params),
  );
  mocks.executeTransaction.mockImplementation(
    async (statements: Array<{ sql: string; params: never[] }>) => {
      database.exec("BEGIN");
      try {
        const changes = statements.map(({ sql, params }) =>
          Number(database.prepare(sql).run(...params).changes),
        );
        database.exec("COMMIT");
        return changes;
      } catch (error) {
        database.exec("ROLLBACK");
        throw error;
      }
    },
  );
  mocks.createFolder.mockResolvedValue({ status: "ok", data: null });
});

afterEach(() => database.close());

describe("folder discovery and creation", () => {
  it("finds empty folders, material folders, and parents with filtering and pagination", async () => {
    addMeeting("meeting", "Work/DEFCON");
    addMeeting("deleted", "Deleted", "2026-09-01");
    database.exec(`
      INSERT INTO folders (id, path) VALUES ('empty', 'Work/Empty');
      INSERT INTO folder_attachments VALUES ('Work/Materials', NULL);
      INSERT INTO folder_attachments VALUES ('Old', '2026-09-01');
    `);

    await expect(
      listTool.execute!({ query: "work", limit: 2 }, options),
    ).resolves.toEqual({
      folders: ["Work", "Work/DEFCON"],
      total: 4,
      next_offset: 2,
    });
    await expect(
      listTool.execute!({ query: "WORK", offset: 2, limit: 2 }, options),
    ).resolves.toEqual({
      folders: ["Work/Empty", "Work/Materials"],
      total: 4,
      next_offset: null,
    });
    await expect(
      listTool.execute!({ query: "Deleted" }, options),
    ).resolves.toMatchObject({ folders: [] });
  });

  it("creates nested folders once and preserves empty folders", async () => {
    await createTool.execute!({ folder_path: " defcons\\weekly/ " }, options);
    await createTool.execute!({ folder_path: "defcons/weekly" }, options);

    expect(mocks.createFolder).toHaveBeenCalledWith("defcons/weekly");
    expect(
      database.prepare("SELECT path FROM folders ORDER BY path").all(),
    ).toEqual([{ path: "defcons" }, { path: "defcons/weekly" }]);
    await expect(listTool.execute!({}, options)).resolves.toMatchObject({
      folders: ["defcons", "defcons/weekly"],
    });
  });

  it.each(["", "   ", "../escape", "/absolute", "work//weekly"])(
    "rejects invalid named folder %j",
    (folder_path) => {
      expect(
        (createTool.inputSchema as z.ZodType).safeParse({ folder_path })
          .success,
      ).toBe(false);
    },
  );

  it("propagates disk creation failures instead of reporting success", async () => {
    mocks.createFolder.mockResolvedValue({
      status: "error",
      error: "disk full",
    });
    await expect(
      createTool.execute!({ folder_path: "defcons" }, options),
    ).rejects.toThrow("disk full");
  });
});

describe("bulk folder assignment", () => {
  beforeEach(() => {
    database.exec(
      "INSERT INTO folders (id, path) VALUES ('defcons', 'defcons')",
    );
  });

  it("moves meetings without changing their contents, deduplicates IDs, and skips already filed meetings", async () => {
    addMeeting("first", "Work");
    addMeeting("second");
    addMeeting("already", "defcons");
    database.exec(
      "INSERT INTO session_documents VALUES ('memo', 'first', 'Keep these notes')",
    );

    const result = await moveTool.execute!(
      {
        meeting_ids: ["first", "second", "already", "first"],
        folder_path: "defcons",
      },
      options,
    );

    expect(result).toMatchObject({
      status: "ok",
      moved: 2,
      unchanged: 1,
      failed: 0,
    });
    expect(result).toMatchObject({
      results: [expect.anything(), expect.anything(), expect.anything()],
    });
    expect(folderFor("first")).toBe("defcons");
    expect(folderFor("second")).toBe("defcons");
    expect(mocks.executeTransaction).toHaveBeenCalledTimes(2);
    expect(
      database.prepare("SELECT body FROM session_documents").get()?.body,
    ).toBe("Keep these notes");

    await expect(
      moveTool.execute!(
        { meeting_ids: ["first", "second"], folder_path: "defcons" },
        options,
      ),
    ).resolves.toMatchObject({ moved: 0, unchanged: 2 });
    expect(mocks.executeTransaction).toHaveBeenCalledTimes(2);
  });

  it("can remove folder assignment", async () => {
    addMeeting("first", "defcons");
    await expect(
      moveTool.execute!({ meeting_ids: ["first"], folder_path: "" }, options),
    ).resolves.toMatchObject({ status: "ok", moved: 1 });
    expect(folderFor("first")).toBe("");
  });

  it.each(["missing", "DEFCONS", "../escape"])(
    "does not change meetings for unresolved destination %j",
    async (folder_path) => {
      addMeeting("first", "Work");
      await expect(
        moveTool.execute!({ meeting_ids: ["first"], folder_path }, options),
      ).resolves.toMatchObject({ status: "error", results: [] });
      expect(folderFor("first")).toBe("Work");
      expect(mocks.executeTransaction).not.toHaveBeenCalled();
    },
  );

  it("reports missing and deleted meetings without treating the whole batch as successful", async () => {
    addMeeting("first");
    addMeeting("deleted", "Work", "2026-09-01");
    await expect(
      moveTool.execute!(
        {
          meeting_ids: ["first", "missing", "deleted"],
          folder_path: "defcons",
        },
        options,
      ),
    ).resolves.toMatchObject({
      status: "partial",
      moved: 1,
      failed: 2,
      results: [
        { meeting_id: "first", status: "moved" },
        { meeting_id: "missing", status: "error" },
        { meeting_id: "deleted", status: "error" },
      ],
    });
    expect(folderFor("deleted")).toBe("Work");
  });

  it("continues after a failed write and reports the exact failed meeting", async () => {
    addMeeting("failed", "Work");
    addMeeting("success");
    mocks.executeTransaction.mockRejectedValueOnce(new Error("disk full"));
    await expect(
      moveTool.execute!(
        { meeting_ids: ["failed", "success"], folder_path: "defcons" },
        options,
      ),
    ).resolves.toMatchObject({
      status: "partial",
      moved: 1,
      failed: 1,
      results: [
        { meeting_id: "failed", status: "error", message: "disk full" },
        { meeting_id: "success", status: "moved" },
      ],
    });
    expect(folderFor("failed")).toBe("Work");
    expect(folderFor("success")).toBe("defcons");
  });

  it("does not claim a move succeeded when the write affected no meeting", async () => {
    addMeeting("first");
    mocks.executeTransaction.mockResolvedValueOnce([0]);
    await expect(
      moveTool.execute!(
        { meeting_ids: ["first"], folder_path: "defcons" },
        options,
      ),
    ).resolves.toMatchObject({ status: "error", moved: 0, failed: 1 });
  });

  it("bounds each batch and rejects empty IDs", () => {
    for (const meeting_ids of [
      [],
      [" "],
      Array.from({ length: 201 }, (_, i) => String(i)),
    ]) {
      expect(
        (moveTool.inputSchema as z.ZodType).safeParse({
          meeting_ids,
          folder_path: "defcons",
        }).success,
      ).toBe(false);
    }
  });

  it("stops further moves when the user cancels a batch", async () => {
    addMeeting("first");
    addMeeting("second");
    const controller = new AbortController();
    mocks.executeTransaction.mockImplementationOnce(async () => {
      database
        .prepare(
          "UPDATE sessions SET folder_path = 'defcons' WHERE id = 'first'",
        )
        .run();
      controller.abort();
      return [1];
    });
    await expect(
      moveTool.execute!(
        { meeting_ids: ["first", "second"], folder_path: "defcons" },
        { ...options, abortSignal: controller.signal },
      ),
    ).resolves.toMatchObject({ status: "partial", moved: 1, failed: 1 });
    expect(folderFor("first")).toBe("defcons");
    expect(folderFor("second")).toBe("");
    expect(mocks.executeTransaction).toHaveBeenCalledOnce();
  });
});
