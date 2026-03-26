import { createMergeableStore } from "tinybase/with-schemas";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  settingsToContent,
  storeToSettings,
  storeValuesToSettings,
} from "./transform";

import { createTestSettingsStore } from "~/store/tinybase/persister/testing/mocks";
import { SCHEMA } from "~/store/tinybase/store/settings";

type FileChangeCallback = (event: { payload: { path: string } }) => void;

const { notifyListen, notifyUnlisten, settingsLoad, settingsSave, mockState } =
  vi.hoisted(() => {
    const state = {
      notifyCallback: null as FileChangeCallback | null,
      settingsLoadData: {} as unknown,
    };

    const notifyUnlisten = vi.fn();
    const notifyListen = vi
      .fn()
      .mockImplementation((cb: FileChangeCallback) => {
        state.notifyCallback = cb;
        return Promise.resolve(notifyUnlisten);
      });

    const settingsLoad = vi
      .fn()
      .mockImplementation(() =>
        Promise.resolve({ status: "ok", data: state.settingsLoadData }),
      );
    const settingsSave = vi
      .fn()
      .mockImplementation(() => Promise.resolve({ status: "ok", data: null }));

    return {
      notifyListen,
      notifyUnlisten,
      settingsLoad,
      settingsSave,
      mockState: state,
    };
  });

vi.mock("@hypr/plugin-notify", () => ({
  events: {
    fileChanged: {
      listen: notifyListen,
    },
  },
}));

vi.mock("@hypr/plugin-settings", () => ({
  commands: {
    load: settingsLoad,
    save: settingsSave,
  },
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    getPreferredLanguages: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: [] }),
  },
}));

describe("settingsPersister roundtrip", () => {
  test("settings -> store -> settings preserves all data", () => {
    const original = {
      ai: {
        llm: {
          openai: {
            base_url: "https://api.openai.com",
            api_key: "sk-123",
          },
          anthropic: {
            base_url: "https://api.anthropic.com",
            api_key: "sk-456",
          },
        },
        stt: {
          deepgram: {
            base_url: "https://api.deepgram.com",
            api_key: "dg-789",
          },
        },
        current_llm_provider: "openai",
        current_llm_model: "gpt-4",
        current_stt_provider: "deepgram",
        current_stt_model: "nova-2",
      },
      notification: {
        event: true,
        detect: false,
        respect_dnd: true,
        ignored_platforms: ["zoom", "slack"],
        included_platforms: ["code"],
      },
      general: {
        autostart: true,
        save_recordings: false,
        telemetry_consent: false,
      },
      language: {
        ai_language: "en",
        spoken_languages: ["en", "ko"],
      },
    };

    const [tables, values] = settingsToContent(original);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    const expected = { ...original, cactus: {} };
    // storeToSettings omits values that equal schema defaults
    delete (expected as any).notification.event;
    expect(result).toEqual(expected);
  });

  test("store -> settings -> store preserves all data", () => {
    const store1 = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);

    store1.setTables({
      ai_providers: {
        "llm:openai": {
          type: "llm",
          base_url: "https://api.openai.com",
          api_key: "sk-123",
        },
        "llm:anthropic": {
          type: "llm",
          base_url: "https://api.anthropic.com",
          api_key: "sk-456",
        },
        "stt:deepgram": {
          type: "stt",
          base_url: "https://api.deepgram.com",
          api_key: "dg-789",
        },
      },
    });
    store1.setValues({
      current_llm_provider: "openai",
      current_llm_model: "gpt-4",
      current_stt_provider: "deepgram",
      current_stt_model: "nova-2",
      notification_event: true,
      notification_detect: false,
      respect_dnd: true,
      ignored_platforms: '["zoom"]',
      included_platforms: '["code"]',
      autostart: true,
      save_recordings: false,
      telemetry_consent: false,
      ai_language: "en",
      spoken_languages: '["en","ko"]',
      mic_active_threshold: 15,
    });

    const settings = storeToSettings(store1);
    const [tables, values] = settingsToContent(settings);

    const store2 = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store2.setTables(tables);
    store2.setValues(values);

    expect(store2.getTables()).toEqual(store1.getTables());
    expect(store2.getValues()).toEqual(store1.getValues());
  });

  test("handles empty data", () => {
    const original = {};

    const [tables, values] = settingsToContent(original);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result).toEqual({
      ai: { llm: {}, stt: {} },
      cactus: {},
      notification: {},
      general: {},
      language: {},
    });
  });

  test("handles partial data - only ai settings", () => {
    const original = {
      ai: {
        llm: {
          openai: {
            base_url: "https://api.openai.com",
            api_key: "sk-123",
          },
        },
        stt: {},
        current_llm_provider: "openai",
      },
    };

    const [tables, values] = settingsToContent(original);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store) as typeof original & {
      ai: { llm: unknown; stt: unknown };
    };

    expect(result.ai?.llm).toEqual(original.ai?.llm);
    expect(result.ai?.current_llm_provider).toEqual(
      original.ai?.current_llm_provider,
    );
  });

  test("handles partial data - only notification settings", () => {
    const original = {
      // different values from defaults
      notification: {
        event: false,
        respect_dnd: true,
      },
    };

    const [tables, values] = settingsToContent(original);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.notification).toEqual(original.notification);
  });

  test("handles partial data - only general settings", () => {
    const original = {
      general: {
        autostart: true,
      },
    };

    const [tables, values] = settingsToContent(original);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.general).toEqual(original.general);
  });

  test("handles partial data - only language settings", () => {
    const original = {
      language: {
        ai_language: "ko",
        spoken_languages: ["ko", "en"],
      },
    };

    const [tables, values] = settingsToContent(original);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.language).toEqual(original.language);
  });

  test("handles migration from double-encoded JSON strings", () => {
    const doubleEncoded = {
      language: {
        spoken_languages: '["en","ko"]',
      },
      notification: {
        ignored_platforms: '["zoom"]',
        included_platforms: '["code"]',
      },
    };

    const [tables, values] = settingsToContent(doubleEncoded);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.language).toEqual({
      spoken_languages: ["en", "ko"],
    });
    expect(result.notification).toEqual({
      ignored_platforms: ["zoom"],
      included_platforms: ["code"],
    });
  });

  test("handles migration from comma-separated strings (old format)", () => {
    const oldFormat = {
      language: {
        spoken_languages: "en,ko,ja",
      },
      notification: {
        ignored_platforms: "zoom,slack",
        included_platforms: "code,terminal",
      },
    };

    const [tables, values] = settingsToContent(oldFormat);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.language).toEqual({
      spoken_languages: ["en", "ko", "ja"],
    });
    expect(result.notification).toEqual({
      ignored_platforms: ["zoom", "slack"],
      included_platforms: ["code", "terminal"],
    });
  });

  test("handles migration from general section to language section", () => {
    const oldSettings = {
      general: {
        autostart: true,
        ai_language: "ko",
        spoken_languages: ["ko", "en"],
      },
    };

    const [tables, values] = settingsToContent(oldSettings);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.language).toEqual({
      ai_language: "ko",
      spoken_languages: ["ko", "en"],
    });
    expect(result.general).toEqual({
      autostart: true,
    });
  });

  test("handles migration from general section with comma-separated spoken_languages", () => {
    const oldSettings = {
      general: {
        ai_language: "ja",
        spoken_languages: "ja,en,ko",
      },
    };

    const [tables, values] = settingsToContent(oldSettings);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.language).toEqual({
      ai_language: "ja",
      spoken_languages: ["ja", "en", "ko"],
    });
  });

  test("storeToSettings omits values that equal schema defaults", () => {
    const [tables, values] = settingsToContent({});
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.general).toEqual({});
    expect(result.notification).toEqual({});
    expect(result.language).toEqual({});
  });

  test("storeToSettings keeps non-default values and omits default ones", () => {
    const [tables, values] = settingsToContent({
      general: {
        autostart: true,
        save_recordings: true,
      },
      notification: {
        event: false,
        respect_dnd: false,
      },
    });
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.general).toEqual({ autostart: true });
    expect(result.notification).toEqual({ event: false });
  });

  test("storeValuesToSettings clears language values matching OS locale defaults", () => {
    const [tables, values] = settingsToContent({
      language: {
        ai_language: "pl",
        spoken_languages: ["pl", "en"],
      },
    });
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);

    const storeValues = store.getValues();
    const result = storeValuesToSettings(
      storeValues as Record<string, unknown>,
      { ai_language: "pl", spoken_languages: ["pl", "en"] },
    );

    expect(result.language).toEqual({});
  });

  test("storeValuesToSettings keeps language values differing from OS locale defaults", () => {
    const [tables, values] = settingsToContent({
      language: {
        ai_language: "ko",
        spoken_languages: ["ko", "en"],
      },
    });
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);

    const storeValues = store.getValues();
    const result = storeValuesToSettings(
      storeValues as Record<string, unknown>,
      { ai_language: "pl", spoken_languages: ["pl"] },
    );

    expect(result.language).toEqual({
      ai_language: "ko",
      spoken_languages: ["ko", "en"],
    });
  });

  test("storeValuesToSettings clears language values matching OS locale defaults by prefix", () => {
    const [tables, values] = settingsToContent({
      language: {
        ai_language: "ko",
        spoken_languages: ["ko", "en"],
      },
    });
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);

    const storeValues = store.getValues();
    const result = storeValuesToSettings(
      storeValues as Record<string, unknown>,
      { ai_language: "ko-KR", spoken_languages: ["ko-KR", "en-US"] },
    );

    expect(result.language).toEqual({});
  });

  test("language section takes precedence over general section", () => {
    const mixedSettings = {
      general: {
        ai_language: "en",
        spoken_languages: ["en"],
      },
      language: {
        ai_language: "ko",
        spoken_languages: ["ko", "ja"],
      },
    };

    const [tables, values] = settingsToContent(mixedSettings);
    const store = createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value);
    store.setTables(tables);
    store.setValues(values);
    const result = storeToSettings(store);

    expect(result.language).toEqual({
      ai_language: "ko",
      spoken_languages: ["ko", "ja"],
    });
  });
});

describe("createSettingsPersister e2e", () => {
  let store: ReturnType<typeof createTestSettingsStore>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockState.notifyCallback = null;
    mockState.settingsLoadData = {};
    store = createTestSettingsStore();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  test("startAutoPersisting loads initial data", async () => {
    mockState.settingsLoadData = {
      general: { autostart: true },
    };

    const { createSettingsPersister } = await import("./persister");
    const persister = createSettingsPersister(store);
    await persister.startAutoPersisting();

    expect(settingsLoad).toHaveBeenCalled();
    expect(store.getValue("autostart")).toBe(true);

    await persister.stopAutoPersisting();
    persister.destroy();
  });

  test("startAutoPersisting auto-saves on store change", async () => {
    mockState.settingsLoadData = {};

    const { createSettingsPersister } = await import("./persister");
    const persister = createSettingsPersister(store);
    await persister.startAutoPersisting();

    settingsSave.mockClear();
    store.setValue("autostart", true);

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(settingsSave).toHaveBeenCalled();
    const savedData = settingsSave.mock.calls[0][0];
    expect(savedData.general?.autostart).toBe(true);

    await persister.stopAutoPersisting();
    persister.destroy();
  });

  test("startAutoPersisting auto-loads on file change event", async () => {
    mockState.settingsLoadData = {};

    const { createSettingsPersister } = await import("./persister");
    const persister = createSettingsPersister(store);
    await persister.startAutoPersisting();

    await new Promise((resolve) => setTimeout(resolve, 10));

    mockState.settingsLoadData = {
      general: { autostart: true, save_recordings: true },
    };
    settingsLoad.mockClear();

    expect(mockState.notifyCallback).not.toBeNull();
    mockState.notifyCallback!({
      payload: { path: "/mock/data/settings.json" },
    });

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(settingsLoad).toHaveBeenCalled();
    expect(store.getValue("autostart")).toBe(true);
    expect(store.getValue("save_recordings")).toBe(true);

    await persister.stopAutoPersisting();
    persister.destroy();
  });

  test("full lifecycle: load, save on change, reload on file event", async () => {
    mockState.settingsLoadData = {
      ai: { current_llm_provider: "openai" },
    };

    const { createSettingsPersister } = await import("./persister");
    const persister = createSettingsPersister(store);
    await persister.startAutoPersisting();

    expect(store.getValue("current_llm_provider")).toBe("openai");

    settingsSave.mockClear();
    store.setValue("current_llm_model", "gpt-4");

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(settingsSave).toHaveBeenCalled();

    mockState.settingsLoadData = {
      ai: {
        current_llm_provider: "anthropic",
        current_llm_model: "claude-3",
      },
    };
    settingsLoad.mockClear();

    mockState.notifyCallback!({
      payload: { path: "/path/to/settings.json" },
    });

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(settingsLoad).toHaveBeenCalled();
    expect(store.getValue("current_llm_provider")).toBe("anthropic");
    expect(store.getValue("current_llm_model")).toBe("claude-3");

    await persister.stopAutoPersisting();
    persister.destroy();
  });

  test("stopAutoPersisting stops responding to store changes", async () => {
    mockState.settingsLoadData = {};

    const { createSettingsPersister } = await import("./persister");
    const persister = createSettingsPersister(store);
    await persister.startAutoPersisting();
    await persister.stopAutoPersisting();

    settingsSave.mockClear();

    store.setValue("autostart", true);

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(settingsSave).not.toHaveBeenCalled();

    persister.destroy();
  });

  test("stopAutoPersisting calls unlisten on file watcher", async () => {
    mockState.settingsLoadData = {};

    const { createSettingsPersister } = await import("./persister");
    const persister = createSettingsPersister(store);
    await persister.startAutoPersisting();

    expect(notifyUnlisten).not.toHaveBeenCalled();

    await persister.stopAutoPersisting();

    expect(notifyUnlisten).toHaveBeenCalled();

    persister.destroy();
  });
});
