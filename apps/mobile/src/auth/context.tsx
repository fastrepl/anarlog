import AsyncStorage from "@react-native-async-storage/async-storage";
import {
  AuthApiError,
  AuthSessionMissingError,
  type Session,
  type SupabaseClient,
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
import { refreshBillingEntitlement as retryBillingEntitlement } from "@/auth/billing-handoff";
import { authStorageKey, supabase } from "@/auth/client";
import { buildSignInUrl, type SignInMethod } from "@/auth/sign-in";
import {
  captureAnalytics,
  identifyAnalytics,
  resetAnalytics,
} from "@/lib/analytics";
import { env, hasSupabaseEnv } from "@/lib/env";
import {
  addOperationalBreadcrumb,
  captureOperationalError,
  setErrorReportingUser,
} from "@/lib/error-reporting";
import { suspendMobileSync } from "@/sync/mobile-sync";
import { updateWatchAccount } from "@/watch-connectivity";

export type AuthState = {
  status: "loading" | "signed_out" | "signed_in";
  bypass: boolean;
  session: Session | null;
  billing: BillingInfo;
  billingReady: boolean;
  signIn: (method: SignInMethod) => Promise<void>;
  refreshBilling: () => Promise<boolean>;
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
          captureOperationalError(error, {
            operation: "auth_session_set",
            tags: { method: "browser_handoff" },
          });
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
        captureOperationalError(error, {
          operation: "auth_session_set",
          tags: { method: "browser_handoff" },
        });
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
  } catch (error) {
    captureOperationalError(error, {
      operation: "auth_persisted_session_read",
      level: "warning",
    });
    return null;
  }
}

async function clearInvalidSession(
  client: SupabaseClient,
  reason: "fatal_error" | "rejected",
) {
  const { error } = await client.auth
    .signOut({ scope: "local" })
    .catch((error: unknown) => ({ error }));
  if (error) {
    captureOperationalError(error, {
      operation: "auth_invalid_session_clear",
      level: "warning",
      tags: { reason },
    });
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const bypass = !hasSupabaseEnv;
  const [session, setSessionState] = useState<Session | null | undefined>(
    bypass ? null : undefined,
  );
  const sessionRef = useRef<Session | null | undefined>(session);
  const identifiedUserIdRef = useRef<string | null>(null);
  const reportedUndecodableUserIdRef = useRef<string | null>(null);
  const billingRefreshRef = useRef<Promise<boolean> | null>(null);
  // Prevents double init in React StrictMode (refresh token races)
  const initStartedRef = useRef(false);

  const setSession = useCallback((next: Session | null) => {
    sessionRef.current = next;
    setSessionState(next);
    const userId = next?.user.id ?? null;
    setErrorReportingUser(userId);
    updateWatchAccount(
      next
        ? {
            userId: next.user.id,
            email: next.user.email ?? null,
          }
        : null,
    );
    if (
      next &&
      !decodeJwtPayload(next.access_token) &&
      reportedUndecodableUserIdRef.current !== userId
    ) {
      reportedUndecodableUserIdRef.current = userId;
      captureOperationalError(new Error("Mobile auth token decode failed"), {
        operation: "auth_token_decode",
        level: "warning",
      });
    } else if (!next) {
      reportedUndecodableUserIdRef.current = null;
    }
    if (userId && userId !== identifiedUserIdRef.current) {
      identifiedUserIdRef.current = userId;
      identifyAnalytics(userId);
    } else if (!userId) {
      identifiedUserIdRef.current = null;
    }
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
              await clearInvalidSession(client, "fatal_error");
              setSession(null);
              return;
            }
            addOperationalBreadcrumb("auth_session_restore", {
              outcome: "persisted_fallback",
              reason: "get_session_error",
            });
            setSession(await readPersistedSession());
            return;
          }
          setSession(data.session ?? null);
        },
        async (error) => {
          if (isFatalSessionError(error)) {
            await clearInvalidSession(client, "rejected");
            setSession(null);
            return;
          }
          addOperationalBreadcrumb("auth_session_restore", {
            outcome: "persisted_fallback",
            reason: "get_session_rejected",
          });
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
    if (!supabase) {
      return;
    }

    const subscription = Linking.addEventListener("url", (event) => {
      handleAuthCallbackUrl(event.url);
    });
    void Linking.getInitialURL().then(
      (url) => {
        if (url) {
          handleAuthCallbackUrl(url);
        }
      },
      (error) => {
        captureOperationalError(error, {
          operation: "auth_initial_url_read",
          level: "warning",
        });
      },
    );

    return () => {
      subscription.remove();
    };
  }, []);

  const signIn = useCallback(async (signInMethod: SignInMethod) => {
    if (!supabase) {
      return;
    }

    captureAnalytics("auth_started", {
      method: "browser_handoff",
      sign_in_method: signInMethod,
      entry_point: "mobile_sign_in",
    });
    try {
      const result = await WebBrowser.openAuthSessionAsync(
        buildSignInUrl(env.appUrl, signInMethod),
        "anarlog://auth/callback",
      );
      if (result.type === "success") {
        handleAuthCallbackUrl(result.url);
      } else {
        captureAnalytics("auth_failed", {
          method: "browser_handoff",
          sign_in_method: signInMethod,
          failure_stage: "cancelled",
        });
      }
    } catch (error) {
      captureOperationalError(error, {
        operation: "auth_browser_open",
        tags: { method: "browser_handoff" },
      });
      captureAnalytics("auth_failed", {
        method: "browser_handoff",
        sign_in_method: signInMethod,
        failure_stage: "open_browser",
      });
      throw error;
    }
  }, []);

  const refreshBilling = useCallback((): Promise<boolean> => {
    const client = supabase;
    if (!client) return Promise.resolve(true);
    if (billingRefreshRef.current) return billingRefreshRef.current;

    let lastError: unknown;
    const task = retryBillingEntitlement({
      refresh: async () => {
        const { data, error } = await client.auth.refreshSession();
        if (error) {
          lastError = error;
          return false;
        }

        const nextSession = data.session;
        if (!nextSession) return false;
        setSession(nextSession);
        return deriveBillingInfo(decodeJwtPayload(nextSession.access_token))
          .isPro;
      },
    })
      .then((unlocked) => {
        if (!unlocked && lastError) {
          captureOperationalError(lastError, {
            operation: "billing_entitlement_refresh",
            level: "warning",
          });
        }
        return unlocked;
      })
      .finally(() => {
        billingRefreshRef.current = null;
      });
    billingRefreshRef.current = task;
    return task;
  }, [setSession]);

  const signOut = useCallback(async () => {
    if (!supabase) {
      return;
    }

    suspendMobileSync();
    captureAnalytics("user_signed_out");
    const { error } = await supabase.auth
      .signOut({ scope: "local" })
      .catch((error: unknown) => ({ error }));
    if (error) {
      captureOperationalError(error, {
        operation: "auth_sign_out",
        level: "warning",
      });
      // auth-js can early-return without clearing storage; force local cleanup
      // so the session cannot silently resurrect on next launch.
      supabase.auth.stopAutoRefresh();
      await AsyncStorage.removeItem(authStorageKey).catch((storageError) => {
        captureOperationalError(storageError, {
          operation: "auth_storage_clear",
          level: "warning",
        });
      });
    }
    setSession(null);
    setErrorReportingUser(null);
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
      refreshBilling,
      signOut,
    }),
    [
      status,
      bypass,
      session,
      billing,
      billingReady,
      signIn,
      refreshBilling,
      signOut,
    ],
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
