import { render } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import { ScheduledSessionAutoStart } from "./scheduled-session-auto-start";

const mocks = vi.hoisted(() => ({
  beginScheduledAutoStart: vi.fn(),
  canStart: true,
  finishScheduledAutoStart: vi.fn(),
  startListening: vi.fn(),
  updateSessionTabState: vi.fn(),
}));

const tab = {
  type: "sessions" as const,
  id: "session-1",
  active: true,
  slotId: "slot-1",
  pinned: false,
  state: { view: null, autoStart: true },
};

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => ({
      tabs: [tab],
      updateSessionTabState: mocks.updateSessionTabState,
    }),
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: any) => unknown) =>
    selector({ canStartLiveSession: () => mocks.canStart }),
}));

vi.mock("~/stt/scheduled-auto-start-state", () => ({
  beginScheduledAutoStart: mocks.beginScheduledAutoStart,
  finishScheduledAutoStart: mocks.finishScheduledAutoStart,
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => mocks.startListening,
}));

beforeEach(() => {
  mocks.canStart = true;
  mocks.beginScheduledAutoStart.mockReset();
  mocks.finishScheduledAutoStart.mockReset();
  mocks.startListening.mockReset().mockResolvedValue(undefined);
  mocks.updateSessionTabState.mockReset();
});

test("starts scheduled recording without waiting for transcription", async () => {
  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
  await vi.waitFor(() =>
    expect(mocks.finishScheduledAutoStart).toHaveBeenCalledWith("session-1"),
  );
});

test("starts when capture readiness becomes available", () => {
  mocks.canStart = false;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.canStart = true;
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});

test("abandons a scheduled start that never becomes ready", async () => {
  vi.useFakeTimers();
  mocks.canStart = false;

  render(<ScheduledSessionAutoStart sessionId="session-1" />);
  await vi.advanceTimersByTimeAsync(30_000);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
});
