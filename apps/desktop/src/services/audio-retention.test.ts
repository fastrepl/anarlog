import { createMergeableStore } from "tinybase/with-schemas";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA as MAIN_SCHEMA } from "@hypr/store";

import {
  cleanupExpiredAudio,
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
    expect(normalizeAudioRetention(false)).toBe("none");
    expect(normalizeAudioRetention(true)).toBe("oneMonth");
    expect(normalizeAudioRetention("invalid")).toBe("oneMonth");
    expect(normalizeAudioRetention("invalid", undefined)).toBeUndefined();
  });

  test("expires immediately when retention is none", () => {
    expect(sessionAudioExpired("not-a-date", "none")).toBe(true);
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
});
