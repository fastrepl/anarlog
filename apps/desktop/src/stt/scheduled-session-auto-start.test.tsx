import { act, cleanup, render } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { ScheduledSessionAutoStart } from "./scheduled-session-auto-start";

import { useAppLock } from "~/lock/store";

const mocks = vi.hoisted(() => ({
  beginScheduledAutoStart: vi.fn(),
  canStart: true,
  liveStatus: "inactive",
  readDueScheduledSessionMeeting: vi.fn(),
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
  state: { view: null, autoStart: true, scheduledAutoStart: true },
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
    selector({
      canStartLiveSession: () => mocks.canStart,
      live: { status: mocks.liveStatus },
    }),
}));

vi.mock("~/session/queries", () => ({
  useSession: () => mocks.session,
}));

vi.mock("~/stt/scheduled-auto-start-state", () => ({
  beginScheduledAutoStart: mocks.beginScheduledAutoStart,
  finishScheduledAutoStart: mocks.finishScheduledAutoStart,
  isScheduledAutoStartInFlight: () => mocks.inFlight,
}));

vi.mock("~/stt/scheduled-auto-start", () => ({
  readDueScheduledSessionMeeting: mocks.readDueScheduledSessionMeeting,
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListeningState: () => ({
    connectionReady: mocks.connectionReady,
    startListening: mocks.startListening,
  }),
}));

beforeEach(() => {
  mocks.canStart = true;
  mocks.liveStatus = "inactive";
  mocks.readDueScheduledSessionMeeting.mockReset().mockResolvedValue({
    id: "event-1",
  });
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

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
  expect(mocks.beginScheduledAutoStart).toHaveBeenCalledWith("session-1");
  await vi.waitFor(() =>
    expect(mocks.finishScheduledAutoStart).toHaveBeenCalledWith("session-1"),
  );
});

test("starts manual recording without calendar eligibility", async () => {
  mocks.readDueScheduledSessionMeeting.mockResolvedValue(null);

  render(
    <ScheduledSessionAutoStart
      sessionId="session-1"
      requiresCalendarEligibility={false}
    />,
  );

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
  expect(mocks.readDueScheduledSessionMeeting).not.toHaveBeenCalled();
  expect(mocks.beginScheduledAutoStart).not.toHaveBeenCalled();
});

test("manual recording supersedes a pending scheduled attendance read", async () => {
  let resolveAttendance!: (value: null) => void;
  mocks.readDueScheduledSessionMeeting.mockReturnValue(
    new Promise((resolve) => {
      resolveAttendance = resolve;
    }),
  );
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  view.rerender(
    <ScheduledSessionAutoStart
      sessionId="session-1"
      requiresCalendarEligibility={false}
    />,
  );

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
  mocks.updateSessionTabState.mockClear();
  await act(async () => resolveAttendance(null));

  expect(mocks.updateSessionTabState).not.toHaveBeenCalled();
  expect(mocks.startListening).toHaveBeenCalledTimes(1);
  expect(mocks.beginScheduledAutoStart).not.toHaveBeenCalled();
});

test("does not start a second lifecycle while a scheduled start is in flight", async () => {
  mocks.inFlight = true;

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  await vi.waitFor(() =>
    expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
      view: null,
      autoStart: null,
      scheduledAutoStart: null,
    }),
  );
  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.beginScheduledAutoStart).not.toHaveBeenCalled();
});

test("starts when capture readiness becomes available", async () => {
  mocks.canStart = false;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.canStart = true;
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
});

test("abandons an armed auto-start immediately while another meeting is recording", () => {
  mocks.canStart = false;
  mocks.liveStatus = "active";

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.beginScheduledAutoStart).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
    scheduledAutoStart: null,
  });
});

test("abandons a pending auto-start when another meeting becomes active", () => {
  mocks.connectionReady = false;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);
  mocks.liveStatus = "active";
  mocks.canStart = false;

  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
    scheduledAutoStart: null,
  });
});

test("starts when the session record becomes available", async () => {
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

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
});

test("waits for the recording hook's connection before auto-starting", async () => {
  mocks.connectionReady = false;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  expect(mocks.startListening).not.toHaveBeenCalled();

  mocks.connectionReady = true;
  view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
});

test.each([
  {
    name: "the recording connection never becomes ready",
    block: () => {
      mocks.connectionReady = false;
    },
  },
  {
    name: "capture readiness never becomes available",
    block: () => {
      mocks.canStart = false;
    },
  },
])("abandons a scheduled start when $name", async ({ block }) => {
  vi.useFakeTimers();
  block();

  try {
    render(<ScheduledSessionAutoStart sessionId="session-1" />);
    await vi.advanceTimersByTimeAsync(30_000);

    expect(mocks.startListening).not.toHaveBeenCalled();
    expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
      view: null,
      autoStart: null,
      scheduledAutoStart: null,
    });
  } finally {
    vi.useRealTimers();
  }
});

test.each([
  {
    name: "the session loads locked so later meetings can start",
    initiallyAvailable: true,
  },
  {
    name: "a pending session becomes locked",
    initiallyAvailable: false,
  },
])("abandons when $name", ({ initiallyAvailable }) => {
  const lockedSession = {
    ...mocks.session!,
    locked: true,
  };
  mocks.session = initiallyAvailable ? lockedSession : null;
  const view = render(<ScheduledSessionAutoStart sessionId="session-1" />);

  if (!initiallyAvailable) {
    expect(mocks.startListening).not.toHaveBeenCalled();
    mocks.session = lockedSession;
    view.rerender(<ScheduledSessionAutoStart sessionId="session-1" />);
  }

  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
    view: null,
    autoStart: null,
    scheduledAutoStart: null,
  });
});

test("starts a locked session after it has been revealed", async () => {
  useAppLock.setState({ revealedNoteIds: { "session-1": true } });
  mocks.session = {
    ...mocks.session!,
    locked: true,
  };

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));
});

test("re-checks attendance immediately before capture starts", async () => {
  mocks.readDueScheduledSessionMeeting.mockResolvedValue(null);

  render(<ScheduledSessionAutoStart sessionId="session-1" />);

  await vi.waitFor(() =>
    expect(mocks.updateSessionTabState).toHaveBeenCalledWith(tab, {
      view: null,
      autoStart: null,
      scheduledAutoStart: null,
    }),
  );
  expect(mocks.startListening).not.toHaveBeenCalled();
  expect(mocks.beginScheduledAutoStart).not.toHaveBeenCalled();
});

test("retries the final attendance read after Strict Mode replays the effect", async () => {
  let resolveFirstRead: (value: { id: string }) => void = () => {};
  mocks.readDueScheduledSessionMeeting
    .mockReturnValueOnce(
      new Promise((resolve) => {
        resolveFirstRead = resolve;
      }),
    )
    .mockResolvedValue({ id: "event-1" });

  render(
    <StrictMode>
      <ScheduledSessionAutoStart sessionId="session-1" />
    </StrictMode>,
  );

  await vi.waitFor(() =>
    expect(mocks.readDueScheduledSessionMeeting).toHaveBeenCalledTimes(2),
  );
  await vi.waitFor(() => expect(mocks.startListening).toHaveBeenCalledTimes(1));

  resolveFirstRead({ id: "event-1" });
  await Promise.resolve();
  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});
