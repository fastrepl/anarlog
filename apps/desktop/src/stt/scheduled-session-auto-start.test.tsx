import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { ScheduledSessionAutoStart } from "./scheduled-session-auto-start";

import { useAppLock } from "~/lock/store";

const mocks = vi.hoisted(() => ({
  beginScheduledAutoStart: vi.fn(),
  canStart: true,
  finishScheduledAutoStart: vi.fn(),
  inFlight: false,
  connectionReady: true,
  session: {
    id: "session-1",
    user_id: "user-1",
    created_at: "2026-05-15T12:00:00.000Z",
    folder_id: "",
    event_json: "",
    title: "Design Review",
    raw_md: "",
    raw_template_id: "",
    locked: false,
  } as {
    id: string;
    user_id: string;
    created_at: string;
    folder_id: string;
    event_json: string;
    title: string;
    raw_md: string;
    raw_template_id: string;
    locked: boolean;
  } | null,
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

vi.mock("~/session/queries", () => ({
  useSession: () => mocks.session,
}));

vi.mock("~/stt/scheduled-auto-start-state", () => ({
  beginScheduledAutoStart: mocks.beginScheduledAutoStart,
  finishScheduledAutoStart: mocks.finishScheduledAutoStart,
  isScheduledAutoStartInFlight: () => mocks.inFlight,
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListeningState: () => ({
    connectionReady: mocks.connectionReady,
    startListening: mocks.startListening,
  }),
}));

beforeEach(() => {
  mocks.canStart = true;
  mocks.inFlight = false;
  mocks.session = {
    id: "session-1",
    user_id: "user-1",
    created_at: "2026-05-15T12:00:00.000Z",
    folder_id: "",
    event_json: "",
    title: "Design Review",
    raw_md: "",
    raw_template_id: "",
    locked: false,
  };
  mocks.beginScheduledAutoStart.mockReset();
  mocks.finishScheduledAutoStart.mockReset();
  mocks.connectionReady = true;
  mocks.startListening.mockReset().mockResolvedValue(undefined);
  mocks.updateSessionTabState.mockReset();
  useAppLock.setState({ revealedNoteIds: {} });
});

afterEach(() => {
  cleanup();
});

test("starts scheduled recording when its connection state is ready", async () => {
  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
  expect(mocks.beginScheduledAutoStart).toHaveBeenCalledWith("session-1");
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
  await vi.waitFor(() =>
    expect(mocks.finishScheduledAutoStart).toHaveBeenCalledWith("session-1"),
  );
});

test("does not start a second lifecycle while a scheduled start is in flight", () => {
  mocks.inFlight = true;

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.beginScheduledAutoStart).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
});

test("starts when capture readiness becomes available", () => {
  mocks.canStart = false;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.canStart = true;
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});

test("starts when the session record becomes available", () => {
  mocks.session = null;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.session = {
    id: "session-1",
    user_id: "user-1",
    created_at: "2026-05-15T12:00:00.000Z",
    folder_id: "",
    event_json: "",
    title: "Design Review",
    raw_md: "",
    raw_template_id: "",
    locked: false,
  };
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});

test("waits for the recording hook's connection before auto-starting", () => {
  mocks.connectionReady = false;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.connectionReady = true;
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});

test("abandons when the recording connection never becomes ready", async () => {
  vi.useFakeTimers();
  mocks.connectionReady = false;

  try {
    render(<ScheduledSessionAutoStart sessionId="session-1" />);
    await vi.advanceTimersByTimeAsync(30_000);

    expect(mocks.startListening).not.toHaveBeenCalled();
    expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
      view: null,
      autoStart: null,
    });
  } finally {
    vi.useRealTimers();
  }
});

test("abandons a scheduled start that never becomes ready", async () => {
  vi.useFakeTimers();
  mocks.canStart = false;

  try {
    render(<ScheduledSessionAutoStart sessionId="session-1" />);
    await vi.advanceTimersByTimeAsync(30_000);

    expect(mocks.startListening).not.toHaveBeenCalled();
    expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
      view: null,
      autoStart: null,
    });
  } finally {
    vi.useRealTimers();
  }
});

test("abandons when the session loads locked so later meetings can start", () => {
  mocks.session = {
    ...mocks.session!,
    locked: true,
  };

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
});

test("abandons when a pending session becomes locked", () => {
  mocks.session = null;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.session = {
    id: "session-1",
    user_id: "user-1",
    created_at: "2026-05-15T12:00:00.000Z",
    folder_id: "",
    event_json: "",
    title: "Design Review",
    raw_md: "",
    raw_template_id: "",
    locked: true,
  };
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
});

test("starts a locked session after it has been revealed", () => {
  useAppLock.setState({ revealedNoteIds: { "session-1": true } });
  mocks.session = {
    ...mocks.session!,
    locked: true,
  };

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});
