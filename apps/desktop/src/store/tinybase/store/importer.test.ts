import { createMergeableStore } from "tinybase/with-schemas";
import { type Mock, beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA } from "@hypr/store";

import { importData } from "./importer";
import type { Store } from "./main";

function createTestStore(): Store {
  return createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value) as Store;
}

describe("importData", () => {
  let store: Store;
  let onPersistComplete: Mock<() => Promise<void>>;

  beforeEach(() => {
    vi.clearAllMocks();
    store = createTestStore();
    onPersistComplete = vi.fn().mockResolvedValue(undefined);
  });

  test("successfully imports data", async () => {
    const data = [
      {
        folders: {
          "folder-1": {
            user_id: "user",
            created_at: "2024-01-01",
            name: "Test",
          },
        },
      },
      {},
    ];

    const result = await importData(store, data, onPersistComplete);

    expect(result).toEqual({
      status: "success",
      rowsImported: 1,
      valuesImported: 0,
    });
    expect(onPersistComplete).toHaveBeenCalledTimes(1);
  });

  test("returns error for invalid format - not array", async () => {
    const result = await importData(store, {}, onPersistComplete);

    expect(result.status).toBe("error");
    expect((result as { error: string }).error).toContain(
      "expected [tables, values] array",
    );
    expect(onPersistComplete).not.toHaveBeenCalled();
  });

  test("returns error for invalid format - wrong array length", async () => {
    const result = await importData(store, [1, 2, 3], onPersistComplete);

    expect(result.status).toBe("error");
    expect((result as { error: string }).error).toContain(
      "expected [tables, values] array",
    );
  });

  test("returns error when tables is not object or null", async () => {
    const result = await importData(store, ["invalid", {}], onPersistComplete);

    expect(result.status).toBe("error");
    expect((result as { error: string }).error).toContain(
      "tables must be an object or null",
    );
  });

  test("returns error when values is not object or null", async () => {
    const result = await importData(store, [{}, "invalid"], onPersistComplete);

    expect(result.status).toBe("error");
    expect((result as { error: string }).error).toContain(
      "values must be an object or null",
    );
  });

  test("handles null tables and values", async () => {
    const result = await importData(store, [null, null], onPersistComplete);

    expect(result).toEqual({
      status: "success",
      rowsImported: 0,
      valuesImported: 0,
    });
  });

  test("merges data into existing store", async () => {
    store.setValues({ current_llm_provider: "existing" });

    const data = [{}, { current_stt_provider: "new" }];

    const result = await importData(store, data, onPersistComplete);

    expect(result.status).toBe("success");
    expect(store.getValue("current_llm_provider")).toBe("existing");
    expect(store.getValue("current_stt_provider")).toBe("new");
  });

  test("onPersistComplete is called after merge", async () => {
    let persistCompleted = false;
    const deferredPersist = vi.fn().mockImplementation(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
      persistCompleted = true;
    });

    await importData(store, [{}, {}], deferredPersist);

    expect(deferredPersist).toHaveBeenCalledTimes(1);
    expect(persistCompleted).toBe(true);
  });
});
