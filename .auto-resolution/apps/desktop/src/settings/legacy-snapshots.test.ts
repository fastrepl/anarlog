import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  load: vi.fn(),
  getTinybaseValues: vi.fn(),
  executeTransaction: vi.fn(
    async (_statements: Array<{ sql: string; params: unknown[] }>) => [1, 1],
  ),
}));

vi.mock("@hypr/plugin-settings", () => ({
  commands: { load: mocks.load },
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: { getTinybaseValues: mocks.getTinybaseValues },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
}));

import {
  LEGACY_MAIN_VALUES_ID,
  LEGACY_SETTINGS_ID,
  refreshLegacySettingsSnapshots,
} from "./legacy-snapshots";

describe("legacy settings snapshots", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.load.mockResolvedValue({ status: "error", error: "missing" });
    mocks.getTinybaseValues.mockResolvedValue({ status: "ok", data: null });
  });

  it("copies both legacy sources into SQLite in one transaction", async () => {
    mocks.load.mockResolvedValue({
      status: "ok",
      data: { general: { theme: "dark" } },
    });
    mocks.getTinybaseValues.mockResolvedValue({
      status: "ok",
      data: JSON.stringify({
        ignored_events: JSON.stringify([{ tracking_id: "event-1" }]),
      }),
    });

    await refreshLegacySettingsSnapshots();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements.map((statement) => statement.params[0])).toEqual([
      LEGACY_SETTINGS_ID,
      LEGACY_MAIN_VALUES_ID,
    ]);
    expect(JSON.parse(String(statements[0].params[1]))).toEqual({
      general: { theme: "dark" },
    });
    expect(JSON.parse(String(statements[1].params[1]))).toEqual({
      ignored_events: JSON.stringify([{ tracking_id: "event-1" }]),
    });
    expect(statements[0].sql).toContain(
      "WHERE app_settings.value_json IS NOT excluded.value_json",
    );
  });

  it("keeps a valid source when the other source is corrupt", async () => {
    mocks.load.mockResolvedValue({
      status: "ok",
      data: { notification: { event: false } },
    });
    mocks.getTinybaseValues.mockResolvedValue({
      status: "ok",
      data: "not-json",
    });

    await refreshLegacySettingsSnapshots();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(1);
    expect(statements[0].params[0]).toBe(LEGACY_SETTINGS_ID);
  });

  it("does not write when neither legacy source is usable", async () => {
    mocks.load.mockRejectedValue(new Error("unavailable"));
    mocks.getTinybaseValues.mockResolvedValue({
      status: "ok",
      data: JSON.stringify([]),
    });

    await refreshLegacySettingsSnapshots();

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });
});
