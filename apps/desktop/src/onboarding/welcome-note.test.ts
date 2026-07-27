import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn(),
  execute: vi.fn(),
  listenerState: {
    live: {
      sessionId: null as string | null,
      status: "inactive",
      captureGenerationBySession: {} as Record<string, number>,
    },
    stop: vi.fn(),
  },
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => mocks.listenerState,
  },
}));

import {
  getOrCreateWelcomeSession,
  setPendingWelcomeSession,
  stopActiveWelcomeDemo,
  takePendingWelcomeSession,
} from "./welcome-note";

beforeEach(() => {
  vi.clearAllMocks();
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    removeItem: (key: string) => values.delete(key),
    setItem: (key: string, value: string) => values.set(key, value),
  });
  mocks.listenerState.live = {
    sessionId: null,
    status: "inactive",
    captureGenerationBySession: {},
  };
});

it("reuses an existing onboarding welcome note", async () => {
  mocks.execute.mockResolvedValueOnce([{ id: "welcome-session" }]);

  await expect(getOrCreateWelcomeSession()).resolves.toBe("welcome-session");
  expect(mocks.createSession).not.toHaveBeenCalled();
  expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
    "anarlog-onboarding-demo-v1",
  ]);
});

it("creates a prerecorded demo note with normal meeting metadata", async () => {
  mocks.execute.mockResolvedValueOnce([]);
  mocks.createSession.mockResolvedValueOnce("welcome-session");

  await expect(getOrCreateWelcomeSession()).resolves.toBe("welcome-session");

  const [title, , initial] = mocks.createSession.mock.calls[0];
  const event = JSON.parse(initial.event_json);
  expect(title).toBe("Welcome to Anarlog");
  expect(event.meeting_link).toBe("https://anarlog.so/onboarding-demo/");
  expect(event.tracking_id).toBe("anarlog-onboarding-demo-v1");
  expect(initial.raw_md).toContain("prerecorded demo meeting");
  expect(initial.raw_md).toContain("Join & record");
  expect(initial.raw_md).toContain("Settings → Transcription");
  expect(initial.raw_md).toContain(
    "If transcription and intelligence are configured",
  );
  expect(initial.raw_md).not.toContain("Anarlog will listen, transcribe");

  const note = JSON.parse(initial.raw_md);
  expect(note.content).toHaveLength(7);
  expect(note.content[1]).toEqual({ type: "paragraph" });
  expect(note.content[3]).toEqual({ type: "paragraph" });
  expect(note.content[5]).toEqual({ type: "paragraph" });
});

it("guards empty event metadata before reading its tracking ID", async () => {
  mocks.execute.mockResolvedValueOnce([]);
  mocks.createSession.mockResolvedValueOnce("welcome-session");

  await getOrCreateWelcomeSession();

  const [query] = mocks.execute.mock.calls[0];
  expect(query).toMatch(
    /CASE\s+WHEN json_valid\(event_json\)\s+THEN json_extract\(event_json, '\$\.tracking_id'\)\s+END = \?/,
  );
});

it("carries the welcome note across a one-time onboarding relaunch", () => {
  setPendingWelcomeSession("welcome-session");

  expect(takePendingWelcomeSession()).toBe("welcome-session");
  expect(takePendingWelcomeSession()).toBeNull();
});

it("stops an active welcome demo after its browser callback", async () => {
  mocks.listenerState.live = {
    sessionId: "welcome-session",
    status: "active",
    captureGenerationBySession: { "welcome-session": 1 },
  };
  mocks.execute.mockResolvedValueOnce([{ id: "welcome-session" }]);

  await stopActiveWelcomeDemo();

  expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
    "welcome-session",
    "anarlog-onboarding-demo-v1",
  ]);
  expect(mocks.listenerState.stop).toHaveBeenCalledOnce();
});

it("ignores a demo callback while another session is active", async () => {
  mocks.listenerState.live = {
    sessionId: "regular-session",
    status: "active",
    captureGenerationBySession: { "regular-session": 1 },
  };
  mocks.execute.mockResolvedValueOnce([]);

  await stopActiveWelcomeDemo();

  expect(mocks.listenerState.stop).not.toHaveBeenCalled();
});

it("does not stop a newer capture of the welcome session", async () => {
  mocks.listenerState.live = {
    sessionId: "welcome-session",
    status: "active",
    captureGenerationBySession: { "welcome-session": 1 },
  };
  let finishQuery!: (rows: { id: string }[]) => void;
  mocks.execute.mockReturnValueOnce(
    new Promise((resolve) => {
      finishQuery = resolve;
    }),
  );

  const completion = stopActiveWelcomeDemo();
  mocks.listenerState.live = {
    sessionId: "welcome-session",
    status: "active",
    captureGenerationBySession: { "welcome-session": 2 },
  };
  finishQuery([{ id: "welcome-session" }]);
  await completion;

  expect(mocks.listenerState.stop).not.toHaveBeenCalled();
});

it("ignores a demo callback when listening already stopped", async () => {
  mocks.listenerState.live = {
    sessionId: "welcome-session",
    status: "inactive",
    captureGenerationBySession: {},
  };

  await stopActiveWelcomeDemo();

  expect(mocks.execute).not.toHaveBeenCalled();
  expect(mocks.listenerState.stop).not.toHaveBeenCalled();
});
