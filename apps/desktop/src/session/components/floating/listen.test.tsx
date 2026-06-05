import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { ListenButton } from "./listen";

import type { Tab } from "~/store/zustand/tabs";

const {
  countdownCallbacks,
  updateSessionTabStateMock,
  useConfigValueMock,
  useListenerMock,
} = vi.hoisted(() => ({
  countdownCallbacks: [] as Array<() => void>,
  updateSessionTabStateMock: vi.fn(),
  useConfigValueMock: vi.fn(() => true),
  useListenerMock: vi.fn((selector) =>
    selector({
      live: { loading: false, sessionId: null },
      canStartLiveSession: () => true,
    }),
  ),
}));

vi.mock("../listen-action", () => ({
  ListenActionButton: ({ sessionId }: { sessionId: string }) => (
    <button type="button">Listen {sessionId}</button>
  ),
}));

vi.mock("~/session/components/shared", () => ({
  useListenButtonState: () => ({
    shouldRender: true,
    isDisabled: false,
    warningMessage: "",
  }),
}));

vi.mock("~/session/hooks/useEventCountdown", () => ({
  useEventCountdown: (
    _sessionId: string,
    options?: { onExpire?: () => void },
  ) => {
    if (options?.onExpire) {
      countdownCallbacks.push(options.onExpire);
    }
    return { label: "meeting starts in 1s" };
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: any) =>
    selector({ updateSessionTabState: updateSessionTabStateMock }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

describe("floating ListenButton", () => {
  afterEach(() => {
    cleanup();
  });

  test("requests auto-start when the event countdown expires", () => {
    const tab = {
      type: "sessions",
      id: "session-1",
      state: { view: null, autoStart: null },
    } as Extract<Tab, { type: "sessions" }>;

    render(<ListenButton tab={tab} />);

    countdownCallbacks[countdownCallbacks.length - 1]?.();

    expect(updateSessionTabStateMock).toHaveBeenCalledWith(tab, {
      view: null,
      autoStart: true,
    });
  });

  test("keeps remote meeting join controls out of the floating slot", () => {
    const tab = {
      type: "sessions",
      id: "session-1",
      state: { view: null, autoStart: null },
    } as Extract<Tab, { type: "sessions" }>;

    render(<ListenButton tab={tab} />);

    expect(
      screen.getByRole("button", { name: "Listen session-1" }),
    ).not.toBeNull();
    expect(screen.queryByRole("button", { name: /Join/ })).toBeNull();
  });
});
