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
  getSeriesFolderRule,
  remapSeriesFolderRules,
  setSeriesFolderRule,
  useSeriesFolderRules,
} from "./series-folders";

function settingRows(rules: unknown): Array<Record<string, unknown>> {
  return [{ value_json: JSON.stringify(rules) }];
}

function writtenRules(): unknown {
  const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
    params: unknown[];
  }>;
  return JSON.parse(statements[0].params[1] as string);
}

describe("series folder auto-add rules", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rowsById = {};
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
    mocks.execute.mockResolvedValue(settingRows([]));

    await expect(getSeriesFolderRule("series-9")).resolves.toBeNull();
    await expect(getSeriesFolderRule("")).resolves.toBeNull();
  });

  it("setSeriesFolderRule replaces the existing entry for the series", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([
        { series_id: "series-1", folder_path: "Old" },
        { series_id: "series-2", folder_path: "Keep" },
      ]),
    );

    await setSeriesFolderRule("series-1", "New");

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    expect(writtenRules()).toEqual([
      { series_id: "series-2", folder_path: "Keep" },
      { series_id: "series-1", folder_path: "New" },
    ]);
  });

  it("clearSeriesFolderRule removes only the matching series", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([
        { series_id: "series-1", folder_path: "Work" },
        { series_id: "series-2", folder_path: "Keep" },
      ]),
    );

    await clearSeriesFolderRule("series-1");

    expect(writtenRules()).toEqual([
      { series_id: "series-2", folder_path: "Keep" },
    ]);
  });

  it("clearSeriesFolderRule skips the write when the series has no rule", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([{ series_id: "series-1", folder_path: "Work" }]),
    );

    await clearSeriesFolderRule("series-9");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("remapSeriesFolderRules follows folder renames including nested paths", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([
        { series_id: "series-1", folder_path: "Work" },
        { series_id: "series-2", folder_path: "Work/Clients" },
        { series_id: "series-3", folder_path: "Personal" },
      ]),
    );

    await remapSeriesFolderRules("Work", "Job");

    expect(writtenRules()).toEqual([
      { series_id: "series-1", folder_path: "Job" },
      { series_id: "series-2", folder_path: "Job/Clients" },
      { series_id: "series-3", folder_path: "Personal" },
    ]);
  });

  it("remapSeriesFolderRules skips the write when no rule references the folder", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([{ series_id: "series-1", folder_path: "Personal" }]),
    );

    await remapSeriesFolderRules("Work", "Job");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("clearSeriesFolderRulesForFolder drops rules for a deleted folder and its children", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([
        { series_id: "series-1", folder_path: "Work" },
        { series_id: "series-2", folder_path: "Work/Clients" },
        { series_id: "series-3", folder_path: "Personal" },
      ]),
    );

    await clearSeriesFolderRulesForFolder("Work");

    expect(writtenRules()).toEqual([
      { series_id: "series-3", folder_path: "Personal" },
    ]);
  });

  it("clearSeriesFolderRulesForFolder skips the write when no rule references the folder", async () => {
    mocks.execute.mockResolvedValue(
      settingRows([{ series_id: "series-1", folder_path: "Personal" }]),
    );

    await clearSeriesFolderRulesForFolder("Work");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });
});
