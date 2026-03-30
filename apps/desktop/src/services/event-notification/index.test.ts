import { createMergeableStore } from "tinybase/with-schemas";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA } from "@hypr/store";

const pluginNotification = vi.hoisted(() => ({
  showNotification: vi.fn().mockResolvedValue({ status: "ok", data: null }),
}));

vi.mock("@hypr/plugin-notification", async () => {
  const actual = await vi.importActual<
    typeof import("@hypr/plugin-notification")
  >("@hypr/plugin-notification");

  return {
    ...actual,
    commands: {
      ...actual.commands,
      showNotification: pluginNotification.showNotification,
    },
  };
});

import { checkEventNotifications } from "./index";

import { SCHEMA as SETTINGS_SCHEMA } from "~/store/tinybase/store/settings";

function createMainStore() {
  return createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value);
}

function createSettingsStore() {
  return createMergeableStore()
    .setTablesSchema(SETTINGS_SCHEMA.table)
    .setValuesSchema(SETTINGS_SCHEMA.value);
}

describe("checkEventNotifications", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-30T16:00:00.000Z"));
    pluginNotification.showNotification.mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("shows persistent notifications for upcoming events", () => {
    const store = createMainStore();
    const settingsStore = createSettingsStore();
    const startTime = "2026-03-30T16:02:00.000Z";

    settingsStore.setValue("notification_event", true);
    store.setRow("events", "event-1", {
      title: "Design review",
      started_at: startTime,
    });

    checkEventNotifications(store, settingsStore, new Map());

    expect(pluginNotification.showNotification).toHaveBeenCalledWith(
      expect.objectContaining({
        key: `event-event-1-${new Date(startTime).getTime()}`,
        title: "Design review",
        message: "Starting in 2 minutes",
        timeout: null,
        source: { type: "calendar_event", event_id: "event-1" },
        action_label: "Start listening",
      }),
    );
  });
});
