import { render } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { ListenerProvider } from "./contexts";

import { createListenerStore } from "~/store/zustand/listener";

const { listenMock, showNotificationMock, useStoreMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  showNotificationMock: vi.fn(),
  useStoreMock: vi.fn(() => null),
}));

vi.mock("@hypr/plugin-detect", () => ({
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

describe("ListenerProvider detect events", () => {
  beforeEach(() => {
    listenMock.mockReset();
    showNotificationMock.mockReset();
    useStoreMock.mockReset();
    useStoreMock.mockReturnValue(null);
    listenMock.mockResolvedValue(() => {});
  });

  test("stops listening when MicStopped arrives", async () => {
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
        apps: [],
      },
    });

    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("passes detected app ids through notification source metadata", async () => {
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
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        source: {
          type: "mic_detected",
          app_names: ["Zoom"],
          app_ids: ["us.zoom.xos"],
          event_ids: [],
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
