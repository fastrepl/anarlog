import { createMergeableStore } from "tinybase";
import { createCustomPersister } from "tinybase/persisters";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { ChangedTables, TablesContent } from "./types";
import {
  asTablesChanges,
  extractChangedTables,
  iterateTableRows,
} from "./utils";

const MergeableStoreOnly = 2;

describe("extractChangedTables", () => {
  describe("defensive input handling", () => {
    test("returns null for undefined", () => {
      expect(extractChangedTables(undefined)).toBeNull();
    });

    test("returns null for null", () => {
      expect(extractChangedTables(null as any)).toBeNull();
    });

    test("returns null for empty inner array (malformed MergeableChanges)", () => {
      const malformed = [[], [{}, "hlc"], 1] as any;
      expect(extractChangedTables(malformed)).toBeNull();
    });

    test("returns null for array as first element (not valid ChangedTables)", () => {
      const malformed = [["not", "valid"], {}, 1] as any;
      expect(extractChangedTables(malformed)).toBeNull();
    });
  });

  describe("e2e: MergeableStore with persister", () => {
    let store: ReturnType<typeof createMergeableStore>;
    let saveFn: ReturnType<typeof vi.fn>;
    let capturedChangedTables: ChangedTables | null;

    beforeEach(async () => {
      store = createMergeableStore("test-store");
      capturedChangedTables = null;

      saveFn = vi.fn(async (_getContent, changes) => {
        capturedChangedTables = extractChangedTables(changes);
      });

      const persister = createCustomPersister(
        store,
        async () => undefined,
        saveFn,
        () => null,
        () => {},
        undefined,
        MergeableStoreOnly,
      );

      await persister.startAutoSave();

      // startAutoSave() calls save() once without changes (initial full save).
      // Clear mocks so tests only see incremental change saves.
      saveFn.mockClear();
      capturedChangedTables = null;
    });

    describe("basic operations", () => {
      test("single cell change", async () => {
        store.setCell("sessions", "session-1", "title", "Meeting Notes");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());

        expect(capturedChangedTables).toEqual({
          sessions: { "session-1": expect.any(Object) },
        });
      });

      test("multiple cells in same row", async () => {
        store.transaction(() => {
          store.setCell("sessions", "session-1", "title", "Meeting");
          store.setCell("sessions", "session-1", "raw_md", "# Notes");
          store.setCell("sessions", "session-1", "created_at", "2024-01-01");
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());

        expect(capturedChangedTables).toEqual({
          sessions: { "session-1": expect.any(Object) },
        });
        expect(Object.keys(capturedChangedTables!)).toHaveLength(1);
      });

      test("multiple rows in same table", async () => {
        store.transaction(() => {
          store.setCell("sessions", "s1", "title", "Session 1");
          store.setCell("sessions", "s2", "title", "Session 2");
          store.setCell("sessions", "s3", "title", "Session 3");
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());

        expect(capturedChangedTables).toEqual({
          sessions: {
            s1: expect.any(Object),
            s2: expect.any(Object),
            s3: expect.any(Object),
          },
        });
      });

      test("multiple tables in single transaction", async () => {
        store.transaction(() => {
          store.setCell("sessions", "session-1", "title", "Meeting");
          store.setCell("humans", "human-1", "name", "Alice");
          store.setCell(
            "transcripts",
            "transcript-1",
            "session_id",
            "session-1",
          );
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());

        expect(capturedChangedTables).toHaveProperty("sessions");
        expect(capturedChangedTables).toHaveProperty("humans");
        expect(capturedChangedTables).toHaveProperty("transcripts");
        expect(capturedChangedTables!.sessions).toHaveProperty("session-1");
        expect(capturedChangedTables!.humans).toHaveProperty("human-1");
        expect(capturedChangedTables!.transcripts).toHaveProperty(
          "transcript-1",
        );
      });
    });

    describe("deletions", () => {
      test("row deletion", async () => {
        store.setCell("sessions", "session-1", "title", "To Delete");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.delRow("sessions", "session-1");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables).toHaveProperty("sessions");
        expect(capturedChangedTables!.sessions).toHaveProperty("session-1");
      });

      test("cell deletion", async () => {
        store.transaction(() => {
          store.setCell("sessions", "session-1", "title", "Title");
          store.setCell("sessions", "session-1", "raw_md", "Content");
        });
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.delCell("sessions", "session-1", "raw_md");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables).toEqual({
          sessions: { "session-1": expect.any(Object) },
        });
      });

      test("table deletion", async () => {
        store.transaction(() => {
          store.setCell("sessions", "s1", "title", "One");
          store.setCell("sessions", "s2", "title", "Two");
        });
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.delTable("sessions");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables).toHaveProperty("sessions");
      });

      test("delete multiple rows in transaction", async () => {
        store.transaction(() => {
          store.setCell("sessions", "s1", "title", "One");
          store.setCell("sessions", "s2", "title", "Two");
          store.setCell("sessions", "s3", "title", "Three");
        });
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.transaction(() => {
          store.delRow("sessions", "s1");
          store.delRow("sessions", "s3");
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
        expect(capturedChangedTables!.sessions).toHaveProperty("s3");
        expect(capturedChangedTables!.sessions).not.toHaveProperty("s2");
      });
    });

    describe("updates", () => {
      test("update existing cell", async () => {
        store.setCell("sessions", "session-1", "title", "Original");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.setCell("sessions", "session-1", "title", "Updated");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables).toEqual({
          sessions: { "session-1": expect.any(Object) },
        });
      });

      test("setting same value does not trigger save", async () => {
        store.setCell("sessions", "session-1", "title", "Same");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.setCell("sessions", "session-1", "title", "Same");

        await new Promise((r) => setTimeout(r, 50));
        expect(saveFn).not.toHaveBeenCalled();
      });

      test("mixed create/update/delete in single transaction", async () => {
        store.transaction(() => {
          store.setCell("sessions", "existing", "title", "Existing");
          store.setCell("humans", "to-delete", "name", "Delete Me");
        });
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.transaction(() => {
          store.setCell("sessions", "new", "title", "New Session");
          store.setCell("sessions", "existing", "title", "Updated");
          store.delRow("humans", "to-delete");
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("new");
        expect(capturedChangedTables!.sessions).toHaveProperty("existing");
        expect(capturedChangedTables!.humans).toHaveProperty("to-delete");
      });
    });

    describe("cell value types", () => {
      test("string values", async () => {
        store.setCell("sessions", "s1", "title", "Hello World");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
      });

      test("empty string value", async () => {
        store.setCell("sessions", "s1", "title", "");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
      });

      test("boolean values", async () => {
        store.setCell("sessions", "s1", "active", true);

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
      });

      test("number values", async () => {
        store.setCell("sessions", "s1", "count", 42);

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
      });

      test("zero value", async () => {
        store.setCell("sessions", "s1", "count", 0);

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
      });
    });

    describe("transaction behavior", () => {
      test("no-op transaction does not call save", async () => {
        store.transaction(() => {});

        await new Promise((r) => setTimeout(r, 50));
        expect(saveFn).not.toHaveBeenCalled();
      });

      test("sequential transactions produce separate save calls", async () => {
        store.setCell("sessions", "s1", "title", "First");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));

        const firstChanges = capturedChangedTables;
        expect(firstChanges).toEqual({ sessions: { s1: expect.any(Object) } });

        store.setCell("sessions", "s2", "title", "Second");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(2));

        expect(capturedChangedTables).toEqual({
          sessions: { s2: expect.any(Object) },
        });
      });

      test("net-zero change still triggers save (MergeableStore tracks HLC)", async () => {
        store.setCell("sessions", "s1", "title", "Original");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.transaction(() => {
          store.setCell("sessions", "s1", "title", "Temp");
          store.setCell("sessions", "s1", "title", "Original");
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("s1");
      });

      test("create and delete in same transaction still triggers save", async () => {
        store.transaction(() => {
          store.setCell("sessions", "temp", "title", "Temporary");
          store.delRow("sessions", "temp");
        });

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables!.sessions).toHaveProperty("temp");
      });
    });

    describe("isolation between tables", () => {
      test("change to one table does not include other tables", async () => {
        store.transaction(() => {
          store.setCell("sessions", "s1", "title", "Session");
          store.setCell("humans", "h1", "name", "Human");
        });
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        saveFn.mockClear();

        store.setCell("sessions", "s1", "title", "Updated Session");

        await vi.waitFor(() => expect(saveFn).toHaveBeenCalled());
        expect(capturedChangedTables).toHaveProperty("sessions");
        expect(capturedChangedTables).not.toHaveProperty("humans");
      });

      test("changes to different tables in separate transactions", async () => {
        store.setCell("sessions", "s1", "title", "Session");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(1));
        expect(capturedChangedTables).toHaveProperty("sessions");
        expect(capturedChangedTables).not.toHaveProperty("humans");

        store.setCell("humans", "h1", "name", "Human");
        await vi.waitFor(() => expect(saveFn).toHaveBeenCalledTimes(2));
        expect(capturedChangedTables).toHaveProperty("humans");
        expect(capturedChangedTables).not.toHaveProperty("sessions");
      });
    });
  });
});

describe("asTablesChanges", () => {
  test("wraps tables in changes format", () => {
    const tables = {
      sessions: { "session-1": { title: "Test" } },
    };

    const result = asTablesChanges(tables);

    expect(result).toEqual([tables, {}, 1]);
  });

  test("returns tuple with empty values object", () => {
    const tables = {};
    const result = asTablesChanges(tables);

    expect(result[1]).toEqual({});
    expect(result[2]).toBe(1);
  });

  test("handles deletion markers", () => {
    const tables = {
      sessions: { "session-1": undefined },
    };

    const result = asTablesChanges(tables);

    expect(result[0]).toEqual(tables);
  });

  test("handles table-level deletion", () => {
    const tables = {
      sessions: undefined,
    };

    const result = asTablesChanges(tables as any);

    expect(result[0]).toEqual({ sessions: undefined });
  });
});

describe("iterateTableRows", () => {
  test("iterates over table rows and adds id", () => {
    const tables: TablesContent = {
      sessions: {
        "session-1": {
          user_id: "user-1",
          created_at: "2024-01-01",
          title: "Test",
          folder_id: "",
          event_json: "",
          raw_md: "",
        },
        "session-2": {
          user_id: "user-1",
          created_at: "2024-01-02",
          title: "Test 2",
          folder_id: "",
          event_json: "",
          raw_md: "",
        },
      },
    };

    const result = iterateTableRows(tables, "sessions");

    expect(result).toHaveLength(2);
    expect(result.find((r) => r.id === "session-1")).toEqual({
      id: "session-1",
      user_id: "user-1",
      created_at: "2024-01-01",
      title: "Test",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });
    expect(result.find((r) => r.id === "session-2")).toEqual({
      id: "session-2",
      user_id: "user-1",
      created_at: "2024-01-02",
      title: "Test 2",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });
  });

  test("returns empty array for undefined tables", () => {
    const result = iterateTableRows(undefined, "sessions");
    expect(result).toEqual([]);
  });

  test("returns empty array for missing table", () => {
    const tables: TablesContent = {};
    const result = iterateTableRows(tables, "sessions");
    expect(result).toEqual([]);
  });

  test("returns empty array for empty table", () => {
    const tables: TablesContent = {
      sessions: {},
    };
    const result = iterateTableRows(tables, "sessions");
    expect(result).toEqual([]);
  });

  test("handles humans table", () => {
    const tables: TablesContent = {
      humans: {
        "human-1": {
          user_id: "user-1",
          name: "John Doe",
          email: "john@example.com",
          org_id: "",
          job_title: "",
          linkedin_username: "",
          memo: "",
        },
      },
    };

    const result = iterateTableRows(tables, "humans");

    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("human-1");
    expect(result[0].name).toBe("John Doe");
  });
});
