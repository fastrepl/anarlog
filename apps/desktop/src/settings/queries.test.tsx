import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  getPreferredLanguages: vi.fn(),
  setProperties: vi.fn(async () => undefined),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    setDisabled: vi.fn(async () => undefined),
    setProperties: mocks.setProperties,
  },
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    getPreferredLanguages: mocks.getPreferredLanguages,
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: vi.fn(() => ({ data: undefined })),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  initializeApplicationSettings,
  parseSettingRows,
  setSettingValues,
  updateSettingValue,
} from "./queries";

describe("SQLite settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.execute.mockResolvedValue([]);
    mocks.getPreferredLanguages.mockResolvedValue({
      status: "error",
      error: "unavailable",
    });
  });

  it("maps the imported settings document into typed values", () => {
    const result = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: {
            theme: "dark",
            save_recordings: false,
          },
          language: {
            spoken_languages: ["en", "ko"],
          },
          notification: {
            ignored_platforms: ["com.example.video"],
          },
        }),
      },
    ]);

    expect(result.values.theme).toBe("dark");
    expect(result.values.audio_retention).toBe("none");
    expect(result.values.spoken_languages).toBe('["en","ko"]');
    expect(result.values.ignored_platforms).toBe('["com.example.video"]');
    expect(result.hasValues.has("theme")).toBe(true);
  });

  it("prefers valid direct rows and falls back from corrupt ones", () => {
    const result = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: { theme: "dark", week_start: "monday" },
        }),
      },
      { id: "theme", value_json: JSON.stringify("light") },
      { id: "week_start", value_json: "not-json" },
    ]);

    expect(result.values.theme).toBe("light");
    expect(result.values.week_start).toBe("monday");
  });

  it("recovers main-store values after settings document values and aliases", () => {
    const result = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: { ai_language: "fr" },
          language: { spoken_languages: ["fr"] },
        }),
      },
      {
        id: "legacy_main_values_document",
        value_json: JSON.stringify({
          ai_language: "ko",
          spoken_languages: JSON.stringify(["ko"]),
          theme: "dark",
        }),
      },
    ]);

    expect(result.values.ai_language).toBe("fr");
    expect(result.values.spoken_languages).toBe('["fr"]');
    expect(result.values.theme).toBe("dark");
  });

  it("writes multiple independent values in one transaction", async () => {
    await setSettingValues({
      theme: "dark",
      notification_event: false,
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("INSERT INTO app_settings");
    expect(statements[0].sql).toContain("ON CONFLICT(id) DO UPDATE");
    expect(statements[0].params.slice(0, 2)).toEqual([
      "theme",
      JSON.stringify("dark"),
    ]);
    expect(statements[1].params.slice(0, 2)).toEqual([
      "notification_event",
      JSON.stringify(false),
    ]);
  });

  it("migrates and persists the consent chat auto-send setting", async () => {
    const imported = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: { consent_auto_send_chat: true },
        }),
      },
    ]);

    expect(imported.values.consent_auto_send_chat).toBe(true);

    await setSettingValues({ consent_auto_send_chat: false });

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.params.slice(0, 2)).toEqual([
      "consent_auto_send_chat",
      JSON.stringify(false),
    ]);
  });

  it("persists OS language defaults only when no stored values exist", async () => {
    let rows: Array<{ id: string; value_json: string }> = [];
    mocks.execute.mockImplementation(async () => rows);
    mocks.executeTransaction.mockImplementation(async (statements) => {
      rows = statements.map((statement) => ({
        id: String(statement.params[0]),
        value_json: String(statement.params[1]),
      }));
      return statements.map(() => 1);
    });
    mocks.getPreferredLanguages.mockResolvedValue({
      status: "ok",
      data: ["ko", "en"],
    });

    await initializeApplicationSettings();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements.map((statement) => statement.params.slice(0, 2))).toEqual(
      [
        ["ai_language", JSON.stringify("ko")],
        ["spoken_languages", JSON.stringify(JSON.stringify(["ko", "en"]))],
      ],
    );
  });

  it("repairs a selected external transcription provider with no model", async () => {
    let rows = [
      {
        id: "current_stt_provider",
        value_json: JSON.stringify("deepgram"),
      },
      { id: "current_stt_model", value_json: JSON.stringify("") },
    ];
    mocks.execute.mockImplementation(async () => rows);
    mocks.executeTransaction.mockImplementation(async (statements) => {
      rows = statements.map((statement) => ({
        id: String(statement.params[0]),
        value_json: String(statement.params[1]),
      }));
      return statements.map(() => 1);
    });

    await initializeApplicationSettings();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements.map((statement) => statement.params.slice(0, 2))).toEqual(
      [["current_stt_model", JSON.stringify("nova-3-general")]],
    );
  });

  it("updates against the latest SQLite value inside the write queue", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "personalization_dictionary_terms",
        value_json: JSON.stringify(JSON.stringify(["Anarlog"])),
      },
    ]);

    const next = await updateSettingValue(
      "personalization_dictionary_terms",
      (current) => JSON.stringify([...JSON.parse(current ?? "[]"), "Erebor"]),
    );

    expect(next).toBe(JSON.stringify(["Anarlog", "Erebor"]));
    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.params.slice(0, 2)).toEqual([
      "personalization_dictionary_terms",
      JSON.stringify(next),
    ]);
  });
});
