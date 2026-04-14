import { render } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { EventListeners } from "./event-listeners";

const {
  notificationListenMock,
  updaterListenMock,
  maybeEmitUpdatedMock,
  getCurrentWebviewWindowLabelMock,
  useMainStoreMock,
  useSettingsStoreMock,
  openNewMock,
  createSessionMock,
  getOrCreateSessionForEventIdMock,
} = vi.hoisted(() => ({
  notificationListenMock: vi.fn(),
  updaterListenMock: vi.fn(),
  maybeEmitUpdatedMock: vi.fn(),
  getCurrentWebviewWindowLabelMock: vi.fn(() => "main"),
  useMainStoreMock: vi.fn(() => null),
  useSettingsStoreMock: vi.fn(() => null),
  openNewMock: vi.fn(),
  createSessionMock: vi.fn(() => "session-new"),
  getOrCreateSessionForEventIdMock: vi.fn(() =>
    Promise.resolve("session-event"),
  ),
}));

vi.mock("@hypr/plugin-notification", () => ({
  events: {
    notificationEvent: {
      listen: notificationListenMock,
    },
  },
}));

vi.mock("@hypr/plugin-updater2", () => ({
  commands: {
    maybeEmitUpdated: maybeEmitUpdatedMock,
  },
  events: {
    updatedEvent: {
      listen: updaterListenMock,
    },
  },
}));

vi.mock("@hypr/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: getCurrentWebviewWindowLabelMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main-store",
  UI: {
    useStore: useMainStoreMock,
  },
}));

vi.mock("~/store/tinybase/store/settings", () => ({
  STORE_ID: "settings-store",
  UI: {
    useStore: useSettingsStoreMock,
  },
}));

vi.mock("~/store/tinybase/store/sessions", () => ({
  createSession: createSessionMock,
  getOrCreateSessionForEventId: getOrCreateSessionForEventIdMock,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof openNewMock }) => unknown) =>
    selector({ openNew: openNewMock }),
}));

describe("EventListeners notification events", () => {
  beforeEach(() => {
    notificationListenMock.mockReset();
    updaterListenMock.mockReset();
    maybeEmitUpdatedMock.mockReset();
    getCurrentWebviewWindowLabelMock.mockReset();
    useMainStoreMock.mockReset();
    useSettingsStoreMock.mockReset();
    openNewMock.mockReset();
    createSessionMock.mockReset();
    getOrCreateSessionForEventIdMock.mockReset();

    getCurrentWebviewWindowLabelMock.mockReturnValue("main");
    notificationListenMock.mockResolvedValue(() => {});
    updaterListenMock.mockResolvedValue(() => {});
    createSessionMock.mockReturnValue("session-new");
    getOrCreateSessionForEventIdMock.mockReturnValue(
      Promise.resolve("session-event"),
    );
    useMainStoreMock.mockReturnValue(null);
    useSettingsStoreMock.mockReturnValue(null);
  });

  test("stores mic-detected footer actions as ignored platforms", async () => {
    const settingsStore = {
      getValue: vi.fn(() => JSON.stringify(["com.existing.app"])),
      setValue: vi.fn(),
    };
    useSettingsStoreMock.mockReturnValue(settingsStore as never);

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_footer_action",
        key: "mic-1",
        source: {
          type: "mic_detected",
          app_names: ["Zoom"],
          app_ids: ["us.zoom.xos", "com.existing.app"],
          event_ids: [],
        },
      },
    });

    expect(settingsStore.setValue).toHaveBeenCalledWith(
      "ignored_platforms",
      JSON.stringify(["com.existing.app", "us.zoom.xos"]),
    );
    expect(openNewMock).not.toHaveBeenCalled();
  });
});
