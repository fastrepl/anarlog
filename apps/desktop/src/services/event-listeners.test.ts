import { beforeEach, describe, expect, it, vi } from "vitest";

const sessionMocks = vi.hoisted(() => ({
  createSession: vi.fn().mockReturnValue("new-session"),
  getOrCreateSessionForEventId: vi
    .fn()
    .mockImplementation(
      (_store: unknown, eventId: string) => `event-${eventId}`,
    ),
}));

vi.mock("~/store/tinybase/store/sessions", () => ({
  createSession: sessionMocks.createSession,
  getOrCreateSessionForEventId: sessionMocks.getOrCreateSessionForEventId,
}));

import { getNotificationOpenConfig } from "./event-listeners";
import { createSummaryReadyNotificationKey } from "./summary-ready-notification";

describe("getNotificationOpenConfig", () => {
  beforeEach(() => {
    sessionMocks.createSession.mockClear();
    sessionMocks.getOrCreateSessionForEventId.mockClear();
  });

  it("opens summary notifications in the enhanced note without autostart", () => {
    const store = {} as never;

    expect(
      getNotificationOpenConfig(
        {
          key: createSummaryReadyNotificationKey("session-1", "note-1"),
          source: null,
        },
        store,
      ),
    ).toEqual({
      id: "session-1",
      state: {
        view: { type: "enhanced", id: "note-1" },
        autoStart: null,
      },
    });
    expect(sessionMocks.createSession).not.toHaveBeenCalled();
    expect(sessionMocks.getOrCreateSessionForEventId).not.toHaveBeenCalled();
  });

  it("opens calendar event notifications in their linked session and autostarts", () => {
    const store = {} as never;

    expect(
      getNotificationOpenConfig(
        {
          key: "event-1",
          source: { type: "calendar_event", event_id: "event-1" },
        },
        store,
      ),
    ).toEqual({
      id: "event-event-1",
      state: { view: null, autoStart: true },
    });
    expect(sessionMocks.getOrCreateSessionForEventId).toHaveBeenCalledWith(
      store,
      "event-1",
    );
  });

  it("falls back to a new session for generic notification clicks", () => {
    const store = {} as never;

    expect(
      getNotificationOpenConfig(
        {
          key: "generic-notification",
          source: null,
        },
        store,
      ),
    ).toEqual({
      id: "new-session",
      state: { view: null, autoStart: true },
    });
    expect(sessionMocks.createSession).toHaveBeenCalledWith(store);
  });
});
