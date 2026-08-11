import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { ListenButton } from "./listen";

import type { Tab } from "~/store/zustand/tabs";

const { useListenerMock } = vi.hoisted(() => ({
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
    recoverySettingsTab: null,
  }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

describe("floating ListenButton", () => {
  afterEach(() => {
    cleanup();
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
    expect(screen.queryByText("starts in 1s")).toBeNull();
    expect(screen.queryByRole("button", { name: /Join/ })).toBeNull();
  });
});
