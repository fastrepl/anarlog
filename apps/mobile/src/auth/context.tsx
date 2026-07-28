import AsyncStorage from "@react-native-async-storage/async-storage";
import {
  AuthApiError,
  AuthSessionMissingError,
  type Session,
} from "@supabase/supabase-js";
import * as Linking from "expo-linking";
import * as WebBrowser from "expo-web-browser";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import {
  decodeJwtPayload,
  deriveBillingInfo,
  type BillingInfo,
} from "@/auth/billing";
import { authStorageKey, supabase } from "@/auth/client";
import {
  captureAnalytics,
  identifyAnalytics,
  resetAnalytics,
} from "@/lib/analytics";
import { env, hasSupabaseEnv } from "@/lib/env";

export type AuthState = {
  status: "loading" | "signed_out" | "signed_in";
  bypass: boolean;
  session: Session | null;
  billing: BillingInfo;
  billingReady: boolean;
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
};

const AuthContext = createContext<AuthState | null>(null);

const isFatalSessionError = (error: unknown): boolean => {
  if (error instanceof AuthSessionMissingError) {
    return true;
  }
  if (error instanceof AuthApiError) {
    return ["refresh_token_not_found", "refresh_token_already_used"].includes(
      error.code ?? "",
    );
  }
  return false;
};

function parseAuthCallbackUrl(
  url: string,
): { accessToken: string; refreshToken: string } | null {
  const queryIndex = url.indexOf("?");
  const base = queryIndex === -1 ? url : url.slice(0, queryIndex);
  if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\/auth\/callback\/?$/.test(base)) {
    return null;
  }

  const params: Record<string, string> = {};
  if (queryIndex !== -1) {
    for (const pair of url.slice(queryIndex + 1).split("&")) {
      const eq = pair.indexOf("=");
      if (eq === -1) {
        continue;
      }
      try {
        params[decodeURIComponent(pair.slice(0, eq))] = decodeURIComponent(
          pair.slice(eq + 1),
        );
      } catch {
        return null;
      }
    }
  }

  const accessToken = params["access_token"];
  const refreshToken = params["refresh_token"];
  if (!accessToken || !refreshToken) {
    return null;
  }
  return { accessToken, refreshToken };
}

// Deep links can be delivered twice (auth-session result + Linking event);
// mirror desktop's 5s dedupe window (apps/desktop/src/auth/deeplink.ts).
const RECENT_CALLBACK_WINDOW_MS = 5_000;
const inFlightTokens = new Set<string>();
const recentTokens = new Map<string, number>();

function acceptAuthTokens(accessToken: string, refreshToken: string): void {
  const client = supabase;
  if (!client) {
    return;
  }

  const now = Date.now();
  for (const [key, completedAt] of recentTokens) {
    if (now - completedAt >= RECENT_CALLBACK_WINDOW_MS) {
      recentTokens.delete(key);
    }
  }

  const key = `${accessToken}\n${refreshToken}`;
  if (inFlightTokens.has(key) || recentTokens.has(key)) {
    return;
  }

  inFlightTokens.add(key);
  void client.auth
    .setSession({ access_token: accessToken, refresh_token: refreshToken })
    .then(
      ({ error }) => {
        inFlightTokens.delete(key);
        if (error) {
          console.error("[auth] setSession failed", error);
          captureAnalytics("auth_failed", {
            method: "browser_handoff",
            failure_stage: "set_session",
          });
        } else {
          recentTokens.set(key, Date.now());
        }
      },
      (error) => {
        inFlightTokens.delete(key);
        console.error("[auth] setSession failed", error);
        captureAnalytics("auth_failed", {
          method: "browser_handoff",
          failure_stage: "set_session",
        });
      },
    );
}

function handleAuthCallbackUrl(url: string): void {
  const tokens = parseAuthCallbackUrl(url);
  if (tokens) {
    acceptAuthTokens(tokens.accessToken, tokens.refreshToken);
  }
}

// Offline fallback: a retryable getSession error must not lock a Pro user out
// of their local data — fall back to the persisted session.
async function readPersistedSession(): Promise<Session | null> {
  try {
    const raw = await AsyncStorage.getItem(authStorageKey);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Session;
    return parsed?.access_token && parsed?.refresh_token ? parsed : null;
  } catch {
    return null;
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const bypass = !hasSupabaseEnv;
  const [session, setSessionState] = useState<Session | null | undefined>(
    bypass ? null : undefined,
  );
  const sessionRef = useRef<Session | null | undefined>(session);
  // Prevents double init in React StrictMode (refresh token races)
  const initStartedRef = useRef(false);

  const setSession = useCallback((next: Session | null) => {
    sessionRef.current = next;
    setSessionState(next);
  }, []);

  useEffect(() => {
    const client = supabase;
    if (!client) {
      return;
    }

    if (!initStartedRef.current) {
      initStartedRef.current = true;
      void client.auth.getSession().then(
        async ({ data, error }) => {
          if (error) {
            if (isFatalSessionError(error)) {
              await client.auth.signOut({ scope: "local" }).catch(() => {});
              setSession(null);
              return;
            }
            setSession(await readPersistedSession());
            return;
          }
          setSession(data.session ?? null);
        },
        async (error) => {
          if (isFatalSessionError(error)) {
            await client.auth.signOut({ scope: "local" }).catch(() => {});
            setSession(null);
            return;
          }
          setSession(await readPersistedSession());
        },
      );
    }

    const {
      data: { subscription },
    } = client.auth.onAuthStateChange((event, nextSession) => {
      // INITIAL_SESSION can race the offline fallback with null; don't let it
      // clobber a session we already restored.
      if (event === "INITIAL_SESSION" && !nextSession && sessionRef.current) {
        return;
      }
      setSession(nextSession);
      if (event === "SIGNED_IN" && nextSession) {
        identifyAnalytics(nextSession.user.id);
        captureAnalytics("user_signed_in", {
          method: "browser_handoff",
        });
      } else if (event === "SIGNED_OUT") {
        resetAnalytics();
      }
    });

    return () => {
      subscription.unsubscribe();
    };
  }, [setSession]);

  useEffect(() => {
    if (session?.user.id) {
      identifyAnalytics(session.user.id);
    }
  }, [session?.user.id]);

  useEffect(() => {
    if (!supabase) {
      return;
    }

    const subscription = Linking.addEventListener("url", (event) => {
      handleAuthCallbackUrl(event.url);
    });
    void Linking.getInitialURL().then((url) => {
      if (url) {
        handleAuthCallbackUrl(url);
      }
    });

    return () => {
      subscription.remove();
    };
  }, []);

  const signIn = useCallback(async () => {
    if (!supabase) {
      return;
    }

    captureAnalytics("auth_started", {
      method: "browser_handoff",
      entry_point: "mobile_sign_in",
    });
    try {
      const result = await WebBrowser.openAuthSessionAsync(
        `${env.appUrl}/auth?flow=desktop&scheme=anarlog`,
        "anarlog://auth/callback",
      );
      if (result.type === "success") {
        handleAuthCallbackUrl(result.url);
      } else {
        captureAnalytics("auth_failed", {
          method: "browser_handoff",
          failure_stage: "cancelled",
        });
      }
    } catch (error) {
      captureAnalytics("auth_failed", {
        method: "browser_handoff",
        failure_stage: "open_browser",
      });
      throw error;
    }
  }, []);

  const signOut = useCallback(async () => {
    if (!supabase) {
      return;
    }

    captureAnalytics("user_signed_out");
    const { error } = await supabase.auth
      .signOut({ scope: "local" })
      .catch((error: unknown) => ({ error }));
    if (error) {
      // auth-js can early-return without clearing storage; force local cleanup
      // so the session cannot silently resurrect on next launch.
      supabase.auth.stopAutoRefresh();
      await AsyncStorage.removeItem(authStorageKey).catch(() => {});
    }
    setSession(null);
    resetAnalytics();
  }, [setSession]);

  const accessToken = session?.access_token ?? null;
  const payload = useMemo(
    () => (accessToken ? decodeJwtPayload(accessToken) : null),
    [accessToken],
  );
  const billing = useMemo(() => deriveBillingInfo(payload), [payload]);

  const status: AuthState["status"] = bypass
    ? "signed_in"
    : session === undefined
      ? "loading"
      : session
        ? "signed_in"
        : "signed_out";
  // `payload` is derived from the session in the same render, so it is only ever
  // null here because the token failed to decode. Gating on it would wedge the
  // app on a loading spinner with no way out, so an undecodable token resolves
  // as ready without entitlement instead.
  const billingReady = bypass || status !== "loading";

  const value = useMemo<AuthState>(
    () => ({
      status,
      bypass,
      session: session ?? null,
      billing,
      billingReady,
      signIn,
      signOut,
    }),
    [status, bypass, session, billing, billingReady, signIn, signOut],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const value = useContext(AuthContext);
  if (!value) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return value;
}
