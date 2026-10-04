import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(),
  rowsById: {} as Record<string, Array<Record<string, unknown>>>,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  executeTransaction: mocks.executeTransaction,
  useLiveQuery: (options: {
    params: unknown[];
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => ({
    data: options.mapRows(mocks.rowsById[String(options.params[0])] ?? []),
  }),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  clearSeriesFolderRule,
  clearSeriesFolderRulesForFolder,
  declineSeriesFolderRule,
  getSeriesFolderRule,
  remapSeriesFolderRules,
  setSeriesFolderRule,
  useDeclinedSeriesFolderIds,
  useSeriesFolderRules,
} from "./series-folders";

function settingRows(rules: unknown): Array<Record<string, unknown>> {
  return [{ value_json: JSON.stringify(rules) }];
}

function writtenJson(call: { params: unknown[] }): unknown {
  return JSON.parse(call.params[1] as string);
}

function writesFor(id: string): Array<{ params: unknown[] }> {
  return mocks.executeTransaction.mock.calls
    .flatMap((call) => call[0] as Array<{ params: unknown[] }>)
    .filter((statement) => statement.params[0] === id);
}

describe("series folder auto-add rules", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rowsById = {};
    mocks.execute.mockImplementation(
      async (_sql: string, params: string[]) => mocks.rowsById[params[0]] ?? [],
    );
  });

  it("reads rules from app_settings and skips invalid entries", () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Work" },
      { series_id: "", folder_path: "Work" },
      { series_id: "series-2", folder_path: "" },
      "junk",
    ]);

    const { result } = renderHook(() => useSeriesFolderRules());

    expect(result.current).toEqual([
      { series_id: "series-1", folder_path: "Work" },
    ]);
  });

  it("returns null for a series without a rule", async () => {
    await expect(getSeriesFolderRule("series-9")).resolves.toBeNull();
    await expect(getSeriesFolderRule("")).resolves.toBeNull();
  });

  it("setSeriesFolderRule replaces the existing entry for the series", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Old" },
      { series_id: "series-2", folder_path: "Keep" },
    ]);

    await setSeriesFolderRule("series-1", "New");

    expect(writesFor("auto_add_series_folders").map(writtenJson)).toEqual([
      [
        { series_id: "series-2", folder_path: "Keep" },
        { series_id: "series-1", folder_path: "New" },
      ],
    ]);
  });

  it("setSeriesFolderRule does not refile existing unfiled sessions of the series", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([]);

    await setSeriesFolderRule("series-1", "Work");

    const statements = mocks.executeTransaction.mock.calls.flatMap(
      (call) => call[0] as Array<{ sql: string }>,
    );
    expect(statements.some(({ sql }) => /update\s+sessions/i.test(sql))).toBe(
      false,
    );
  });

  it("clearSeriesFolderRule removes only the matching series", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Work" },
      { series_id: "series-2", folder_path: "Keep" },
    ]);

    await clearSeriesFolderRule("series-1");

    expect(writesFor("auto_add_series_folders").map(writtenJson)).toEqual([
      [{ series_id: "series-2", folder_path: "Keep" }],
    ]);
  });

  it("clearSeriesFolderRule skips the write when the series has no rule", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Work" },
    ]);

    await clearSeriesFolderRule("series-9");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("remapSeriesFolderRules follows folder renames including nested paths", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Work" },
      { series_id: "series-2", folder_path: "Work/Clients" },
      { series_id: "series-3", folder_path: "Personal" },
    ]);

    await remapSeriesFolderRules("Work", "Job");

    expect(writesFor("auto_add_series_folders").map(writtenJson)).toEqual([
      [
        { series_id: "series-1", folder_path: "Job" },
        { series_id: "series-2", folder_path: "Job/Clients" },
        { series_id: "series-3", folder_path: "Personal" },
      ],
    ]);
  });

  it("remapSeriesFolderRules skips the write when no rule references the folder", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Personal" },
    ]);

    await remapSeriesFolderRules("Work", "Job");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("clearSeriesFolderRulesForFolder drops rules for a deleted folder and its children", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Work" },
      { series_id: "series-2", folder_path: "Work/Clients" },
      { series_id: "series-3", folder_path: "Personal" },
    ]);

    await clearSeriesFolderRulesForFolder("Work");

    expect(writesFor("auto_add_series_folders").map(writtenJson)).toEqual([
      [{ series_id: "series-3", folder_path: "Personal" }],
    ]);
  });

  it("clearSeriesFolderRulesForFolder skips the write when no rule references the folder", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([
      { series_id: "series-1", folder_path: "Personal" },
    ]);

    await clearSeriesFolderRulesForFolder("Work");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });
});

describe("declined series folder ids", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rowsById = {};
    mocks.execute.mockImplementation(
      async (_sql: string, params: string[]) => mocks.rowsById[params[0]] ?? [],
    );
  });

  it("reads declined series ids from app_settings and skips invalid entries", () => {
    mocks.rowsById.auto_add_series_folders_declined = settingRows([
      "series-1",
      "",
      42,
      { series_id: "series-2" },
    ]);

    const { result } = renderHook(() => useDeclinedSeriesFolderIds());

    expect(result.current).toEqual(["series-1"]);
  });

  it("declineSeriesFolderRule persists the series id once", async () => {
    mocks.rowsById.auto_add_series_folders_declined = settingRows(["series-1"]);

    await declineSeriesFolderRule("series-1");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();

    await declineSeriesFolderRule("series-2");
    expect(
      writesFor("auto_add_series_folders_declined").map(writtenJson),
    ).toEqual([["series-1", "series-2"]]);
  });

  it("setSeriesFolderRule clears the decline for that series", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([]);
    mocks.rowsById.auto_add_series_folders_declined = settingRows([
      "series-1",
      "series-2",
    ]);

    await setSeriesFolderRule("series-1", "Work");

    expect(
      writesFor("auto_add_series_folders_declined").map(writtenJson),
    ).toEqual([["series-2"]]);
  });

  it("setSeriesFolderRule leaves declined ids untouched when the series was not declined", async () => {
    mocks.rowsById.auto_add_series_folders = settingRows([]);
    mocks.rowsById.auto_add_series_folders_declined = settingRows(["series-9"]);

    await setSeriesFolderRule("series-1", "Work");

    expect(writesFor("auto_add_series_folders_declined")).toEqual([]);
  });
});
