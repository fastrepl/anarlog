import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { AUTO_STOP_CONFIRM_DELAY_MS, ListenerProvider } from "./contexts";

import { createListenerStore } from "~/store/zustand/listener";

const {
  listMicUsingApplicationsMock,
  listenMock,
  showNotificationMock,
  useStoreMock,
  useSettingsStoreMock,
} = vi.hoisted(() => ({
  listMicUsingApplicationsMock: vi.fn(),
  listenMock: vi.fn(),
  showNotificationMock: vi.fn(),
  useStoreMock: vi.fn(() => null),
  useSettingsStoreMock: vi.fn(() => null),
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

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "test-store",
  UI: {
    useStore: useStoreMock,
  },
}));

vi.mock("~/store/tinybase/store/settings", () => ({
  STORE_ID: "settings-store",
  UI: {
    useStore: useSettingsStoreMock,
  },
}));

function setStoreActive(store: ReturnType<typeof createListenerStore>) {
  store.setState((state) => ({
    live: { ...state.live, status: "active" },
  }));
}

describe("ListenerProvider detect events", () => {
  beforeEach(() => {
    listenMock.mockReset();
    showNotificationMock.mockReset();
    useStoreMock.mockReset();
    useSettingsStoreMock.mockReset();
    useStoreMock.mockReturnValue(null);
    useSettingsStoreMock.mockReturnValue(null);
    listenMock.mockResolvedValue(() => {});
    listMicUsingApplicationsMock.mockResolvedValue({ status: "ok", data: [] });
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

  test("does not stop on MicStopped when auto-stop is disabled", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    useSettingsStoreMock.mockReturnValue({
      getValue: vi.fn((key: string) =>
        key === "auto_stop_meetings" ? false : undefined,
      ),
    } as any);

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

    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        source: {
          type: "mic_detected",
          app_names: ["Zoom", "Zoom"],
          app_ids: ["us.zoom.xos"],
          event_ids: [],
        },
        footer: {
          text: "Ignore this app?",
          actionLabel: "Yes",
        },
        icon: null,
      }),
    );
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
