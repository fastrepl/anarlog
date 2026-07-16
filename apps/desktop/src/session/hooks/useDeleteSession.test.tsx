import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DeletedSessionData } from "~/store/zustand/undo-delete";

const mocks = vi.hoisted(() => {
  const deletedSessionData: DeletedSessionData = {
    session: {
      id: "session-1",
      title: "Deleted note",
    },
    tombstone: "2026-01-01T00:00:00Z",
    deletedAt: 1,
  };

  return {
    addDeletion: vi.fn(),
    auth: {
      session: null as any,
      supabase: null as any,
    },
    deleteSessionShareBySession: vi.fn(),
    emitTo: vi.fn(() => Promise.resolve()),
    finalizeSessionDeletion: vi.fn(),
    getAllWebviewWindows: vi.fn<
      () => Promise<Array<{ label: string; close: () => Promise<void> }>>
    >(() => Promise.resolve([])),
    getCurrentWebviewWindowLabel: vi.fn(() => "main"),
    ignoreEvent: vi.fn(),
    invalidateResource: vi.fn(),
    listenerGetState: vi.fn(),
    listenerStop: vi.fn(),
    listen: vi.fn(),
    loadManagedSharedNoteForSession: vi.fn(),
    removeDurableSharedNoteCache: vi.fn(),
    softDeleteSession: vi.fn(() => Promise.resolve(deletedSessionData)),
    toastError: vi.fn(),
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

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("~/auth", () => ({
  useOptionalAuth: () => mocks.auth,
}));

vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: mocks.ignoreEvent,
  }),
}));

vi.mock("~/session/queries", () => ({
  finalizeSessionDeletion: mocks.finalizeSessionDeletion,
  softDeleteSession: mocks.softDeleteSession,
}));

vi.mock("~/session-sharing/client", async (importOriginal) => {
  const original =
    await importOriginal<typeof import("~/session-sharing/client")>();
  return {
    ...original,
    deleteSessionShareBySession: mocks.deleteSessionShareBySession,
  };
});

vi.mock("~/shared-notes/cache", () => ({
  loadManagedSharedNoteForSession: mocks.loadManagedSharedNoteForSession,
  removeDurableSharedNoteCache: mocks.removeDurableSharedNoteCache,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: mocks.listenerGetState,
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
    mocks.auth.session = null;
    mocks.auth.supabase = null;
    mocks.loadManagedSharedNoteForSession.mockResolvedValue(null);
    mocks.removeDurableSharedNoteCache.mockResolvedValue(undefined);
    mocks.softDeleteSession.mockResolvedValue(mocks.deletedSessionData);
    mocks.emitTo.mockResolvedValue(undefined);
    mocks.getAllWebviewWindows.mockResolvedValue([]);
    mocks.getCurrentWebviewWindowLabel.mockReturnValue("main");
    mocks.listenerGetState.mockReturnValue({
      live: {
        sessionId: null,
        status: "inactive",
        loading: false,
      },
      stop: mocks.listenerStop,
    });
    mocks.listen.mockResolvedValue(vi.fn());
  });

  it("revokes a known managed share before deleting its local note", async () => {
    const shareId = "33333333-3333-4333-8333-333333333333";
    const workspaceId = "22222222-2222-4222-8222-222222222222";
    mocks.auth.session = {
      access_token: "expired-pro-access-token",
      token_type: "bearer",
      user: {
        id: "11111111-1111-4111-8111-111111111111",
        is_anonymous: false,
      },
    };
    mocks.auth.supabase = {};
    mocks.loadManagedSharedNoteForSession.mockResolvedValue({
      shareId,
      workspaceId,
      sessionId: "session-1",
    });
    mocks.deleteSessionShareBySession.mockResolvedValue({
      shareId,
      accessVersion: 4,
      deletedAt: "2026-07-17T01:00:00Z",
      wasDeleted: true,
    });
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => {
      expect(mocks.softDeleteSession).toHaveBeenCalledWith("session-1");
    });
    expect(mocks.deleteSessionShareBySession).toHaveBeenCalledWith(
      {
        session: mocks.auth.session,
        supabase: mocks.auth.supabase,
      },
      { workspaceId, sessionId: "session-1" },
    );
    expect(mocks.removeDurableSharedNoteCache).toHaveBeenCalledWith(
      mocks.auth.session.user.id,
      shareId,
    );
    expect(
      mocks.deleteSessionShareBySession.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.softDeleteSession.mock.invocationCallOrder[0]!);
  });

  it("keeps a known shared note local when remote revocation fails", async () => {
    const token = "secret-share-token";
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    mocks.auth.session = {
      access_token: "owner-access-token",
      token_type: "bearer",
      user: {
        id: "11111111-1111-4111-8111-111111111111",
        is_anonymous: false,
      },
    };
    mocks.auth.supabase = {};
    mocks.loadManagedSharedNoteForSession.mockResolvedValue({
      shareId: "33333333-3333-4333-8333-333333333333",
      workspaceId: "22222222-2222-4222-8222-222222222222",
      sessionId: "session-1",
    });
    mocks.deleteSessionShareBySession.mockRejectedValue(new Error(token));
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledOnce());
    expect(mocks.softDeleteSession).not.toHaveBeenCalled();
    expect(mocks.getAllWebviewWindows).not.toHaveBeenCalled();
    expect(JSON.stringify(consoleError.mock.calls)).not.toContain(token);
    consoleError.mockRestore();
  });

  it("deletes an unshared signed-in note without a remote mutation", async () => {
    mocks.auth.session = {
      access_token: "owner-access-token",
      token_type: "bearer",
      user: {
        id: "11111111-1111-4111-8111-111111111111",
        is_anonymous: false,
      },
    };
    mocks.auth.supabase = {};
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => {
      expect(mocks.softDeleteSession).toHaveBeenCalledWith("session-1");
    });
    expect(mocks.loadManagedSharedNoteForSession).toHaveBeenCalledWith(
      mocks.auth.session.user.id,
      "session-1",
    );
    expect(mocks.deleteSessionShareBySession).not.toHaveBeenCalled();
  });

  it("adds the undo deletion locally in the main window", async () => {
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1", "tracking-1");
    });

    await waitFor(() => {
      expect(mocks.addDeletion).toHaveBeenCalledWith(
        mocks.deletedSessionData,
        expect.any(Function),
      );
    });
    expect(mocks.ignoreEvent).toHaveBeenCalledWith("tracking-1");
    expect(mocks.softDeleteSession).toHaveBeenCalledWith("session-1");
    expect(mocks.invalidateResource).toHaveBeenCalledWith(
      "sessions",
      "session-1",
    );
    expect(mocks.emitTo).not.toHaveBeenCalled();
  });

  it("stops listening before deleting the active session", async () => {
    mocks.listenerGetState.mockReturnValue({
      live: {
        sessionId: "session-1",
        status: "active",
        loading: false,
      },
      stop: mocks.listenerStop,
    });
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    await waitFor(() => {
      expect(mocks.softDeleteSession).toHaveBeenCalledWith("session-1");
    });
    expect(mocks.listenerStop).toHaveBeenCalledTimes(1);
    expect(mocks.listenerStop.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.softDeleteSession.mock.invocationCallOrder[0],
    );
  });

  it("does not stop listening when deleting an inactive session", () => {
    mocks.listenerGetState.mockReturnValue({
      live: {
        sessionId: "session-2",
        status: "active",
        loading: false,
      },
      stop: mocks.listenerStop,
    });
    const { result } = renderHook(() => useDeleteSession());

    act(() => {
      result.current("session-1");
    });

    expect(mocks.listenerStop).not.toHaveBeenCalled();
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
    expect(mocks.softDeleteSession).not.toHaveBeenCalled();
  });
});
