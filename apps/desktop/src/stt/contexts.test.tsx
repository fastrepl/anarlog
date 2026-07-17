import { resolveResource } from "@tauri-apps/api/path";
import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { parseAutoStopEndedNotificationKey } from "./auto-stop-notification";
import {
  AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS,
  AUTO_STOP_CONFIRM_DELAY_MS,
  AUTO_STOP_EVENT_END_GRACE_MS,
  ListenerProvider,
} from "./contexts";

import { createListenerStore } from "~/store/zustand/listener";

const {
  listMicUsingApplicationsMock,
  listenMock,
  showNotificationMock,
  useStoreMock,
  useConfigValueMock,
  getNearbyCalendarEventsMock,
  loadSessionEventMock,
} = vi.hoisted(() => ({
  listMicUsingApplicationsMock: vi.fn(),
  listenMock: vi.fn(),
  showNotificationMock: vi.fn(),
  useStoreMock: vi.fn(() => null),
  useConfigValueMock: vi.fn((_key: string) => true),
  getNearbyCalendarEventsMock: vi.fn(),
  loadSessionEventMock: vi.fn(),
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: listMicUsingApplicationsMock,
  },
  events: {
    detectEvent: {
      listen: listenMock,
    },
  },
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: {
    showNotification: showNotificationMock,
  },
}));

vi.mock("~/calendar/queries", () => ({
  getNearbyCalendarEvents: getNearbyCalendarEventsMock,
}));

vi.mock("~/session/queries", () => ({
  loadSessionEvent: loadSessionEventMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

function setStoreActive(
  store: ReturnType<typeof createListenerStore>,
  sessionId = "session-1",
) {
  store.setState((state) => ({
    live: { ...state.live, sessionId, status: "active" },
  }));
}

function mockSessionEventStore(event: {
  started_at: string;
  ended_at: string;
  is_all_day?: boolean;
}) {
  return {
    getRow: vi.fn((table: string, rowId: string) =>
      table === "sessions" && rowId === "session-1"
        ? {
            event_json: JSON.stringify({
              tracking_id: "tracking-1",
              calendar_id: "calendar-1",
              title: "Design sync",
              has_recurrence_rules: false,
              ...event,
            }),
          }
        : undefined,
    ),
    forEachRow: vi.fn(),
  };
}

function mockNearbyEventStore(event: {
  id?: string;
  title?: string;
  started_at: string;
  meeting_link?: string;
  location?: string;
  description?: string;
  participants_json?: string;
  is_all_day?: boolean;
}) {
  return mockNearbyEventStoreMany([event]);
}

function mockNearbyEventStoreMany(
  events: Array<{
    id?: string;
    title?: string;
    started_at: string;
    meeting_link?: string;
    location?: string;
    description?: string;
    participants_json?: string;
    is_all_day?: boolean;
  }>,
) {
  return {
    getRow: vi.fn((table: string, rowId: string) => {
      const event = events.find(
        (event, index) => (event.id ?? `event-${index + 1}`) === rowId,
      );
      if (table !== "events" || !event) {
        return undefined;
      }

      return {
        title: event.title ?? "Design sync",
        started_at: event.started_at,
        meeting_link: event.meeting_link,
        location: event.location,
        description: event.description,
        participants_json: event.participants_json,
        is_all_day: event.is_all_day ?? false,
      };
    }),
    forEachRow: vi.fn((table: string, callback: (rowId: string) => void) => {
      if (table === "events") {
        events.forEach((event, index) =>
          callback(event.id ?? `event-${index + 1}`),
        );
      }
    }),
  };
}

async function readConfiguredSessionEvent(sessionId: string) {
  const store = useStoreMock() as any;
  const row = store?.getRow?.("sessions", sessionId);
  if (!row?.event_json) return null;
  return JSON.parse(row.event_json);
}

async function readConfiguredNearbyEvents(nowMs: number, windowMs: number) {
  const store = useStoreMock() as any;
  if (!store) return [];

  const rows: Array<{
    id: string;
    title: string;
    meetingLink?: string;
    location?: string;
    description?: string;
    participantNames: string[];
    startedAt: number;
  }> = [];
  store.forEachRow?.("events", (eventId: string) => {
    const event = store.getRow?.("events", eventId);
    if (!event?.started_at || event.is_all_day) return;
    const startedAt = new Date(event.started_at).getTime();
    if (Number.isNaN(startedAt) || Math.abs(startedAt - nowMs) > windowMs) {
      return;
    }

    let participants: Array<{ name?: string; is_current_user?: boolean }> = [];
    try {
      const parsed = JSON.parse(event.participants_json || "[]");
      if (Array.isArray(parsed)) participants = parsed;
    } catch {}

    rows.push({
      id: eventId,
      title: event.title || "Untitled Event",
      meetingLink: event.meeting_link || undefined,
      location: event.location || undefined,
      description: event.description || undefined,
      participantNames: [
        ...new Set(
          participants
            .filter((participant) => !participant.is_current_user)
            .map((participant) => participant.name?.trim() || "")
            .filter(Boolean),
        ),
      ],
      startedAt,
    });
  });

  rows.sort(
    (a, b) =>
      Math.abs(a.startedAt - nowMs) - Math.abs(b.startedAt - nowMs) ||
      a.startedAt - b.startedAt,
  );
  return rows.map(({ startedAt: _startedAt, ...event }) => event);
}

describe("ListenerProvider detect events", () => {
  beforeEach(() => {
    listenMock.mockReset();
    showNotificationMock.mockReset();
    useStoreMock.mockReset();
    useConfigValueMock.mockReset();
    getNearbyCalendarEventsMock.mockReset();
    loadSessionEventMock.mockReset();
    useStoreMock.mockReturnValue(null);
    useConfigValueMock.mockReturnValue(true);
    getNearbyCalendarEventsMock.mockImplementation(readConfiguredNearbyEvents);
    loadSessionEventMock.mockImplementation(readConfiguredSessionEvent);
    listenMock.mockResolvedValue(() => {});
    listMicUsingApplicationsMock.mockResolvedValue({ status: "ok", data: [] });
    Object.defineProperty(window.navigator, "onLine", {
      configurable: true,
      value: true,
    });
    vi.useRealTimers();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  test("does not stop listening on MicStopped when no trigger apps are set (manual session — regression: #5120)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micStopped",
        apps: [
          { id: "/opt/homebrew/bin/ffmpeg", name: "ffmpeg" },
          { id: "us.zoom.xos", name: "Zoom" },
        ],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("stops listening after confirming a trigger app remains stopped", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("does not stop when a trigger app resumes during the auto-stop grace period", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "us.zoom.xos", name: "Zoom" }],
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("holds a network-interrupted meeting until its event end grace expires", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");
    const endedAtMs = new Date("2026-05-19T10:30:00.000Z").getTime();
    const deadlineMs = endedAtMs + AUTO_STOP_EVENT_END_GRACE_MS;

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });
    window.dispatchEvent(new Event("online"));

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(stopSpy).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(deadlineMs - Date.now() - 1);
    expect(stopSpy).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("cancels a network interruption hold when the meeting resumes during event grace", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");
    const endedAtMs = new Date("2026-05-19T10:30:00.000Z").getTime();
    const deadlineMs = endedAtMs + AUTO_STOP_EVENT_END_GRACE_MS;

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    await vi.advanceTimersByTimeAsync(
      endedAtMs + AUTO_STOP_EVENT_END_GRACE_MS / 2 - Date.now(),
    );

    window.dispatchEvent(new Event("online"));
    handler({
      payload: {
        type: "micDetected",
        key: "mic-resumed",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    await vi.advanceTimersByTimeAsync(deadlineMs - Date.now());
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("upgrades a pending auto-stop when the network drops during confirmation", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS - 1);
    window.dispatchEvent(new Event("offline"));
    await vi.advanceTimersByTimeAsync(1);

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("keeps standard auto-stop behavior for offline ad-hoc meetings", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    vi.useFakeTimers();
    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("does not let an interrupted meeting timer stop a replacement session", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");
    const deadlineMs =
      new Date("2026-05-19T10:30:00.000Z").getTime() +
      AUTO_STOP_EVENT_END_GRACE_MS;

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    setStoreActive(store, "session-2");
    await vi.advanceTimersByTimeAsync(deadlineMs - Date.now());

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("does not hold for a future linked event outside the early-start buffer", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T11:00:00.000Z",
        ended_at: "2026-05-19T11:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-05-19T10:00:00.000Z"));

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("does not stop on MicStopped when auto-stop is disabled", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    useConfigValueMock.mockReturnValue(false);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("does not stop on MicStopped when only a non-trigger app stops (auto-session — regression: #4846)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "/opt/homebrew/bin/ffmpeg", name: "ffmpeg" }],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("does not stop after non-trigger MicStopped when a trigger app is still active", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "us.zoom.xos", name: "Zoom" }],
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "/opt/homebrew/bin/ffmpeg", name: "ffmpeg" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test.each([
    [[{ id: "com.kakao.KakaoTalkMac", name: "KakaoTalk" }]],
    [[{ id: "pid:42", name: "KakaoTalk Helper" }]],
  ])(
    "does not auto-stop KakaoTalk sessions from screen-share mic transitions",
    async (stoppedApps) => {
      const store = createListenerStore();
      const stopSpy = vi.fn();

      store.setState({ stop: stopSpy });
      store.getState().setTriggerAppIds(["com.kakao.KakaoTalkMac"]);
      setStoreActive(store);

      render(
        <ListenerProvider store={store}>
          <div>child</div>
        </ListenerProvider>,
      );

      await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

      const handler = listenMock.mock.calls[0]?.[0];
      expect(handler).toBeTypeOf("function");

      vi.useFakeTimers();
      listMicUsingApplicationsMock.mockClear();

      handler({
        payload: {
          type: "micStopped",
          apps: stoppedApps,
        },
      });

      await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

      expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
      expect(stopSpy).not.toHaveBeenCalled();
    },
  );

  test("does not auto-stop co-trigger sessions while KakaoTalk remains active after a helper stop", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store
      .getState()
      .setTriggerAppIds(["com.kakao.KakaoTalkMac", "us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.kakao.KakaoTalkMac", name: "KakaoTalk" }],
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "KakaoTalk Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("auto-stops co-trigger sessions after a helper stop when no trigger app remains active", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store
      .getState()
      .setTriggerAppIds(["com.kakao.KakaoTalkMac", "us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "KakaoTalk Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("auto-stops when MicStopped omits the trigger app and no trigger app remains active (regression: #5436)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.microsoft.teams2"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "Microsoft Teams Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("auto-stops Teams running in a browser when the browser no longer uses the mic (regression: #5436)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["company.thebrowser.Browser"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "company.thebrowser.Browser", name: "Arc" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("keeps direct trigger auto-stop confidence when a later helper stop arrives", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "error",
      error: "failed to read mic snapshot",
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "Zoom Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("passes ignorable app ids and footer metadata through mic-detected notifications", async () => {
    const store = createListenerStore();

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [
          { id: "pid:42", name: "Zoom" },
          { id: "us.zoom.xos", name: "Zoom" },
        ],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: {
            type: "mic_detected",
            app_names: ["Zoom", "Zoom"],
            app_ids: ["us.zoom.xos"],
            event_ids: [],
          },
          footer: {
            text: "Ignore Zoom?",
            actionLabel: "Yes",
            icon: {
              type: "path",
              path: "/resources/notification-icons/zoom.svg",
            },
          },
          icon: {
            type: "path",
            path: "/resources/notification-icons/zoom.svg",
          },
        }),
      ),
    );
  });

  test("does not show mic-detected prompts when detection notifications are disabled", async () => {
    const store = createListenerStore();
    useConfigValueMock.mockImplementation((key: string) =>
      key === "notification_detect" ? false : true,
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    await Promise.resolve();

    expect(showNotificationMock).not.toHaveBeenCalled();
    expect(getNearbyCalendarEventsMock).not.toHaveBeenCalled();
  });

  test("shows iPhone call icon and label for AV Capture mic notifications", async () => {
    const store = createListenerStore();

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "pid:42", name: "AV Capture" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: {
            type: "mic_detected",
            app_names: ["iPhone Call"],
            app_ids: [],
            event_ids: [],
          },
          footer: null,
          icon: {
            type: "path",
            path: "/resources/notification-icons/phone.png",
          },
        }),
      ),
    );
  });

  test("shows iPhone call icon and label for avconferenced mic notifications", async () => {
    const store = createListenerStore();

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "/usr/libexec/avconferenced", name: "avconferenced" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: {
            type: "mic_detected",
            app_names: ["iPhone Call"],
            app_ids: ["/usr/libexec/avconferenced"],
            event_ids: [],
          },
          footer: {
            text: "Ignore iPhone Call?",
            actionLabel: "Yes",
            icon: {
              type: "path",
              path: "/resources/notification-icons/phone.png",
            },
          },
          icon: {
            type: "path",
            path: "/resources/notification-icons/phone.png",
          },
        }),
      ),
    );
  });

  test("shows meeting platform for browser mic notifications with nearby meeting link", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Design sync",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "at.studio.AsideBrowser", name: "Aside" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: {
            type: "mic_detected",
            app_names: ["Google Meet"],
            app_ids: ["at.studio.AsideBrowser"],
            event_ids: ["event-1"],
          },
          title: "Are you in Design sync right now?",
          action_label: "Yes",
          options: null,
          footer: {
            text: "Ignore Google Meet?",
            actionLabel: "Yes",
            icon: {
              type: "path",
              path: "/resources/notification-icons/google-meet.svg",
            },
          },
          icon: {
            type: "path",
            path: "/resources/notification-icons/google-meet.svg",
          },
        }),
      ),
    );
  });

  test("uses event participants for nearby mic notification copy", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Design sync",
        started_at: "2026-06-24T02:09:00.000Z",
        participants_json: JSON.stringify([
          { name: "John", email: "john@example.com", is_current_user: true },
          { name: "Artem", email: "artem@example.com" },
        ]),
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Are you talking to Artem right now?",
          source: expect.objectContaining({
            event_ids: ["event-1"],
          }),
          action_label: "Yes",
          options: null,
        }),
      ),
    );
  });

  test("uses event title for nearby mic notification copy with several participants", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Design sync",
        started_at: "2026-06-24T02:09:00.000Z",
        participants_json: JSON.stringify([
          { name: "John", email: "john@example.com", is_current_user: true },
          { name: "Artem", email: "artem@example.com" },
          { name: "Ananya", email: "ananya@example.com" },
          { name: "Maria", email: "maria@example.com" },
        ]),
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Are you in Design sync right now?",
          source: expect.objectContaining({
            event_ids: ["event-1"],
          }),
          action_label: "Yes",
          options: null,
        }),
      ),
    );
  });

  test("detects Microsoft Teams live join links for browser mic notifications", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Partner sync",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://teams.live.com/meet/1234567890",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: ["Microsoft Teams"],
            app_ids: ["com.google.Chrome"],
          }),
          footer: expect.objectContaining({
            text: "Ignore Microsoft Teams?",
            icon: {
              type: "path",
              path: "/resources/notification-icons/microsoft-teams.svg",
            },
          }),
          icon: {
            type: "path",
            path: "/resources/notification-icons/microsoft-teams.svg",
          },
        }),
      ),
    );
  });

  test("prefers explicit meeting links over earlier nearby event text", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStoreMany([
        {
          title: "Discord planning",
          started_at: "2026-06-24T02:08:00.000Z",
        },
        {
          title: "Design sync",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: "https://meet.google.com/abc-defg-hij",
        },
      ]),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: ["Google Meet"],
            event_ids: ["event-2"],
          }),
          title: "Are you in Design sync right now?",
          action_label: "Yes",
          options: null,
          footer: expect.objectContaining({
            text: "Ignore Google Meet?",
          }),
          icon: {
            type: "path",
            path: "/resources/notification-icons/google-meet.svg",
          },
        }),
      ),
    );
  });

  test("does not infer browser platform from a different nearby event", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStoreMany([
        {
          title: "Sales sync",
          started_at: "2026-06-24T02:09:00.000Z",
        },
        {
          title: "Design sync",
          started_at: "2026-06-24T02:10:00.000Z",
          meeting_link: "https://meet.google.com/abc-defg-hij",
        },
      ]),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: ["Google Chrome"],
            event_ids: ["event-1"],
          }),
          title: "Are you in Sales sync right now?",
          footer: expect.objectContaining({
            text: "Ignore Google Chrome?",
          }),
          icon: { type: "bundle_id", bundle_id: "com.google.Chrome" },
        }),
      ),
    );
  });

  test("does not infer chat platforms from incidental calendar text", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Quarterly signal review",
        started_at: "2026-06-24T02:09:00.000Z",
        description: "Discuss discordance in metrics with the messenger team",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: ["Google Chrome"],
          }),
          footer: expect.objectContaining({
            text: "Ignore Google Chrome?",
            icon: { type: "bundle_id", bundle_id: "com.google.Chrome" },
          }),
          icon: { type: "bundle_id", bundle_id: "com.google.Chrome" },
        }),
      ),
    );
  });

  test("does not show a stale mic prompt when listening starts while icons resolve", async () => {
    const store = createListenerStore();
    let iconResolverReady = false;
    let resolveIcon = () => {};

    vi.mocked(resolveResource).mockImplementationOnce(
      (path: string) =>
        new Promise((resolve) => {
          resolveIcon = () => {
            resolve(`/resources/${path}`);
          };
          iconResolverReady = true;
        }),
    );
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Customer call",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://webex.com/meet/customer-call",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() => expect(iconResolverReady).toBe(true));

    setStoreActive(store);
    resolveIcon();

    await vi.waitFor(() =>
      expect(store.getState().live.triggerAppIds).toEqual([
        "com.google.Chrome",
      ]),
    );
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("does not show duplicate mic prompts while icons resolve", async () => {
    const store = createListenerStore();
    let iconResolverReady = false;
    let resolveIcon = () => {};

    vi.mocked(resolveResource).mockImplementationOnce(
      (path: string) =>
        new Promise((resolve) => {
          resolveIcon = () => {
            resolve(`/resources/${path}`);
          };
          iconResolverReady = true;
        }),
    );
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Product review",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://meet.jit.si/product-review",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() => expect(iconResolverReady).toBe(true));

    handler({
      payload: {
        type: "micDetected",
        key: "mic-2",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    resolveIcon();

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledTimes(1),
    );
    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        key: "mic-1",
        source: expect.objectContaining({
          app_names: ["Jitsi"],
        }),
      }),
    );
  });

  test("does not let calendar video links override detected native meeting apps", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Design sync",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [
          { id: "com.tinyspeck.slackmacgap", name: "Slack" },
          { id: "com.google.Chrome", name: "Google Chrome" },
        ],
        duration_secs: 15,
      },
    });

    const slackIcon = {
      type: "path",
      path: "/resources/notification-icons/slack.svg",
    };

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: ["Slack", "Google Chrome"],
            app_ids: ["com.tinyspeck.slackmacgap", "com.google.Chrome"],
          }),
          footer: expect.objectContaining({
            text: "Ignore Slack and Google Chrome?",
            icon: slackIcon,
          }),
          icon: slackIcon,
        }),
      ),
    );
  });

  test.each([
    "https://app.cal.com/video/founder-call",
    "https://cal.com/video/founder-call",
  ])(
    "shows Cal Video for browser mic notifications with %s",
    async (meetingLink) => {
      const store = createListenerStore();

      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
      (useStoreMock as any).mockReturnValue(
        mockNearbyEventStore({
          title: "Founder call",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: meetingLink,
        }),
      );

      render(
        <ListenerProvider store={store}>
          <div>child</div>
        </ListenerProvider>,
      );

      await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

      const handler = listenMock.mock.calls[0]?.[0];
      expect(handler).toBeTypeOf("function");

      handler({
        payload: {
          type: "micDetected",
          key: "mic-1",
          apps: [{ id: "at.studio.AsideBrowser", name: "Aside" }],
          duration_secs: 15,
        },
      });

      await vi.waitFor(() =>
        expect(showNotificationMock).toHaveBeenCalledWith(
          expect.objectContaining({
            source: expect.objectContaining({
              app_names: ["Cal Video"],
              app_ids: ["at.studio.AsideBrowser"],
              event_ids: ["event-1"],
            }),
            title: "Are you in Founder call right now?",
            action_label: "Yes",
            options: null,
            footer: expect.objectContaining({
              text: "Ignore Cal Video?",
              icon: {
                type: "path",
                path: "/resources/notification-icons/cal-video.png",
              },
            }),
            icon: {
              type: "path",
              path: "/resources/notification-icons/cal-video.png",
            },
          }),
        ),
      );
    },
  );

  test("shows Cal Video for protocol-less Cal.com video text", async () => {
    const store = createListenerStore();

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Founder call",
        started_at: "2026-06-24T02:09:00.000Z",
        location: "cal.com/video/founder-call",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "at.studio.AsideBrowser", name: "Aside" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: ["Cal Video"],
            app_ids: ["at.studio.AsideBrowser"],
            event_ids: ["event-1"],
          }),
          footer: expect.objectContaining({
            text: "Ignore Cal Video?",
          }),
        }),
      ),
    );
  });

  test.each([
    {
      app: { id: "com.apple.FaceTime", name: "FaceTime" },
      icon: { type: "bundle_id", bundle_id: "com.apple.FaceTime" },
    },
    {
      app: { id: "net.whatsapp.WhatsApp", name: "WhatsApp" },
      icon: {
        type: "path",
        path: "/resources/notification-icons/whatsapp.png",
      },
    },
    {
      app: { id: "com.kakao.KakaoTalkMac", name: "KakaoTalk" },
      icon: {
        type: "path",
        path: "/resources/notification-icons/kakaotalk.png",
      },
    },
  ])(
    "uses app-specific icons for $app.name mic notifications",
    async ({ app, icon }) => {
      const store = createListenerStore();

      render(
        <ListenerProvider store={store}>
          <div>child</div>
        </ListenerProvider>,
      );

      await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

      const handler = listenMock.mock.calls[0]?.[0];
      expect(handler).toBeTypeOf("function");

      handler({
        payload: {
          type: "micDetected",
          key: "mic-1",
          apps: [app],
          duration_secs: 15,
        },
      });

      await vi.waitFor(() =>
        expect(showNotificationMock).toHaveBeenCalledWith(
          expect.objectContaining({
            source: expect.objectContaining({
              app_names: [app.name],
              app_ids: [app.id],
            }),
            footer: expect.objectContaining({
              text: `Ignore ${app.name}?`,
              icon,
            }),
            icon,
          }),
        ),
      );
    },
  );

  test("records trigger app ids from micDetected while already listening", async () => {
    const store = createListenerStore();

    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [
          { id: "pid:42", name: "Chrome Helper" },
          { id: "com.google.Chrome", name: "Google Chrome" },
        ],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).not.toHaveBeenCalled();
    expect(store.getState().live.triggerAppIds).toEqual(["com.google.Chrome"]);
  });

  test("records trigger app ids from micDetected while listening is starting", async () => {
    const store = createListenerStore();

    store.setState((state) => ({
      live: {
        ...state.live,
        loading: true,
        sessionId: "session-1",
        status: "inactive",
      },
    }));

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [
          { id: "pid:42", name: "Chrome Helper" },
          { id: "com.google.Chrome", name: "Google Chrome" },
        ],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).not.toHaveBeenCalled();
    expect(store.getState().live.triggerAppIds).toEqual(["com.google.Chrome"]);
  });

  test("auto-stops after a trigger app learned during active listening stops", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("uses the standard auto-stop grace period for browser meeting triggers without calendar context", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test.each([
    { id: "com.google.Chrome", name: "Google Chrome" },
    { id: "at.studio.AsideBrowser", name: "Aside" },
    { id: "net.imput.helium", name: "Helium" },
  ])(
    "asks before stopping when $name stops well before the scheduled end",
    async (browser) => {
      const store = createListenerStore();
      const stopSpy = vi.fn();
      const now = new Date("2026-05-19T10:05:00.000Z");

      store.setState({ stop: stopSpy });
      store.getState().setTriggerAppIds([browser.id]);
      setStoreActive(store);
      (useStoreMock as any).mockReturnValue(
        mockSessionEventStore({
          started_at: "2026-05-19T10:00:00.000Z",
          ended_at: "2026-05-19T10:30:00.000Z",
        }),
      );

      render(
        <ListenerProvider store={store}>
          <div>child</div>
        </ListenerProvider>,
      );

      await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

      const handler = listenMock.mock.calls[0]?.[0];
      expect(handler).toBeTypeOf("function");

      vi.useFakeTimers();
      vi.setSystemTime(now);
      listMicUsingApplicationsMock.mockClear();

      handler({
        payload: {
          type: "micStopped",
          apps: [browser],
        },
      });

      await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

      expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
      expect(stopSpy).not.toHaveBeenCalled();
      const notification = showNotificationMock.mock.calls[0]?.[0];
      expect(parseAutoStopEndedNotificationKey(notification.key)).toBe(
        "session-1",
      );
      expect(notification).toEqual({
        key: expect.stringContaining("auto-stop-ended:session-1"),
        title: "Did your meeting end?",
        message: "Anarlog will stop listening in 30 seconds.",
        timeout: { secs: 30, nanos: 0 },
        source: null,
        start_time: null,
        participants: null,
        event_details: null,
        action_label: "Stop",
        action_variant: "destructive",
        options: null,
        footer: null,
        icon: { type: "bundle_id", bundle_id: browser.id },
      });
    },
  );

  test("auto-stops browser meetings inside the scheduled end window", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const endedAtMs = new Date("2026-05-19T10:30:00.000Z").getTime();
    const now = new Date(
      endedAtMs - AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS + 1,
    );

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    vi.setSystemTime(now);
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("cancels pending auto-stop when a browser trigger restarts", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS - 1);

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.advanceTimersByTimeAsync(1);

    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("stops listening when sleep starts", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "sleepStateChanged",
        value: true,
      },
    });

    expect(stopSpy).toHaveBeenCalledTimes(1);
  });
});
