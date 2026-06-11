import { createMergeableStore } from "tinybase/with-schemas";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA as MAIN_SCHEMA } from "@hypr/store";

import {
  cleanupExpiredAudio,
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
  sessionAudioExpired,
} from "./audio-retention";

import type * as main from "~/store/tinybase/store/main";
import { SCHEMA as SETTINGS_SCHEMA } from "~/store/tinybase/store/settings";
import type * as settings from "~/store/tinybase/store/settings";

const { audioDeleteMock, audioDeleteOrphanedExpiredMock, getSessionModeMock } =
  vi.hoisted(() => ({
    audioDeleteMock: vi.fn(),
    audioDeleteOrphanedExpiredMock: vi.fn(),
    getSessionModeMock: vi.fn(),
  }));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioDelete: audioDeleteMock,
    audioDeleteOrphanedExpired: audioDeleteOrphanedExpiredMock,
  },
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({
      getSessionMode: getSessionModeMock,
    }),
  },
}));

function createMainStore() {
  return createMergeableStore()
    .setTablesSchema(MAIN_SCHEMA.table)
    .setValuesSchema(MAIN_SCHEMA.value) as main.Store;
}

function createSettingsStore() {
  return createMergeableStore()
    .setTablesSchema(SETTINGS_SCHEMA.table)
    .setValuesSchema(SETTINGS_SCHEMA.value) as settings.Store;
}

describe("audio retention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    audioDeleteMock.mockResolvedValue({ status: "ok", data: null });
    audioDeleteOrphanedExpiredMock.mockResolvedValue({
      status: "ok",
      data: [],
    });
    getSessionModeMock.mockReturnValue("inactive");
  });

  test("normalizes current and legacy values", () => {
    expect(normalizeAudioRetention("none")).toBe("none");
    expect(normalizeAudioRetention("oneWeek")).toBe("oneWeek");
    expect(normalizeAudioRetention("forever")).toBe("forever");
    expect(normalizeAudioRetention(false)).toBe("none");
    expect(normalizeAudioRetention(true)).toBe("forever");
    expect(normalizeAudioRetention("invalid")).toBe("forever");
    expect(normalizeAudioRetention("invalid", undefined)).toBeUndefined();
  });

  test("expires immediately when retention is none", () => {
    expect(sessionAudioExpired("not-a-date", "none")).toBe(true);
  });

  test("never expires when retention is forever", () => {
    expect(sessionAudioExpired("2026-01-01T00:00:00.000Z", "forever")).toBe(
      false,
    );
    expect(sessionAudioExpired("not-a-date", "forever")).toBe(false);
  });

  test("expires after the selected retention window", () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");

    expect(sessionAudioExpired("2026-05-11T23:59:59.999Z", "oneDay", now)).toBe(
      true,
    );
    expect(sessionAudioExpired("2026-05-12T00:00:00.001Z", "oneDay", now)).toBe(
      false,
    );
    expect(
      sessionAudioExpired("2026-05-09T23:59:59.999Z", "threeDays", now),
    ).toBe(true);
    expect(
      sessionAudioExpired("2026-05-10T00:00:00.001Z", "threeDays", now),
    ).toBe(false);
    expect(
      sessionAudioExpired("2026-05-05T23:59:59.999Z", "oneWeek", now),
    ).toBe(true);
    expect(
      sessionAudioExpired("2026-05-06T00:00:00.001Z", "oneWeek", now),
    ).toBe(false);
    expect(
      sessionAudioExpired("2026-04-12T23:59:59.999Z", "oneMonth", now),
    ).toBe(true);
    expect(
      sessionAudioExpired("2026-04-13T00:00:00.001Z", "oneMonth", now),
    ).toBe(false);
  });

  test("does not expire sessions with invalid creation dates", () => {
    expect(sessionAudioExpired(null, "oneDay")).toBe(false);
    expect(sessionAudioExpired("not-a-date", "oneDay")).toBe(false);
  });

  test("deletes expired inactive audio and orphaned expired audio", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("audio_retention", "oneDay");
    store.setRow("sessions", "expired", {
      user_id: "user",
      created_at: "2026-05-11T23:59:59.999Z",
      title: "",
      raw_md: "",
    });
    store.setRow("sessions", "fresh", {
      user_id: "user",
      created_at: "2026-05-12T00:00:00.001Z",
      title: "",
      raw_md: "",
    });
    store.setRow("sessions", "active", {
      user_id: "user",
      created_at: "2026-05-11T23:59:59.999Z",
      title: "",
      raw_md: "",
    });

    getSessionModeMock.mockImplementation((sessionId) =>
      sessionId === "active" ? "active" : "inactive",
    );
    audioDeleteOrphanedExpiredMock.mockResolvedValue({
      status: "ok",
      data: ["orphan"],
    });

    const deleted = await cleanupExpiredAudio(store, settingsStore, now);

    expect(audioDeleteMock).toHaveBeenCalledTimes(1);
    expect(audioDeleteMock).toHaveBeenCalledWith("expired");
    expect(audioDeleteOrphanedExpiredMock).toHaveBeenCalledWith(
      ["expired", "fresh", "active"],
      24 * 60 * 60 * 1000,
      now,
    );
    expect(deleted).toEqual(["expired", "orphan"]);
  });

  test("keeps unsaved audio when retention is none until transcript words exist", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("audio_retention", "none");
    store.setRow("sessions", "unprocessed", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      title: "",
      raw_md: "",
    });
    store.setRow("sessions", "processed", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      title: "",
      raw_md: "",
    });
    store.setRow("transcripts", "processed-transcript", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      session_id: "processed",
      started_at: now,
      words: JSON.stringify([{ text: " saved" }]),
      speaker_hints: "[]",
      memo_md: "",
    });

    const deleted = await cleanupExpiredAudio(store, settingsStore, now);

    expect(audioDeleteMock).toHaveBeenCalledTimes(1);
    expect(audioDeleteMock).toHaveBeenCalledWith("processed");
    expect(deleted).toEqual(["processed"]);
  });

  test("deletes processed audio immediately when retention is none", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("audio_retention", "none");
    store.setRow("sessions", "processed", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      title: "",
      raw_md: "",
    });
    store.setRow("transcripts", "processed-transcript", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      session_id: "processed",
      started_at: now,
      words: JSON.stringify([{ text: " saved" }]),
      speaker_hints: "[]",
      memo_md: "",
    });

    const deleted = await deleteProcessedAudioForRetention(
      store,
      settingsStore,
      "processed",
    );

    expect(deleted).toBe(true);
    expect(audioDeleteMock).toHaveBeenCalledTimes(1);
    expect(audioDeleteMock).toHaveBeenCalledWith("processed");
  });

  test("does not delete unprocessed audio immediately when retention is none", async () => {
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("audio_retention", "none");
    store.setRow("sessions", "unprocessed", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      title: "",
      raw_md: "",
    });

    const deleted = await deleteProcessedAudioForRetention(
      store,
      settingsStore,
      "unprocessed",
    );

    expect(deleted).toBe(false);
    expect(audioDeleteMock).not.toHaveBeenCalled();
  });

  test("does not delete processed audio immediately when retention is not none", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("audio_retention", "oneDay");
    store.setRow("sessions", "processed", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      title: "",
      raw_md: "",
    });
    store.setRow("transcripts", "processed-transcript", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      session_id: "processed",
      started_at: now,
      words: JSON.stringify([{ text: " saved" }]),
      speaker_hints: "[]",
      memo_md: "",
    });

    const deleted = await deleteProcessedAudioForRetention(
      store,
      settingsStore,
      "processed",
    );

    expect(deleted).toBe(false);
    expect(audioDeleteMock).not.toHaveBeenCalled();
  });

  test("does not delete session or orphaned audio when retention is forever", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("audio_retention", "forever");
    store.setRow("sessions", "old", {
      user_id: "user",
      created_at: "2026-01-01T00:00:00.000Z",
      title: "",
      raw_md: "",
    });

    const deleted = await cleanupExpiredAudio(store, settingsStore, now);

    expect(audioDeleteMock).not.toHaveBeenCalled();
    expect(audioDeleteOrphanedExpiredMock).not.toHaveBeenCalled();
    expect(deleted).toEqual([]);
  });

  test("uses legacy save_recordings when audio_retention is missing", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("save_recordings", false);
    store.setRow("sessions", "processed", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      title: "",
      raw_md: "",
    });
    store.setRow("transcripts", "processed-transcript", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      session_id: "processed",
      started_at: now,
      words: JSON.stringify([{ text: " saved" }]),
      speaker_hints: "[]",
      memo_md: "",
    });

    const deleted = await cleanupExpiredAudio(store, settingsStore, now);

    expect(audioDeleteMock).toHaveBeenCalledTimes(1);
    expect(audioDeleteMock).toHaveBeenCalledWith("processed");
    expect(deleted).toEqual(["processed"]);
  });

  test("prefers explicit audio_retention over legacy save_recordings", async () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");
    const store = createMainStore();
    const settingsStore = createSettingsStore();

    settingsStore.setValue("save_recordings", false);
    settingsStore.setValue("audio_retention", "oneMonth");
    store.setRow("sessions", "fresh", {
      user_id: "user",
      created_at: "2026-05-01T00:00:00.000Z",
      title: "",
      raw_md: "",
    });
    store.setRow("transcripts", "fresh-transcript", {
      user_id: "user",
      created_at: "2026-05-13T00:00:00.000Z",
      session_id: "fresh",
      started_at: now,
      words: JSON.stringify([{ text: " saved" }]),
      speaker_hints: "[]",
      memo_md: "",
    });

    const deleted = await cleanupExpiredAudio(store, settingsStore, now);

    expect(audioDeleteMock).not.toHaveBeenCalled();
    expect(audioDeleteOrphanedExpiredMock).toHaveBeenCalledWith(
      ["fresh"],
      30 * 24 * 60 * 60 * 1000,
      now,
    );
    expect(deleted).toEqual([]);
  });
});
