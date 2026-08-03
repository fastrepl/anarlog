import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn(),
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
}));

import { useNewNoteAndListen } from "./useNewNote";

import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { resetTabsStore } from "~/store/zustand/tabs/test-utils";

beforeEach(() => {
  vi.clearAllMocks();
  resetTabsStore();
  listenerStore.setState(listenerStore.getInitialState(), true);
});

it("can open a listening note without a listener provider", async () => {
  mocks.createSession.mockResolvedValueOnce("new-session");
  const { result } = renderHook(() => useNewNoteAndListen());

  act(() => result.current());

  await vi.waitFor(() => {
    expect(useTabs.getState().currentTab).toMatchObject({
      type: "sessions",
      id: "new-session",
      state: { autoStart: true },
    });
  });
});

it("reads the current live session when the handler runs", () => {
  const { result } = renderHook(() => useNewNoteAndListen());

  listenerStore.setState((state) => ({
    live: {
      ...state.live,
      status: "active",
      sessionId: "live-session",
    },
  }));
  act(() => result.current());

  expect(mocks.createSession).not.toHaveBeenCalled();
  expect(useTabs.getState().currentTab).toMatchObject({
    type: "sessions",
    id: "live-session",
  });
});
