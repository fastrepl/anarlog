import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AuthProvider, useAuth } from "./context";

const mocks = vi.hoisted(() => ({
  authCallback: null as
    | ((event: AuthChangeEvent, session: Session | null) => void)
    | null,
  claimCloudsyncAccountForAuth: vi.fn(),
  clearAuthStorage: vi.fn(),
  getSession: vi.fn(),
  handleCloudsyncAuthChange: vi.fn(),
  isFatalSessionError: vi.fn(),
  persistAuthSession: vi.fn(),
  prepareCloudsyncSignOut: vi.fn(),
  refreshSession: vi.fn(),
  signOut: vi.fn(),
  startAutoRefresh: vi.fn(),
  stopAutoRefresh: vi.fn(),
}));

vi.mock("./client", () => ({
  persistAuthSession: mocks.persistAuthSession,
  supabase: {
    auth: {
      getSession: mocks.getSession,
      onAuthStateChange: vi.fn(
        (
          callback: (event: AuthChangeEvent, session: Session | null) => void,
        ) => {
          mocks.authCallback = callback;
          return {
            data: {
              subscription: {
                unsubscribe: vi.fn(),
              },
            },
          };
        },
      ),
      refreshSession: mocks.refreshSession,
      setSession: vi.fn(),
      signOut: mocks.signOut,
      startAutoRefresh: mocks.startAutoRefresh,
      stopAutoRefresh: mocks.stopAutoRefresh,
    },
  },
}));

vi.mock("./cloudsync", () => ({
  claimCloudsyncAccountForAuth: mocks.claimCloudsyncAccountForAuth,
  handleCloudsyncAuthChange: mocks.handleCloudsyncAuthChange,
  prepareCloudsyncSignOut: mocks.prepareCloudsyncSignOut,
}));

vi.mock("./errors", () => ({
  clearAuthStorage: mocks.clearAuthStorage,
  isFatalSessionError: mocks.isFatalSessionError,
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn(),
    identify: vi.fn(),
  },
}));

vi.mock("@hypr/plugin-auth", () => ({
  commands: {
    decodeClaims: vi.fn().mockResolvedValue({ status: "error" }),
  },
}));

vi.mock("@hypr/plugin-misc", () => ({
  commands: {
    getFingerprint: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: "fingerprint" }),
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openUrl: vi.fn(),
  },
}));

vi.mock("@hypr/plugin-windows", () => ({
  openUrlWithInstruction: vi.fn(),
}));

vi.mock("@hypr/supabase", () => ({
  deriveBillingInfo: vi.fn(() => ({ plan: "free", trialEnd: null })),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("1.0.0"),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn().mockResolvedValue(vi.fn()),
  })),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: vi.fn(() => "macos"),
  version: vi.fn(() => "1.0.0"),
}));

vi.mock("~/shared/utils", () => ({
  buildWebAppUrl: vi.fn(),
  DEVICE_FINGERPRINT_HEADER: "x-device-fingerprint",
  id: vi.fn(() => "request-id"),
  REQUEST_ID_HEADER: "x-request-id",
}));

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function makeSession(userId: string): Session {
  return {
    access_token: `access-${userId}`,
    expires_at: 4_102_444_800,
    expires_in: 3_600,
    refresh_token: `refresh-${userId}`,
    token_type: "bearer",
    user: {
      app_metadata: {},
      aud: "authenticated",
      created_at: "2026-01-01T00:00:00.000Z",
      id: userId,
      user_metadata: {},
    },
  };
}

function SessionProbe() {
  const { refreshSession, session, signOut } = useAuth();
  return (
    <>
      <div data-testid="session">{session?.user.id ?? "none"}</div>
      <div data-testid="access-token">{session?.access_token ?? "none"}</div>
      <button onClick={() => void refreshSession().catch(() => {})}>
        Refresh
      </button>
      <button onClick={() => void signOut().catch(() => {})}>Sign out</button>
    </>
  );
}

function renderAuthProvider() {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <SessionProbe />
      </AuthProvider>
    </QueryClientProvider>,
  );
}

describe("AuthProvider", () => {
  beforeEach(() => {
    mocks.authCallback = null;
    mocks.claimCloudsyncAccountForAuth.mockReset();
    mocks.clearAuthStorage.mockReset();
    mocks.getSession.mockReset();
    mocks.handleCloudsyncAuthChange.mockReset();
    mocks.isFatalSessionError.mockReset();
    mocks.persistAuthSession.mockReset();
    mocks.prepareCloudsyncSignOut.mockReset();
    mocks.refreshSession.mockReset();
    mocks.signOut.mockReset();
    mocks.startAutoRefresh.mockReset();
    mocks.stopAutoRefresh.mockReset();
    mocks.clearAuthStorage.mockResolvedValue(undefined);
    mocks.claimCloudsyncAccountForAuth.mockResolvedValue(true);
    mocks.getSession.mockImplementation(() => new Promise(() => {}));
    mocks.handleCloudsyncAuthChange.mockResolvedValue("ok");
    mocks.isFatalSessionError.mockReturnValue(false);
    mocks.persistAuthSession.mockResolvedValue(undefined);
    mocks.prepareCloudsyncSignOut.mockResolvedValue(undefined);
    mocks.refreshSession.mockResolvedValue({
      data: { session: null },
      error: null,
    });
    mocks.signOut.mockResolvedValue({ error: null });
    mocks.startAutoRefresh.mockResolvedValue(undefined);
    mocks.stopAutoRefresh.mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
  });

  it("keeps a session hidden until its local account claim succeeds", async () => {
    const nextSession = makeSession("bound-account");
    const claim = deferred<boolean>();
    mocks.claimCloudsyncAccountForAuth.mockReturnValue(claim.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });

    await waitFor(() => {
      expect(mocks.claimCloudsyncAccountForAuth).toHaveBeenCalledWith(
        nextSession.user.id,
      );
    });
    expect(screen.getByTestId("session").textContent).toBe("none");
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
    );

    await act(async () => {
      claim.resolve(true);
      await claim.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        nextSession.user.id,
      );
    });
  });

  it("keeps a refreshed session hidden until admission succeeds", async () => {
    const currentSession = makeSession("bound-account");
    const refreshedSession = {
      ...currentSession,
      access_token: "refreshed-access-token",
    };

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });
    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        currentSession.access_token,
      );
    });

    const claim = deferred<boolean>();
    mocks.claimCloudsyncAccountForAuth.mockReturnValueOnce(claim.promise);
    mocks.refreshSession.mockImplementationOnce(async () => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
      return { data: { session: refreshedSession }, error: null };
    });

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => {
      expect(mocks.claimCloudsyncAccountForAuth).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByTestId("access-token").textContent).toBe(
      currentSession.access_token,
    );

    await act(async () => {
      claim.resolve(true);
      await claim.promise;
    });
    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        refreshedSession.access_token,
      );
    });
  });

  it("rejects a different local database account before admission", async () => {
    const foreignSession = makeSession("foreign-account");
    mocks.claimCloudsyncAccountForAuth.mockResolvedValue(false);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });
    expect(screen.getByTestId("session").textContent).toBe("none");
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_IN",
      foreignSession,
    );
  });

  it("fails closed when the local database account cannot be verified", async () => {
    const nextSession = makeSession("unverified-account");
    mocks.claimCloudsyncAccountForAuth.mockRejectedValue(
      new Error("database unavailable"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });
    expect(screen.getByTestId("session").textContent).toBe("none");
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
    );
  });

  it("restores a newer accepted session when stale mismatch cleanup was already running", async () => {
    const oldSession = makeSession("old-account");
    const newSession = makeSession("new-account");
    const clear = deferred();
    mocks.handleCloudsyncAuthChange.mockImplementation(
      async (_event: AuthChangeEvent, nextSession: Session | null) =>
        nextSession?.user.id === oldSession.user.id ? "account_mismatch" : "ok",
    );
    mocks.clearAuthStorage.mockReturnValue(clear.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", oldSession);
    });

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", newSession);
    });

    await act(async () => {
      clear.resolve();
      await clear.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        newSession.user.id,
      );
    });

    expect(mocks.persistAuthSession).toHaveBeenCalledWith(newSession);
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_IN",
      newSession,
    );
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("clears and suspends the current session on an actual account mismatch", async () => {
    const foreignSession = makeSession("foreign-account");
    mocks.handleCloudsyncAuthChange.mockImplementation(
      async (event: AuthChangeEvent) =>
        event === "SIGNED_IN" ? "account_mismatch" : "ok",
    );
    mocks.signOut.mockImplementation(async () => {
      mocks.authCallback?.("SIGNED_OUT", null);
      return { error: null };
    });

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });

    await waitFor(() => {
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "SIGNED_OUT",
        null,
      );
    });

    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    expect(mocks.stopAutoRefresh).toHaveBeenCalled();
    expect(mocks.signOut).toHaveBeenCalledTimes(1);
    expect(mocks.signOut).toHaveBeenCalledWith({ scope: "local" });
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe("none");
  });

  it("admits the bound account when CloudSync credential exchange stays offline", async () => {
    const localSession = makeSession("bound-account");
    mocks.handleCloudsyncAuthChange.mockResolvedValue("ok");

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", localSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        localSession.user.id,
      );
    });

    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_IN",
      localSession,
    );
  });

  it("does not let delayed fatal initial cleanup erase a newer session", async () => {
    const fatalError = new Error("invalid refresh token");
    const initialSession = deferred<{
      data: { session: null };
      error: Error;
    }>();
    const clear = deferred();
    const newSession = makeSession("new-account");
    mocks.getSession.mockReturnValue(initialSession.promise);
    mocks.isFatalSessionError.mockImplementation(
      (error: unknown) => error === fatalError,
    );
    mocks.clearAuthStorage.mockReturnValue(clear.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    await act(async () => {
      initialSession.resolve({
        data: { session: null },
        error: fatalError,
      });
      await initialSession.promise;
    });

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", newSession);
    });

    await act(async () => {
      clear.resolve();
      await clear.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        newSession.user.id,
      );
    });

    expect(mocks.persistAuthSession).toHaveBeenCalledWith(newSession);
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("clears fatal initial storage after the auth subscription initializes", async () => {
    const fatalError = new Error("invalid refresh token");
    const initialSession = deferred<{
      data: { session: null };
      error: Error;
    }>();
    mocks.getSession.mockReturnValue(initialSession.promise);
    mocks.isFatalSessionError.mockImplementation(
      (error: unknown) => error === fatalError,
    );

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("INITIAL_SESSION", null);
    });

    await act(async () => {
      initialSession.resolve({
        data: { session: null },
        error: fatalError,
      });
      await initialSession.promise;
    });

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });

    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("does not run delayed explicit sign-out cleanup after a newer token refresh", async () => {
    const oldSession = makeSession("bound-account");
    const refreshedSession = makeSession("bound-account");
    const signOut = deferred<{ error: null }>();
    mocks.signOut.mockReturnValue(signOut.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", oldSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        oldSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.signOut).toHaveBeenCalledTimes(1);
    });

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await act(async () => {
      signOut.resolve({ error: null });
      await signOut.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        refreshedSession.user.id,
      );
    });

    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("keeps the account signed in when CloudSync has unsent changes", async () => {
    const localSession = makeSession("bound-account");
    mocks.prepareCloudsyncSignOut.mockRejectedValue(
      new Error("cloudsync has unsent local changes"),
    );

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", localSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        localSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.prepareCloudsyncSignOut).toHaveBeenCalledWith(localSession);
    });

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe(
      localSession.user.id,
    );
  });

  it("restores a newer session when explicit sign-out cleanup was already clearing storage", async () => {
    const oldSession = makeSession("bound-account");
    const refreshedSession = makeSession("bound-account");
    const clear = deferred();
    mocks.clearAuthStorage.mockReturnValue(clear.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", oldSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        oldSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await act(async () => {
      clear.resolve();
      await clear.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        refreshedSession.user.id,
      );
    });

    expect(mocks.persistAuthSession).toHaveBeenCalledWith(refreshedSession);
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });
});
