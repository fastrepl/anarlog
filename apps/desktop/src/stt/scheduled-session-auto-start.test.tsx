import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { ScheduledSessionAutoStart } from "./scheduled-session-auto-start";

import { useAppLock } from "~/lock/store";
import {
  queueScheduledAutoJoin,
  takeScheduledAutoJoin,
} from "~/stt/scheduled-auto-start-state";

const mocks = vi.hoisted(() => ({
  beginScheduledAutoStart: vi.fn(),
  canStart: true,
  finishScheduledAutoStart: vi.fn(),
  openUrl: vi.fn(),
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

vi.mock("~/stt/scheduled-auto-start-state", async () => {
  const actual = await vi.importActual<
    typeof import("./scheduled-auto-start-state")
  >("./scheduled-auto-start-state");
  return {
    ...actual,
    beginScheduledAutoStart: mocks.beginScheduledAutoStart,
    finishScheduledAutoStart: mocks.finishScheduledAutoStart,
  };
});

vi.mock("~/stt/useStartListening", () => ({
  useStartListeningState: () => ({
    connectionReady: mocks.connectionReady,
    startListening: mocks.startListening,
  }),
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

beforeEach(() => {
  mocks.canStart = true;
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
  mocks.openUrl.mockReset();
  mocks.connectionReady = true;
  mocks.startListening.mockReset().mockResolvedValue(undefined);
  mocks.updateSessionTabState.mockReset();
  takeScheduledAutoJoin("session-1");
  useAppLock.setState({ revealedNoteIds: {} });
});

afterEach(() => {
  cleanup();
});

test("starts scheduled recording when its connection state is ready", async () => {
  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
  expect(mocks.openUrl).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
  await vi.waitFor(() =>
    expect(mocks.finishScheduledAutoStart).toHaveBeenCalledWith("session-1"),
  );
});

test("opens a queued meeting link when listening starts", async () => {
  queueScheduledAutoJoin("session-1", "https://meet.google.com/abc-defg-hij");

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).toHaveBeenCalledTimes(1);
  expect(mocks.openUrl).toHaveBeenCalledWith(
    "https://meet.google.com/abc-defg-hij",
    null,
  );
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
    expect(mocks.openUrl).not.toHaveBeenCalled();
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
  queueScheduledAutoJoin("session-1", "https://meet.google.com/abc-defg-hij");

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.openUrl).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
  });
  expect(takeScheduledAutoJoin("session-1")).toBeUndefined();
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

test("discards a queued meeting link when the start is abandoned", async () => {
  vi.useFakeTimers();
  mocks.canStart = false;
  queueScheduledAutoJoin("session-1", "https://meet.google.com/abc-defg-hij");

  try {
    render(<ScheduledSessionAutoStart sessionId="session-1" />);
    await vi.advanceTimersByTimeAsync(30_000);

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(takeScheduledAutoJoin("session-1")).toBeUndefined();
  } finally {
    vi.useRealTimers();
  }
});
