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
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAuth } from "./auth-context";
import type { CloudsyncAccountAdmission } from "./cloudsync";
import * as authProviderModule from "./context";

const { AuthProvider } = authProviderModule;

const mocks = vi.hoisted(() => ({
  analyticsClearGroups: vi.fn(),
  analyticsEvent: vi.fn(),
  analyticsIdentify: vi.fn(),
  authCallback: null as
    | ((event: AuthChangeEvent, session: Session | null) => void)
    | null,
  bindCloudsyncAccountForAuth: vi.fn(),
  clearAuthStorage: vi.fn(),
  currentWebviewWindowLabel: "main",
  emit: vi.fn(),
  emitTo: vi.fn(),
  eventCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
  focusCallback: null as ((event: { payload: boolean }) => void) | null,
  getSession: vi.fn(),
  handleCloudsyncAuthChange: vi.fn(),
  persistAuthSession: vi.fn(),
  readPersistedAuthSession: vi.fn(),
  prepareCloudsyncSignOut: vi.fn(),
  refreshCloudsyncForSession: vi.fn(),
  refreshSession: vi.fn(),
  signOut: vi.fn(),
  startAutoRefresh: vi.fn(),
  stopAutoRefresh: vi.fn(),
  toastDismiss: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("./client", () => ({
  persistAuthSession: mocks.persistAuthSession,
  readPersistedAuthSession: mocks.readPersistedAuthSession,
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
  bindCloudsyncAccountForAuth: mocks.bindCloudsyncAccountForAuth,
  handleCloudsyncAuthChange: mocks.handleCloudsyncAuthChange,
  prepareCloudsyncSignOut: mocks.prepareCloudsyncSignOut,
  refreshCloudsyncForSession: mocks.refreshCloudsyncForSession,
}));

vi.mock("./errors", () => ({
  clearAuthStorage: mocks.clearAuthStorage,
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    clearGroups: mocks.analyticsClearGroups,
    event: mocks.analyticsEvent,
    identify: mocks.analyticsIdentify,
  },
}));

vi.mock("@anlg/plugin-auth", () => ({
  commands: {
    decodeClaims: vi.fn().mockResolvedValue({ status: "error" }),
  },
}));

vi.mock("@anlg/plugin-misc", () => ({
  commands: {
    getFingerprint: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: "fingerprint" }),
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: {
    openUrl: vi.fn(),
  },
}));

vi.mock("@anlg/plugin-windows", () => ({
  openUrlWithInstruction: vi.fn(),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    dismiss: mocks.toastDismiss,
    error: mocks.toastError,
  },
}));

vi.mock("@anlg/supabase", () => ({
  deriveBillingInfo: vi.fn(() => ({ plan: "free", trialEnd: null })),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("1.0.0"),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
  emitTo: mocks.emitTo,
  listen: vi.fn(
    (event: string, callback: (event: { payload: unknown }) => void) => {
      mocks.eventCallbacks.set(event, callback);
      return Promise.resolve(() => {
        if (mocks.eventCallbacks.get(event) === callback) {
          mocks.eventCallbacks.delete(event);
        }
      });
    },
  ),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: vi.fn(() => ({
    label: mocks.currentWebviewWindowLabel,
  })),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn((callback: (event: { payload: boolean }) => void) => {
      mocks.focusCallback = callback;
      return Promise.resolve(vi.fn());
    }),
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
  const { getHeaders, getSessionForRequest, refreshSession, session, signOut } =
    useAuth();
  const [requestAccessToken, setRequestAccessToken] = useState("none");
  return (
    <>
      <div data-testid="session">{session?.user.id ?? "none"}</div>
      <div data-testid="access-token">{session?.access_token ?? "none"}</div>
      <div data-testid="authorization">
        {getHeaders()?.Authorization ?? "none"}
      </div>
      <div data-testid="request-access-token">{requestAccessToken}</div>
      <button onClick={() => void refreshSession().catch(() => {})}>
        Refresh
      </button>
      <button
        onClick={() =>
          void getSessionForRequest().then((requestSession) => {
            setRequestAccessToken(requestSession?.access_token ?? "none");
          })
        }
      >
        Get request session
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
  it("keeps the provider module compatible with Fast Refresh", () => {
    expect(Object.keys(authProviderModule)).toEqual(["AuthProvider"]);
  });

  beforeEach(() => {
    mocks.analyticsClearGroups.mockReset();
    mocks.analyticsEvent.mockReset();
    mocks.analyticsIdentify.mockReset();
    mocks.authCallback = null;
    mocks.bindCloudsyncAccountForAuth.mockReset();
    mocks.clearAuthStorage.mockReset();
    mocks.currentWebviewWindowLabel = "main";
    mocks.emit.mockReset();
    mocks.emitTo.mockReset();
    mocks.eventCallbacks.clear();
    mocks.focusCallback = null;
    mocks.getSession.mockReset();
    mocks.handleCloudsyncAuthChange.mockReset();
    mocks.persistAuthSession.mockReset();
    mocks.readPersistedAuthSession.mockReset();
    mocks.prepareCloudsyncSignOut.mockReset();
    mocks.refreshCloudsyncForSession.mockReset();
    mocks.refreshSession.mockReset();
    mocks.signOut.mockReset();
    mocks.startAutoRefresh.mockReset();
    mocks.stopAutoRefresh.mockReset();
    mocks.toastDismiss.mockReset();
    mocks.toastError.mockReset();
    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("claimed");
    mocks.clearAuthStorage.mockResolvedValue(undefined);
    mocks.emit.mockResolvedValue(undefined);
    mocks.emitTo.mockResolvedValue(undefined);
    mocks.getSession.mockImplementation(() => new Promise(() => {}));
    mocks.handleCloudsyncAuthChange.mockResolvedValue("ok");
    mocks.persistAuthSession.mockResolvedValue(undefined);
    mocks.readPersistedAuthSession.mockResolvedValue(null);
    mocks.prepareCloudsyncSignOut.mockResolvedValue(undefined);
    mocks.refreshCloudsyncForSession.mockResolvedValue("ok");
    mocks.refreshSession.mockResolvedValue({
      data: { session: null },
      error: null,
    });
    mocks.signOut.mockResolvedValue({ error: null });
    mocks.startAutoRefresh.mockResolvedValue(undefined);
    mocks.stopAutoRefresh.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it("associates signed-in analytics with the account group", async () => {
    const currentSession = makeSession("account-id");
    currentSession.user.email = "person@example.com";

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(mocks.analyticsIdentify).toHaveBeenCalledWith(
        "account-id",
        expect.objectContaining({
          group: {
            type: "account",
            key: "account-id",
            properties: {
              name: "person@example.com",
              email: "person@example.com",
              created_at: "2026-01-01T00:00:00.000Z",
              plan: "free",
              trial_end_date: null,
            },
          },
        }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.analyticsClearGroups).toHaveBeenCalledTimes(1);
    });
  });

  it("clears account groups after an in-flight identify settles", async () => {
    const identify = deferred();
    mocks.analyticsIdentify.mockReturnValueOnce(identify.promise);
    const currentSession = makeSession("account-id");

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(mocks.analyticsIdentify).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });
    expect(mocks.analyticsClearGroups).not.toHaveBeenCalled();

    identify.resolve();

    await waitFor(() => {
      expect(mocks.analyticsClearGroups).toHaveBeenCalledTimes(1);
    });
  });

  it("clears account groups after the sign-in event settles", async () => {
    const signInEvent = deferred();
    mocks.analyticsEvent.mockReturnValueOnce(signInEvent.promise);
    const currentSession = makeSession("event-account-id");

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(mocks.analyticsEvent).toHaveBeenCalledWith({
        event: "user_signed_in",
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });
    expect(mocks.analyticsClearGroups).not.toHaveBeenCalled();

    signInEvent.resolve();

    await waitFor(() => {
      expect(mocks.analyticsClearGroups).toHaveBeenCalledTimes(1);
    });
  });

  it("refreshes cloudsync when the main window regains focus", async () => {
    const currentSession = makeSession("bound-account");

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.focusCallback).not.toBeNull();
    });

    mocks.getSession.mockResolvedValue({
      data: { session: currentSession },
      error: null,
    });

    act(() => {
      mocks.focusCallback?.({ payload: true });
    });

    await waitFor(() => {
      expect(mocks.refreshCloudsyncForSession).toHaveBeenCalledWith(
        currentSession,
        expect.any(Function),
      );
    });
    expect(mocks.refreshSession).not.toHaveBeenCalled();
  });

  it("rejects a mismatched account found during focus recovery", async () => {
    const foreignSession = makeSession("foreign-account");
    mocks.refreshCloudsyncForSession.mockResolvedValueOnce("account_mismatch");
    mocks.signOut.mockImplementationOnce(async () => {
      mocks.authCallback?.("SIGNED_OUT", null);
      return { error: null };
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
      expect(mocks.focusCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        foreignSession.user.id,
      );
    });

    mocks.getSession.mockResolvedValueOnce({
      data: { session: foreignSession },
      error: null,
    });

    act(() => {
      mocks.focusCallback?.({ payload: true });
    });

    await waitFor(() => {
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "SIGNED_OUT",
        null,
      );
    });

    expect(mocks.refreshCloudsyncForSession).toHaveBeenCalledWith(
      foreignSession,
      expect.any(Function),
    );
    expect(mocks.stopAutoRefresh).toHaveBeenCalled();
    expect(mocks.signOut).toHaveBeenCalledTimes(1);
    expect(mocks.signOut).toHaveBeenCalledWith({ scope: "local" });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("session").textContent).toBe("none");
  });

  it("refreshes an expiring session before cloudsync when focus returns", async () => {
    const staleSession = makeSession("bound-account");
    staleSession.expires_at = Math.floor(Date.now() / 1000) + 60;
    const refreshedSession = makeSession("bound-account");
    refreshedSession.access_token = "refreshed-access-token";

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.focusCallback).not.toBeNull();
    });

    mocks.getSession.mockResolvedValueOnce({
      data: { session: staleSession },
      error: null,
    });
    mocks.refreshSession.mockImplementationOnce(async () => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
      return {
        data: { session: refreshedSession },
        error: null,
      };
    });

    act(() => {
      mocks.focusCallback?.({ payload: true });
    });

    await waitFor(() => {
      expect(mocks.refreshSession).toHaveBeenCalledTimes(1);
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "TOKEN_REFRESHED",
        refreshedSession,
        expect.any(Function),
      );
    });
    expect(mocks.refreshCloudsyncForSession).not.toHaveBeenCalled();
  });

  it("refreshes an expiring session before an authenticated request", async () => {
    const staleSession = makeSession("bound-account");
    staleSession.expires_at = Math.floor(Date.now() / 1000) + 60;
    const refreshedSession = makeSession("bound-account");
    mocks.getSession.mockResolvedValue({
      data: { session: staleSession },
      error: null,
    });
    mocks.refreshSession.mockResolvedValueOnce({
      data: { session: refreshedSession },
      error: null,
    });

    renderAuthProvider();

    fireEvent.click(
      screen.getByRole("button", { name: "Get request session" }),
    );

    await waitFor(() => {
      expect(mocks.refreshSession).toHaveBeenCalledTimes(1);
    });
  });

  it("uses the current session when the SDK lookup fails", async () => {
    const currentSession = makeSession("bound-account");

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    mocks.getSession.mockRejectedValueOnce(new Error("offline"));
    fireEvent.click(
      screen.getByRole("button", { name: "Get request session" }),
    );

    await waitFor(() => {
      expect(screen.getByTestId("request-access-token").textContent).toBe(
        currentSession.access_token,
      );
    });
  });

  it("uses a still-valid session when proactive refresh fails", async () => {
    const currentSession = makeSession("bound-account");
    currentSession.expires_at = Math.floor(Date.now() / 1000) + 60;

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    mocks.getSession.mockResolvedValueOnce({
      data: { session: currentSession },
      error: null,
    });
    mocks.refreshSession.mockRejectedValueOnce(new Error("offline"));
    fireEvent.click(
      screen.getByRole("button", { name: "Get request session" }),
    );

    await waitFor(() => {
      expect(screen.getByTestId("request-access-token").textContent).toBe(
        currentSession.access_token,
      );
    });
  });

  it("only runs cloudsync from the main window", async () => {
    const currentSession = makeSession("bound-account");
    mocks.currentWebviewWindowLabel = "note-session-id";

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
      expect(mocks.focusCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
      mocks.focusCallback?.({ payload: true });
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledWith(
      currentSession.user.id,
    );
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalled();
    expect(mocks.refreshCloudsyncForSession).not.toHaveBeenCalled();
  });

  it("routes secondary-window account rejection through the main window", async () => {
    const foreignSession = makeSession("foreign-account");
    mocks.currentWebviewWindowLabel = "note-session-id";
    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("mismatch");
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "anlg:auth-sign-out-request",
        {
          requestId: "request-id",
          sourceLabel: "note-session-id",
        },
      );
    });
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-result")?.({
        payload: { requestId: "request-id", completed: true, error: null },
      });
    });

    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe("none");
  });

  it("preserves shared auth when main rejects secondary cleanup", async () => {
    const foreignSession = makeSession("foreign-account");
    mocks.currentWebviewWindowLabel = "note-session-id";
    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("mismatch");
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "anlg:auth-sign-out-request",
        expect.objectContaining({ sourceLabel: "note-session-id" }),
      );
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-result")?.({
        payload: { requestId: "request-id", completed: false, error: null },
      });
    });

    await waitFor(() => {
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-result")).toBe(false);
    });
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe("none");
  });

  it("routes secondary-window sign-out through the main window", async () => {
    const currentSession = makeSession("bound-account");
    mocks.currentWebviewWindowLabel = "note-session-id";
    mocks.signOut.mockImplementationOnce(async () => {
      mocks.authCallback?.("SIGNED_OUT", null);
      return { error: null };
    });

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "anlg:auth-sign-out-request",
        {
          requestId: "request-id",
          sourceLabel: "note-session-id",
        },
      );
    });

    expect(mocks.prepareCloudsyncSignOut).not.toHaveBeenCalled();
    expect(mocks.signOut).not.toHaveBeenCalled();

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-result")?.({
        payload: { requestId: "request-id", completed: true, error: null },
      });
    });

    await waitFor(() => {
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-result")).toBe(false);
      expect(screen.getByTestId("session").textContent).toBe("none");
    });
    expect(mocks.stopAutoRefresh).toHaveBeenCalled();
    expect(mocks.signOut).toHaveBeenCalledWith({ scope: "local" });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    expect(mocks.emitTo).toHaveBeenCalledTimes(1);
  });

  it("preserves a secondary-window session after an unsolicited SDK sign-out", async () => {
    const currentSession = makeSession("bound-account");
    const recoveredSession = {
      ...makeSession("bound-account"),
      access_token: "recovered-access-token",
    };
    mocks.currentWebviewWindowLabel = "note-session-id";
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    act(() => {
      mocks.authCallback?.("SIGNED_OUT", null);
    });

    expect(screen.getByTestId("session").textContent).toBe(
      currentSession.user.id,
    );
    expect(screen.getByTestId("authorization").textContent).toBe(
      `bearer ${currentSession.access_token}`,
    );
    expect(mocks.emitTo).not.toHaveBeenCalled();
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.signOut).not.toHaveBeenCalled();

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", recoveredSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        recoveredSession.access_token,
      );
      expect(screen.getByTestId("authorization").textContent).toBe(
        `bearer ${recoveredSession.access_token}`,
      );
    });
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
  });

  it("clears a secondary-window session after committed sign-out", async () => {
    const currentSession = makeSession("bound-account");
    mocks.currentWebviewWindowLabel = "note-session-id";

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-committed")).toBe(
        true,
      );
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-committed")?.({
        payload: { sourceLabel: "main" },
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe("none");
    });
    expect(mocks.stopAutoRefresh).toHaveBeenCalled();
    expect(mocks.signOut).toHaveBeenCalledWith({ scope: "local" });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    expect(mocks.emitTo).not.toHaveBeenCalled();
  });

  it("keeps the secondary-window session when main sign-out fails", async () => {
    const currentSession = makeSession("bound-account");
    mocks.currentWebviewWindowLabel = "note-session-id";

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "anlg:auth-sign-out-request",
        {
          requestId: "request-id",
          sourceLabel: "note-session-id",
        },
      );
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-result")?.({
        payload: {
          requestId: "request-id",
          completed: false,
          error: "cloudsync suspension failed",
        },
      });
    });

    await waitFor(() => {
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-result")).toBe(false);
    });
    expect(screen.getByTestId("session").textContent).toBe(
      currentSession.user.id,
    );
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.prepareCloudsyncSignOut).not.toHaveBeenCalled();
    expect(mocks.signOut).not.toHaveBeenCalled();
  });

  it("keeps the secondary-window session when main sign-out is superseded", async () => {
    const currentSession = makeSession("bound-account");
    mocks.currentWebviewWindowLabel = "note-session-id";

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "anlg:auth-sign-out-request",
        {
          requestId: "request-id",
          sourceLabel: "note-session-id",
        },
      );
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-result")?.({
        payload: { requestId: "request-id", completed: false, error: null },
      });
    });

    await waitFor(() => {
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-result")).toBe(false);
    });
    expect(screen.getByTestId("session").textContent).toBe(
      currentSession.user.id,
    );
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
  });

  it("runs remote sign-out in the main window and acknowledges it", async () => {
    const currentSession = makeSession("bound-account");
    mocks.signOut.mockImplementationOnce(async () => {
      mocks.authCallback?.("SIGNED_OUT", null);
      return { error: null };
    });

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-request")).toBe(true);
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-request")?.({
        payload: {
          requestId: "remote-request-id",
          sourceLabel: "note-session-id",
        },
      });
    });

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "note-session-id",
        "anlg:auth-sign-out-result",
        { requestId: "remote-request-id", completed: true, error: null },
      );
    });

    expect(mocks.prepareCloudsyncSignOut).toHaveBeenCalledWith(
      currentSession,
      expect.any(Function),
    );
    expect(mocks.signOut).toHaveBeenCalledWith({ scope: "local" });
    expect(mocks.emit).toHaveBeenCalledWith("anlg:auth-sign-out-committed", {
      sourceLabel: "main",
    });
    expect(
      mocks.prepareCloudsyncSignOut.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.signOut.mock.invocationCallOrder[0]);
  });

  it("clears the visible session before cloudsync teardown finishes", async () => {
    const currentSession = makeSession("bound-account");
    const teardown = deferred<"ok">();

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });
    mocks.handleCloudsyncAuthChange.mockReturnValueOnce(teardown.promise);

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe("none");
    });

    teardown.resolve("ok");
    await teardown.promise;
  });

  it("does not queue sign-out behind delayed CloudSync activation", async () => {
    const currentSession = makeSession("bound-account");
    const activation = deferred<"ok">();
    mocks.handleCloudsyncAuthChange.mockReturnValueOnce(activation.promise);
    mocks.signOut.mockImplementationOnce(async () => {
      mocks.authCallback?.("SIGNED_OUT", null);
      return { error: null };
    });

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "SIGNED_IN",
        currentSession,
        expect.any(Function),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe("none");
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "SIGNED_OUT",
        null,
      );
    });

    await act(async () => {
      activation.resolve("ok");
      await activation.promise;
    });

    expect(screen.getByTestId("session").textContent).toBe("none");
  });

  it("fails remote sign-out closed when the React session is empty", async () => {
    mocks.prepareCloudsyncSignOut.mockRejectedValueOnce(
      new Error("cloudsync suspension failed"),
    );
    mocks.getSession.mockResolvedValue({
      data: { session: null },
      error: null,
    });

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    await waitFor(() => {
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "INITIAL_SESSION",
        null,
        expect.any(Function),
      );
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-request")).toBe(true);
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-request")?.({
        payload: {
          requestId: "remote-request-id",
          sourceLabel: "note-session-id",
        },
      });
    });

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "note-session-id",
        "anlg:auth-sign-out-result",
        {
          requestId: "remote-request-id",
          completed: false,
          error: "cloudsync suspension failed",
        },
      );
    });

    expect(mocks.prepareCloudsyncSignOut).toHaveBeenCalledWith(
      null,
      expect.any(Function),
    );
    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
  });

  it("does not sign out a session that wins during cloudsync preflight", async () => {
    const currentSession = makeSession("bound-account");
    const refreshedSession = {
      ...makeSession("bound-account"),
      access_token: "refreshed-access-token",
    };
    const preflight = deferred();
    mocks.prepareCloudsyncSignOut.mockReturnValueOnce(preflight.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(mocks.prepareCloudsyncSignOut).toHaveBeenCalledWith(
        currentSession,
        expect.any(Function),
      );
    });

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        refreshedSession.access_token,
      );
    });

    await act(async () => {
      preflight.resolve();
      await preflight.promise;
    });

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(screen.getByTestId("access-token").textContent).toBe(
      refreshedSession.access_token,
    );
  });

  it("reports remote sign-out as incomplete when a newer auth transition wins", async () => {
    const currentSession = makeSession("bound-account");
    const refreshedSession = {
      ...makeSession("bound-account"),
      access_token: "refreshed-access-token",
    };
    const signOut = deferred<{ error: null }>();
    mocks.signOut.mockReturnValue(signOut.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-request")).toBe(true);
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-request")?.({
        payload: {
          requestId: "remote-request-id",
          sourceLabel: "note-session-id",
        },
      });
    });

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
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "note-session-id",
        "anlg:auth-sign-out-result",
        { requestId: "remote-request-id", completed: false, error: null },
      );
      expect(screen.getByTestId("access-token").textContent).toBe(
        refreshedSession.access_token,
      );
    });

    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("returns the main-window preflight error to the requesting window", async () => {
    const currentSession = makeSession("bound-account");
    mocks.prepareCloudsyncSignOut.mockRejectedValue(
      new Error("cloudsync suspension failed"),
    );

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        currentSession.user.id,
      );
      expect(mocks.eventCallbacks.has("anlg:auth-sign-out-request")).toBe(true);
    });

    act(() => {
      mocks.eventCallbacks.get("anlg:auth-sign-out-request")?.({
        payload: {
          requestId: "remote-request-id",
          sourceLabel: "note-session-id",
        },
      });
    });

    await waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "note-session-id",
        "anlg:auth-sign-out-result",
        {
          requestId: "remote-request-id",
          completed: false,
          error: "cloudsync suspension failed",
        },
      );
    });

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe(
      currentSession.user.id,
    );
  });

  it("keeps a session hidden until its local account claim succeeds", async () => {
    const nextSession = makeSession("bound-account");
    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth.mockReturnValue(claim.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });

    await waitFor(() => {
      expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledWith(
        nextSession.user.id,
      );
    });
    expect(screen.getByTestId("session").textContent).toBe("none");
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
      expect.any(Function),
    );

    await act(async () => {
      claim.resolve("claimed");
      await claim.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        nextSession.user.id,
      );
    });
  });

  it("keeps a rapid sign-out then sign-in hidden until the new account is claimed", async () => {
    const nextSession = makeSession("new-account");
    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth.mockReturnValue(claim.promise);

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_OUT", null);
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });

    await waitFor(() => {
      expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledWith(
        nextSession.user.id,
      );
    });
    expect(screen.getByTestId("session").textContent).toBe("none");
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
      expect.any(Function),
    );

    await act(async () => {
      claim.resolve("claimed");
      await claim.promise;
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        nextSession.user.id,
      );
    });
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
      expect.any(Function),
    );
  });

  it("applies a refreshed token for the admitted account without re-admission", async () => {
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

    mocks.bindCloudsyncAccountForAuth.mockReturnValueOnce(
      new Promise<CloudsyncAccountAdmission>(() => {}),
    );

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        refreshedSession.access_token,
      );
    });
    expect(screen.getByTestId("authorization").textContent).toBe(
      `bearer ${refreshedSession.access_token}`,
    );
    expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(1);
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "TOKEN_REFRESHED",
      refreshedSession,
      expect.any(Function),
    );
  });

  it("applies a refreshed token while an earlier auth transition is still blocked", async () => {
    const currentSession = makeSession("bound-account");
    const refreshedSession = {
      ...currentSession,
      access_token: "refreshed-access-token",
    };
    // The SIGNED_IN transition never finishes its CloudSync handshake, which
    // would previously have held every later auth event in the queue.
    mocks.handleCloudsyncAuthChange.mockImplementation(
      (event: AuthChangeEvent) =>
        event === "SIGNED_IN" ? new Promise(() => {}) : Promise.resolve("ok"),
    );

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", currentSession);
    });
    await waitFor(() => {
      expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
        "SIGNED_IN",
        currentSession,
        expect.any(Function),
      );
    });

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        refreshedSession.access_token,
      );
    });
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "TOKEN_REFRESHED",
      refreshedSession,
      expect.any(Function),
    );
  });

  it("keeps re-admitting refreshes for a session preserved without verification", async () => {
    const currentSession = makeSession("unverified-account");
    const refreshedSession = {
      ...currentSession,
      access_token: "refreshed-access-token",
    };
    mocks.bindCloudsyncAccountForAuth.mockRejectedValueOnce(
      new Error("database unavailable"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

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

    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth.mockReturnValueOnce(claim.promise);

    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await waitFor(() => {
      expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByTestId("access-token").textContent).toBe(
      currentSession.access_token,
    );

    await act(async () => {
      claim.resolve("claimed");
      await claim.promise;
    });
    await waitFor(() => {
      expect(screen.getByTestId("access-token").textContent).toBe(
        refreshedSession.access_token,
      );
    });
  });

  it("re-runs admission when a CloudSync refresh supersedes the local bind", async () => {
    const nextSession = makeSession("bound-account");
    mocks.bindCloudsyncAccountForAuth
      .mockResolvedValueOnce("superseded")
      .mockResolvedValueOnce("claimed");

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        nextSession.user.id,
      );
    });
    expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(2);
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
      expect.any(Function),
    );
  });

  it("does not admit an account whose bind keeps being superseded", async () => {
    const nextSession = makeSession("bound-account");
    const refreshedSession = {
      ...nextSession,
      access_token: "refreshed-access-token",
    };
    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("superseded");
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        nextSession.user.id,
      );
    });
    expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(3);
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalled();

    // Still unadmitted: the next refresh must go back through admission.
    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("mismatch");
    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe("none");
    });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
  });

  it("does not admit late when the stalled bind resolves as superseded", async () => {
    const nextSession = makeSession("bound-account");
    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth
      .mockReturnValueOnce(claim.promise)
      .mockResolvedValue("superseded");
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    vi.useFakeTimers();
    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(screen.getByTestId("session").textContent).toBe(nextSession.user.id);

    await act(async () => {
      claim.resolve("superseded");
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(3);
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe(nextSession.user.id);
  });

  it("preserves the local session when account admission stalls and finishes it late", async () => {
    const nextSession = makeSession("bound-account");
    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth.mockReturnValue(claim.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    vi.useFakeTimers();
    act(() => {
      mocks.authCallback?.("SIGNED_IN", nextSession);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(14_000);
    });
    expect(screen.getByTestId("session").textContent).toBe("none");

    const refreshStartsBeforePreserve =
      mocks.startAutoRefresh.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });
    expect(screen.getByTestId("session").textContent).toBe(nextSession.user.id);
    expect(mocks.startAutoRefresh.mock.calls.length).toBeGreaterThan(
      refreshStartsBeforePreserve,
    );
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalled();

    const refreshStartsBeforeAdmission =
      mocks.startAutoRefresh.mock.calls.length;
    await act(async () => {
      claim.resolve("claimed");
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(mocks.startAutoRefresh.mock.calls.length).toBeGreaterThan(
      refreshStartsBeforeAdmission,
    );
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
      expect.any(Function),
    );
  });

  it("writes a late-admitted session back after sign-out cleanup cleared storage", async () => {
    const oldSession = makeSession("bound-account");
    const refreshedSession = {
      ...oldSession,
      access_token: "refreshed-access-token",
    };
    const clear = deferred();
    mocks.clearAuthStorage.mockReturnValue(clear.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

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

    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth.mockReturnValueOnce(claim.promise);
    vi.useFakeTimers();
    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });
    await act(async () => {
      clear.resolve();
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(2);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(screen.getByTestId("access-token").textContent).toBe(
      refreshedSession.access_token,
    );
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();

    await act(async () => {
      claim.resolve("claimed");
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(mocks.persistAuthSession).toHaveBeenCalledWith(refreshedSession);
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "TOKEN_REFRESHED",
      refreshedSession,
      expect.any(Function),
    );
  });

  it("re-admits a refresh that arrives during account-mismatch cleanup", async () => {
    const boundSession = makeSession("bound-account");
    const refreshedSession = {
      ...boundSession,
      access_token: "refreshed-access-token",
    };
    const clear = deferred();
    mocks.clearAuthStorage.mockReturnValueOnce(clear.promise);
    mocks.handleCloudsyncAuthChange.mockResolvedValueOnce("account_mismatch");
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", boundSession);
    });
    await waitFor(() => {
      expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    });
    expect(screen.getByTestId("access-token").textContent).toBe(
      boundSession.access_token,
    );

    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("mismatch");
    act(() => {
      mocks.authCallback?.("TOKEN_REFRESHED", refreshedSession);
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByTestId("access-token").textContent).toBe(
      boundSession.access_token,
    );

    await act(async () => {
      clear.resolve();
      await clear.promise;
    });

    await waitFor(() => {
      expect(mocks.bindCloudsyncAccountForAuth).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe("none");
    });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(2);
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
  });

  it("rejects a stalled admission that later reports another account", async () => {
    const foreignSession = makeSession("foreign-account");
    const claim = deferred<CloudsyncAccountAdmission>();
    mocks.bindCloudsyncAccountForAuth.mockReturnValue(claim.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    vi.useFakeTimers();
    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(screen.getByTestId("session").textContent).toBe(
      foreignSession.user.id,
    );

    await act(async () => {
      claim.resolve("mismatch");
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("session").textContent).toBe("none");
    expect(mocks.toastError).toHaveBeenCalledWith(
      "The notes on this device are linked to another Anarlog account. Sign in with the account previously used here.",
      { id: "auth-account-mismatch" },
    );
  });

  it("rejects a different local database account before admission", async () => {
    const foreignSession = makeSession("foreign-account");
    mocks.bindCloudsyncAccountForAuth.mockResolvedValue("mismatch");
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
      expect.any(Function),
    );
    expect(mocks.toastError).toHaveBeenCalledWith(
      "The notes on this device are linked to another Anarlog account. Sign in with the account previously used here.",
      { id: "auth-account-mismatch" },
    );
  });

  it("preserves local auth when the database account cannot be verified", async () => {
    const nextSession = makeSession("unverified-account");
    mocks.bindCloudsyncAccountForAuth.mockRejectedValue(
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
      expect(screen.getByTestId("session").textContent).toBe(
        nextSession.user.id,
      );
    });
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_IN",
      nextSession,
      expect.any(Function),
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
      expect.any(Function),
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

  it("rejects the current session when scheduled CloudSync renewal reports a mismatch", async () => {
    const foreignSession = makeSession("foreign-account");
    let reportAccountMismatch: (() => Promise<void>) | undefined;
    mocks.handleCloudsyncAuthChange.mockImplementation(
      async (
        event: AuthChangeEvent,
        _session: Session | null,
        onAccountMismatch?: () => Promise<void>,
      ) => {
        if (event === "SIGNED_IN") {
          reportAccountMismatch = onAccountMismatch;
        }
        return "ok";
      },
    );

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", foreignSession);
    });

    await waitFor(() => {
      expect(reportAccountMismatch).toBeTypeOf("function");
      expect(screen.getByTestId("session").textContent).toBe(
        foreignSession.user.id,
      );
    });

    await act(async () => {
      await reportAccountMismatch?.();
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe("none");
    });
    expect(mocks.stopAutoRefresh).toHaveBeenCalled();
    expect(mocks.signOut).toHaveBeenCalledWith({ scope: "local" });
    expect(mocks.clearAuthStorage).toHaveBeenCalledTimes(1);
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("ignores a scheduled mismatch from a superseded auth transition", async () => {
    const oldSession = makeSession("old-account");
    const newSession = makeSession("new-account");
    let reportOldAccountMismatch: (() => Promise<void>) | undefined;
    mocks.handleCloudsyncAuthChange.mockImplementation(
      async (
        event: AuthChangeEvent,
        nextSession: Session | null,
        onAccountMismatch?: () => Promise<void>,
      ) => {
        if (event === "SIGNED_IN" && nextSession === oldSession) {
          reportOldAccountMismatch = onAccountMismatch;
        }
        return "ok";
      },
    );

    renderAuthProvider();

    await waitFor(() => {
      expect(mocks.authCallback).not.toBeNull();
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", oldSession);
    });

    await waitFor(() => {
      expect(reportOldAccountMismatch).toBeTypeOf("function");
      expect(screen.getByTestId("session").textContent).toBe(
        oldSession.user.id,
      );
    });

    act(() => {
      mocks.authCallback?.("SIGNED_IN", newSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        newSession.user.id,
      );
    });

    await act(async () => {
      await reportOldAccountMismatch?.();
    });

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(screen.getByTestId("session").textContent).toBe(newSession.user.id);
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
      expect.any(Function),
    );
  });

  it("does not let failed initial recovery erase a newer session", async () => {
    const fatalError = new Error("invalid refresh token");
    const initialSession = deferred<{
      data: { session: null };
      error: Error;
    }>();
    const newSession = makeSession("new-account");
    mocks.getSession.mockReturnValue(initialSession.promise);

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

    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();

    act(() => {
      mocks.authCallback?.("SIGNED_IN", newSession);
    });

    await waitFor(() => {
      expect(screen.getByTestId("session").textContent).toBe(
        newSession.user.id,
      );
    });

    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).not.toHaveBeenCalledWith(
      "SIGNED_OUT",
      null,
    );
  });

  it("restores stored auth when initial token refresh fails", async () => {
    const fatalError = new Error("invalid refresh token");
    const initialSession = deferred<{
      data: { session: null };
      error: Error;
    }>();
    const storedSession = makeSession("stored-account");
    mocks.getSession.mockReturnValue(initialSession.promise);
    mocks.readPersistedAuthSession.mockResolvedValue(storedSession);

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
      expect(screen.getByTestId("session").textContent).toBe(
        storedSession.user.id,
      );
    });

    expect(mocks.clearAuthStorage).not.toHaveBeenCalled();
    expect(mocks.persistAuthSession).not.toHaveBeenCalled();
    expect(mocks.handleCloudsyncAuthChange).toHaveBeenCalledWith(
      "INITIAL_SESSION",
      storedSession,
      expect.any(Function),
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

  it("keeps the account signed in when CloudSync suspension fails", async () => {
    const localSession = makeSession("bound-account");
    mocks.prepareCloudsyncSignOut.mockRejectedValue(
      new Error("cloudsync suspension failed"),
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
      expect(mocks.prepareCloudsyncSignOut).toHaveBeenCalledWith(
        localSession,
        expect.any(Function),
      );
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
