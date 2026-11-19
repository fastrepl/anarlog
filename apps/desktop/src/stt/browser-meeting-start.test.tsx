import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import {
  BrowserMeetingStartBridge,
  START_GO_EVENT,
  START_REPLY_EVENT,
  START_REQUEST_EVENT,
} from "./browser-meeting-start";

const mocks = vi.hoisted(() => ({
  handlers: new Map<string, Set<(event: { payload: unknown }) => void>>(),
  replies: [] as Array<Record<string, unknown>>,
  engine: "native_meeting" as "native_meeting" | "legacy",
  getMeetingCaptureCapability: vi.fn(),
  createSession: vi.fn(async () => "note-1"),
  openNew: vi.fn(),
  live: {
    loading: false,
    status: "inactive",
    sessionId: null as string | null,
  },
  recorded: [] as Array<{ sessionId: string; extensionStart?: string }>,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (name: string, handler: (event: { payload: unknown }) => void) => {
      const handlers = mocks.handlers.get(name) ?? new Set();
      handlers.add(handler);
      mocks.handlers.set(name, handlers);
      return () => handlers.delete(handler);
    },
  ),
  emit: vi.fn(async (name: string, payload: Record<string, unknown>) => {
    if (name === START_REPLY_EVENT) {
      mocks.replies.push(payload);
    }
  }),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    getMeetingCaptureCapability: mocks.getMeetingCaptureCapability,
  },
}));

vi.mock("~/session/queries", () => ({ createSession: mocks.createSession }));
vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "microphone_device" ? "USB Mic" : undefined,
}));
vi.mock("~/sidebar/note-filter", () => ({
  folderIdForNewNote: () => undefined,
  useSidebarNotes: { getState: () => ({}) },
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: { getState: () => ({ openNew: mocks.openNew }) },
}));
vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: { getState: () => ({ live: mocks.live }) },
}));

// The record button's own path, which applies the person's selected providers; this bridge only
// decides which note it records, when, and for which request.
vi.mock("./useStartListeningWithBatchOverride", () => ({
  useStartListeningWithBatchOverride:
    (sessionId: string, options?: { extensionStart?: string }) => async () => {
      mocks.recorded.push({
        sessionId,
        extensionStart: options?.extensionStart,
      });
    },
}));

function deliver(name: string, payload: unknown) {
  for (const handler of mocks.handlers.get(name) ?? []) {
    handler({ payload });
  }
}

async function requestStart(requestId: string) {
  await waitFor(() =>
    expect(mocks.handlers.get(START_REQUEST_EVENT)?.size).toBe(1),
  );
  act(() => deliver(START_REQUEST_EVENT, { requestId }));
}

beforeEach(() => {
  mocks.handlers.clear();
  mocks.replies.length = 0;
  mocks.recorded.length = 0;
  mocks.engine = "native_meeting";
  Object.assign(mocks.live, {
    loading: false,
    status: "inactive",
    sessionId: null,
  });
  mocks.getMeetingCaptureCapability.mockImplementation(async () => ({
    status: "ok",
    data: { engine: mocks.engine },
  }));
  mocks.createSession.mockClear();
  mocks.openNew.mockClear();
});

afterEach(cleanup);

test("records a new note with the record path, for its request, only once the app says go", async () => {
  render(<BrowserMeetingStartBridge />);
  await requestStart("extension-start-1");

  await waitFor(() =>
    expect(mocks.replies).toEqual([
      {
        status: "session",
        requestId: "extension-start-1",
        sessionId: "note-1",
      },
    ]),
  );
  expect(mocks.getMeetingCaptureCapability).toHaveBeenCalledWith("USB Mic");
  expect(mocks.openNew).toHaveBeenCalledWith({
    type: "sessions",
    id: "note-1",
  });
  // The app hasn't reserved the note for the extension yet.
  expect(mocks.recorded).toEqual([]);

  act(() =>
    deliver(START_GO_EVENT, {
      requestId: "someone-else",
      sessionId: "note-1",
      go: true,
    }),
  );
  expect(mocks.recorded).toEqual([]);
  act(() =>
    deliver(START_GO_EVENT, {
      requestId: "extension-start-1",
      sessionId: "note-1",
      go: true,
    }),
  );

  await waitFor(() =>
    expect(mocks.replies[mocks.replies.length - 1]).toEqual({
      status: "done",
      requestId: "extension-start-1",
      sessionId: "note-1",
      message: null,
    }),
  );
  expect(mocks.recorded).toEqual([
    { sessionId: "note-1", extensionStart: "extension-start-1" },
  ]);
});

test("refuses without a note when the selected microphone records on the legacy engine", async () => {
  mocks.engine = "legacy";
  render(<BrowserMeetingStartBridge />);
  await requestStart("extension-start-1");

  await waitFor(() =>
    expect(mocks.replies).toEqual([
      expect.objectContaining({
        status: "refused",
        requestId: "extension-start-1",
      }),
    ]),
  );
  expect(mocks.createSession).not.toHaveBeenCalled();
  expect(mocks.recorded).toEqual([]);
});

test("refuses while Anarlog is already recording", async () => {
  Object.assign(mocks.live, { status: "active", sessionId: "other-note" });
  render(<BrowserMeetingStartBridge />);
  await requestStart("extension-start-1");

  await waitFor(() =>
    expect(mocks.replies[0]).toEqual(
      expect.objectContaining({ status: "refused" }),
    ),
  );
  expect(mocks.createSession).not.toHaveBeenCalled();
  expect(mocks.getMeetingCaptureCapability).not.toHaveBeenCalled();
});

test("never records a note the app dropped, then runs the next request", async () => {
  render(<BrowserMeetingStartBridge />);
  await requestStart("extension-start-1");
  await waitFor(() => expect(mocks.replies).toHaveLength(1));
  act(() =>
    deliver(START_GO_EVENT, {
      requestId: "extension-start-1",
      sessionId: "note-1",
      go: false,
    }),
  );
  await waitFor(() =>
    expect(mocks.handlers.get(START_GO_EVENT)?.size ?? 0).toBe(0),
  );
  expect(mocks.recorded).toEqual([]);

  mocks.createSession.mockResolvedValueOnce("note-2");
  act(() => deliver(START_REQUEST_EVENT, { requestId: "extension-start-2" }));
  await waitFor(() => expect(mocks.replies).toHaveLength(2));
  act(() =>
    deliver(START_GO_EVENT, {
      requestId: "extension-start-2",
      sessionId: "note-2",
      go: true,
    }),
  );
  await waitFor(() =>
    expect(mocks.replies[mocks.replies.length - 1]).toEqual({
      status: "done",
      requestId: "extension-start-2",
      sessionId: "note-2",
      message: null,
    }),
  );
  expect(mocks.recorded).toEqual([
    { sessionId: "note-2", extensionStart: "extension-start-2" },
  ]);
});
