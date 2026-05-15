import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { checkEventNotifications } from ".";

import type * as main from "~/store/tinybase/store/main";
import type * as settings from "~/store/tinybase/store/settings";

const { showNotificationMock } = vi.hoisted(() => ({
  showNotificationMock: vi.fn(),
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: {
    showNotification: showNotificationMock,
  },
}));

describe("checkEventNotifications", () => {
  beforeEach(() => {
    showNotificationMock.mockReset();
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-05-15T12:00:00.000Z").getTime(),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  test("scheduled meeting notifications do not include expanded payloads", () => {
    const store = {
      getValue: vi.fn(() => undefined),
      forEachRow: vi.fn((table: string, callback: (rowId: string) => void) => {
        if (table === "events") {
          callback("event-1");
        }
      }),
      getRow: vi.fn((table: string, rowId: string) => {
        if (table === "events" && rowId === "event-1") {
          return {
            started_at: "2026-05-15T12:02:00.000Z",
            tracking_id_event: "tracking-1",
            title: "Design Review",
            meeting_link: "https://meet.example.com/design",
          };
        }
        return undefined;
      }),
    } as unknown as main.Store;
    const settingsStore = {
      getValue: vi.fn((key: string) =>
        key === "notification_event" ? true : undefined,
      ),
    } as unknown as settings.Store;

    checkEventNotifications(store, settingsStore, new Map());

    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        source: { type: "calendar_event", event_id: "event-1" },
        action_label: "Open notes",
        participants: null,
        event_details: null,
        options: null,
        footer: null,
      }),
    );
  });
});
