import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DeletedSessionData } from "~/store/zustand/undo-delete";

const mocks = vi.hoisted(() => {
  const deletedSessionData: DeletedSessionData = {
    session: {
      id: "session-1",
      user_id: "user-1",
      created_at: "2026-01-01T00:00:00Z",
      folder_id: "",
      event_json: "",
      title: "Deleted note",
      raw_md: "",
    },
    transcripts: [],
    participants: [],
    tagSessions: [],
    enhancedNotes: [],
    keyFacts: null,
    deletedAt: 1,
  };

  return {
    addDeletion: vi.fn(),
    captureSessionData: vi.fn(() => deletedSessionData),
    deleteSessionCascade: vi.fn(),
    emitTo: vi.fn(() => Promise.resolve()),
    finalizeSessionDeletion: vi.fn(),
    getAllWebviewWindows: vi.fn<
      () => Promise<Array<{ label: string; close: () => Promise<void> }>>
    >(() => Promise.resolve([])),
    getCurrentWebviewWindowLabel: vi.fn(() => "main"),
    ignoreEvent: vi.fn(),
    indexes: {},
    invalidateResource: vi.fn(),
    listen: vi.fn(),
    store: {},
    deletedSessionData,
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  emitTo: mocks.emitTo,
  listen: mocks.listen,
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getAllWebviewWindows: mocks.getAllWebviewWindows,
}));

vi.mock("@hypr/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: mocks.getCurrentWebviewWindowLabel,
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: mocks.ignoreEvent,
  }),
}));

vi.mock("~/store/tinybase/store/deleteSession", () => ({
  captureSessionData: mocks.captureSessionData,
  deleteSessionCascade: mocks.deleteSessionCascade,
  finalizeSessionDeletion: mocks.finalizeSessionDeletion,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useStore: () => mocks.store,
    useIndexes: () => mocks.indexes,
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      invalidateResource: typeof mocks.invalidateResource;
    }) => unknown,
  ) =>
    selector({
      invalidateResource: mocks.invalidateResource,
    }),
}));

vi.mock("~/store/zustand/undo-delete", () => ({
  useUndoDelete: (
    selector: (state: { addDeletion: typeof mocks.addDeletion }) => unknown,
  ) =>
    selector({
      addDeletion: mocks.addDeletion,
    }),
}));

import {
  useDeleteSession,
  useRemoteSessionDeletionUndoListener,
} from "./useDeleteSession";

describe("useDeleteSession", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.captureSessionData.mockReturnValue(mocks.deletedSessionData);
    mocks.emitTo.mockResolvedValue(undefined);
    mocks.getAllWebviewWindows.mockResolvedValue([]);
    mocks.getCurrentWebviewWindowLabel.mockReturnValue("main");
    mocks.listen.mockResolvedValue(vi.fn());
  });

  it("adds the undo deletion locally in the main window", () => {
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1", "tracking-1");
    });

    expect(mocks.ignoreEvent).toHaveBeenCalledWith("tracking-1");
    expect(mocks.captureSessionData).toHaveBeenCalledWith(
      mocks.store,
      mocks.indexes,
      "session-1",
    );
    expect(mocks.invalidateResource).toHaveBeenCalledWith(
      "sessions",
      "session-1",
    );
    expect(mocks.deleteSessionCascade).toHaveBeenCalledWith(
      mocks.store,
      mocks.indexes,
      "session-1",
      { deferFilesystemDelete: true },
    );
    expect(mocks.addDeletion).toHaveBeenCalledWith(
      mocks.deletedSessionData,
      expect.any(Function),
    );
    expect(mocks.emitTo).not.toHaveBeenCalled();
  });

  it("forwards undo data to main and closes the matching note window", async () => {
    const close = vi.fn(() => Promise.resolve());
    mocks.getCurrentWebviewWindowLabel.mockReturnValue("note-session-1");
    mocks.getAllWebviewWindows.mockResolvedValue([
      { label: "note-session-1", close },
      { label: "note-session-2", close: vi.fn() },
    ]);
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "hypr://session-deleted-for-undo",
        {
          sessionId: "session-1",
          data: mocks.deletedSessionData,
        },
      );
      expect(close).toHaveBeenCalled();
    });

    expect(mocks.addDeletion).not.toHaveBeenCalled();
  });

  it("closes the matching note window when deleting from the main window", async () => {
    const close = vi.fn(() => Promise.resolve());
    mocks.getAllWebviewWindows.mockResolvedValue([
      { label: "note-session-1", close },
    ]);
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => {
      expect(close).toHaveBeenCalled();
    });
  });

  it("still closes the standalone note window when forwarding undo data fails", async () => {
    const close = vi.fn(() => Promise.resolve());
    mocks.getCurrentWebviewWindowLabel.mockReturnValue("note-session-1");
    mocks.emitTo.mockRejectedValue(new Error("main window unavailable"));
    mocks.getAllWebviewWindows.mockResolvedValue([
      { label: "note-session-1", close },
    ]);
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => {
      expect(close).toHaveBeenCalled();
    });
  });

  it("listens for forwarded standalone note deletions in the main window", async () => {
    let handler:
      | ((event: {
          payload: { sessionId: string; data: DeletedSessionData };
        }) => void)
      | null = null;
    mocks.listen.mockImplementation((_, callback) => {
      handler = callback;
      return Promise.resolve(vi.fn());
    });

    renderHook(() => useRemoteSessionDeletionUndoListener(true));

    await waitFor(() => {
      expect(mocks.listen).toHaveBeenCalledWith(
        "hypr://session-deleted-for-undo",
        expect.any(Function),
      );
    });

    act(() => {
      handler?.({
        payload: {
          sessionId: "session-1",
          data: mocks.deletedSessionData,
        },
      });
    });

    expect(mocks.addDeletion).toHaveBeenCalledWith(
      mocks.deletedSessionData,
      expect.any(Function),
    );
    expect(mocks.invalidateResource).toHaveBeenCalledWith(
      "sessions",
      "session-1",
    );
    expect(mocks.deleteSessionCascade).toHaveBeenCalledWith(
      mocks.store,
      mocks.indexes,
      "session-1",
      { deferFilesystemDelete: true },
    );
  });
});
