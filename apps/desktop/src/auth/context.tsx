import {
  type AuthChangeEvent,
  AuthRetryableFetchError,
  AuthSessionMissingError,
  type Session,
  type SupabaseClient,
} from "@supabase/supabase-js";
import { useMutation } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { version as osVersion, platform } from "@tauri-apps/plugin-os";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as miscCommands } from "@hypr/plugin-misc";
import { commands as openerCommands } from "@hypr/plugin-opener2";

import { supabase } from "./client";
import { clearAuthStorage, isFatalSessionError } from "./errors";

import {
  buildWebAppUrl,
  DEVICE_FINGERPRINT_HEADER,
  REQUEST_ID_HEADER,
  id,
} from "~/shared/utils";

type AuthState = {
  supabase: SupabaseClient | null;
  session: Session | null;
  isRefreshingSession: boolean;
};

type AuthActions = {
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  refreshSession: () => Promise<Session | null>;
};

type AuthTokenHandlers = {
  handleAuthCallback: (url: string) => Promise<void>;
  setSessionFromTokens: (
    accessToken: string,
    refreshToken: string,
  ) => Promise<void>;
};

type AuthUtils = {
  getHeaders: () => Record<string, string> | null;
  getAvatarUrl: () => Promise<string | null>;
};

export type AuthContextType = AuthState &
  AuthActions &
  AuthTokenHandlers &
  AuthUtils;

const AuthContext = createContext<AuthContextType | null>(null);

export function useAuth() {
  const context = useContext(AuthContext);

  if (!context) {
    throw new Error("'useAuth' must be used within an 'AuthProvider'");
  }

  return context;
}

async function clearInvalidSession(
  _client: SupabaseClient,
  setSession: (session: Session | null) => void,
): Promise<void> {
  await clearAuthStorage();
  setSession(null);
}

async function initSession(
  client: SupabaseClient,
  setSession: (session: Session | null) => void,
): Promise<void> {
  const onClear = () => clearInvalidSession(client, setSession);

  try {
    const { data, error } = await client.auth.getSession();

    if (error) {
      if (isFatalSessionError(error)) {
        await onClear();
      }
      return;
    }

    if (data.session) {
      setSession(data.session);
    }
  } catch (e) {
    if (isFatalSessionError(e)) {
      await onClear();
    }
  }
}

let trackedUserId: string | null = null;

async function trackAuthEvent(
  event: AuthChangeEvent,
  session: Session | null,
): Promise<void> {
  if ((event === "SIGNED_IN" || event === "INITIAL_SESSION") && session) {
    if (session.user.id === trackedUserId) {
      return;
    }

    trackedUserId = session.user.id;

    const appVersion = await getVersion();
    void analyticsCommands.identify(session.user.id, {
      email: session.user.email,
      set: {
        account_created_date: session.user.created_at,
        is_signed_up: true,
        app_version: appVersion,
        os_version: osVersion(),
        platform: platform(),
      },
    });

    if (event === "SIGNED_IN") {
      void analyticsCommands.event({ event: "user_signed_in" });
    }
  }

  if (event === "SIGNED_OUT") {
    trackedUserId = null;
  }
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [fingerprint, setFingerprint] = useState<string | null>(null);
  // Prevents double initSession in React StrictMode, which can cause refresh token races
  const initStartedRef = useRef(false);

  useEffect(() => {
    miscCommands.getFingerprint().then((result) => {
      if (result.status === "ok") {
        setFingerprint(result.data);
      }
    });
  }, []);

  const setSessionFromTokens = useCallback(
    async (accessToken: string, refreshToken: string) => {
      if (!supabase) {
        console.error("Supabase client not found");
        return;
      }

      const res = await supabase.auth.setSession({
        access_token: accessToken,
        refresh_token: refreshToken,
      });

      if (res.error) {
        console.error(res.error);
      } else {
        setSession(res.data.session);
      }
    },
    [],
  );

  const handleAuthCallback = useCallback(
    async (url: string) => {
      const parsed = new URL(url);
      const accessToken = parsed.searchParams.get("access_token");
      const refreshToken = parsed.searchParams.get("refresh_token");

      if (!accessToken || !refreshToken) {
        console.error("invalid_callback_url");
        return;
      }

      await setSessionFromTokens(accessToken, refreshToken);
    },
    [setSessionFromTokens],
  );

  useEffect(() => {
    if (!supabase) {
      return;
    }

    if (!initStartedRef.current) {
      initStartedRef.current = true;
      void initSession(supabase, setSession);
    }

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((event, session) => {
      console.log(
        `[auth] onAuthStateChange: ${event}`,
        session ? `expires_at=${session.expires_at}` : "no session",
      );
      void trackAuthEvent(event, session);
      setSession(session);
    });

    return () => {
      subscription.unsubscribe();
    };
  }, []);

  // Tauri's visibilitychange event is broken (always reports "visible" on Windows,
  // only fires on minimize/maximize on macOS — not when hidden behind other windows).
  // The Supabase SDK relies on visibilitychange to start/stop its auto-refresh ticker,
  // which can cause sessions to expire during inactivity when the window is hidden.
  // We bypass this by running the ticker continuously and using Tauri's native
  // onFocusChanged for immediate recovery after sleep/hibernate.
  // See: https://supabase.com/docs/guides/auth/sessions
  // See: https://github.com/tauri-apps/tauri/issues/10592
  useEffect(() => {
    if (!supabase) {
      return;
    }

    const client = supabase;

    // startAutoRefresh() removes the SDK's visibilitychange listener and
    // runs the refresh ticker continuously (checks storage every 30s,
    // only makes a network call when the token is near expiry).
    console.log("[auth] startAutoRefresh: mounting continuous ticker");
    void client.auth.startAutoRefresh();

    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        console.log(`[auth] onFocusChanged: focused=${focused}`);
        if (focused) {
          // Restart the ticker on window focus to trigger an immediate refresh
          // check, recovering stale sessions after sleep/hibernate.
          console.log("[auth] startAutoRefresh: window regained focus");
          void client.auth.startAutoRefresh();
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      });

    return () => {
      console.log("[auth] stopAutoRefresh: unmounting");
      cancelled = true;
      unlisten?.();
      void client.auth.stopAutoRefresh();
    };
  }, []);

  const signIn = useCallback(async () => {
    const url = await buildWebAppUrl("/auth");
    await openerCommands.openUrl(url, null);
  }, []);

  const signOut = useCallback(async () => {
    if (!supabase) {
      return;
    }

    try {
      const { error } = await supabase.auth.signOut({ scope: "local" });
      if (error) {
        if (
          error instanceof AuthRetryableFetchError ||
          error instanceof AuthSessionMissingError
        ) {
          trackedUserId = null;
          await clearAuthStorage();
          setSession(null);
          return;
        }
        console.error(error);
        return;
      }

      trackedUserId = null;
      await clearAuthStorage();
      setSession(null);
    } catch (e) {
      if (
        e instanceof AuthRetryableFetchError ||
        e instanceof AuthSessionMissingError
      ) {
        trackedUserId = null;
        await clearAuthStorage();
        setSession(null);
      }
    }
  }, []);

  const refreshSessionMutation = useMutation({
    mutationFn: async (): Promise<Session | null> => {
      if (!supabase) {
        return null;
      }

      const { data, error } = await supabase.auth.refreshSession();
      if (error) {
        return null;
      }
      if (data.session) {
        setSession(data.session);
        return data.session;
      }
      return null;
    },
  });

  const refreshSession = useCallback(
    () => refreshSessionMutation.mutateAsync(),
    [refreshSessionMutation.mutateAsync],
  );

  const getHeaders = useCallback(() => {
    if (!session) {
      return null;
    }

    const headers: Record<string, string> = {
      Authorization: `${session.token_type} ${session.access_token}`,
      [REQUEST_ID_HEADER]: id(),
    };

    if (fingerprint) {
      headers[DEVICE_FINGERPRINT_HEADER] = fingerprint;
    }

    return headers;
  }, [session, fingerprint]);

  const getAvatarUrl = useCallback(async () => {
    const email = session?.user.email;

    if (!email) {
      return null;
    }

    const address = email.trim().toLowerCase();
    const encoder = new TextEncoder();
    const data = encoder.encode(address);
    const hashBuffer = await crypto.subtle.digest("SHA-256", data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    const hash = hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");

    return `https://gravatar.com/avatar/${hash}?d=404`;
  }, [session]);

  const value = useMemo(
    () => ({
      session,
      supabase,
      signIn,
      signOut,
      refreshSession,
      isRefreshingSession: refreshSessionMutation.isPending,
      handleAuthCallback,
      setSessionFromTokens,
      getHeaders,
      getAvatarUrl,
    }),
    [
      session,
      signIn,
      signOut,
      refreshSession,
      refreshSessionMutation.isPending,
      handleAuthCallback,
      setSessionFromTokens,
      getHeaders,
      getAvatarUrl,
    ],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
