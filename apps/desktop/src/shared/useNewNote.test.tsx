import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn(),
  downloadDir: vi.fn(),
  reservePendingUpload: vi.fn(),
  selectFile: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: mocks.downloadDir,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: mocks.selectFile,
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
}));

vi.mock("~/stt/pending-upload", () => ({
  reservePendingUpload: mocks.reservePendingUpload,
}));

import {
  openSessionAndListen,
  useNewNoteAndListen,
  useNewNoteAndUpload,
} from "./useNewNote";

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

it("does not rearm auto-start when opening the active live session", () => {
  useTabs.getState().openNew({
    type: "sessions",
    id: "live-session",
    state: { view: null, autoStart: null },
  });
  listenerStore.setState((state) => ({
    live: {
      ...state.live,
      status: "active",
      sessionId: "live-session",
    },
  }));

  openSessionAndListen("live-session");

  expect(useTabs.getState().currentTab).toMatchObject({
    type: "sessions",
    id: "live-session",
    state: { autoStart: null },
  });
});

it("opens the requested session without auto-start while another is live", () => {
  listenerStore.setState((state) => ({
    live: {
      ...state.live,
      status: "active",
      sessionId: "live-session",
    },
  }));

  openSessionAndListen("calendar-session");

  expect(useTabs.getState().currentTab).toMatchObject({
    type: "sessions",
    id: "calendar-session",
    state: { autoStart: null },
  });
});

it("reports a full pending-upload queue without creating a session", async () => {
  mocks.downloadDir.mockResolvedValueOnce("/downloads");
  mocks.selectFile.mockResolvedValueOnce("/downloads/meeting.mp3");
  mocks.reservePendingUpload.mockReturnValueOnce(null);
  const { result } = renderHook(() => useNewNoteAndUpload());

  await act(() => result.current("audio"));

  expect(mocks.createSession).not.toHaveBeenCalled();
  expect(mocks.toastError).toHaveBeenCalledWith(
    "Too many uploads are waiting. Open an existing upload and try again.",
  );
});

it("releases a pending-upload reservation when session creation fails", async () => {
  const cancel = vi.fn();
  mocks.downloadDir.mockResolvedValueOnce("/downloads");
  mocks.selectFile.mockResolvedValueOnce("/downloads/meeting.mp3");
  mocks.reservePendingUpload.mockReturnValueOnce({
    commit: vi.fn(),
    cancel,
  });
  mocks.createSession.mockRejectedValueOnce(new Error("database unavailable"));
  const { result } = renderHook(() => useNewNoteAndUpload());

  await expect(result.current("audio")).rejects.toThrow("database unavailable");

  expect(cancel).toHaveBeenCalledOnce();
});
