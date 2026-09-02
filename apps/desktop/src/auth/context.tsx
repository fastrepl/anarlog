import { t } from "@lingui/core/macro";
import {
  type AuthChangeEvent,
  AuthRetryableFetchError,
  AuthSessionMissingError,
  type Session,
} from "@supabase/supabase-js";
import { useMutation } from "@tanstack/react-query";
import { emit, emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { commands as miscCommands } from "@anlg/plugin-misc";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import { getProviderProfileImageUrl } from "@anlg/supabase/profile";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  clearAuthAnalyticsGroups,
  resetTrackedAuthIdentity,
  trackAuthEvent,
} from "./auth-analytics";
import { AuthContext } from "./auth-context";
import { persistAuthSession, supabase } from "./client";
import {
  bindCloudsyncAccountForAuth,
  handleCloudsyncAuthChange,
  prepareCloudsyncSignOut,
  refreshCloudsyncForSession,
} from "./cloudsync";
import { clearAuthStorage } from "./errors";
import { loadInitialSession } from "./initial-session";
import {
  AUTH_SIGN_OUT_COMMITTED_EVENT,
  AUTH_SIGN_OUT_REQUEST_EVENT,
  AUTH_SIGN_OUT_RESULT_EVENT,
  type AuthSignOutCommittedPayload,
  type AuthSignOutRequestPayload,
  type AuthSignOutResultPayload,
  getErrorMessage,
  isAuthSignOutCommittedPayload,
  isAuthSignOutRequestPayload,
  requestMainSignOut,
} from "./sign-out-coordination";

import { trackAnalyticsEvent } from "~/analytics";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  buildWebAppUrl,
  DEVICE_FINGERPRINT_HEADER,
  REQUEST_ID_HEADER,
  id,
} from "~/shared/utils";

const ACCOUNT_MISMATCH_TOAST_ID = "auth-account-mismatch";

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null | undefined>(undefined);
  const [fingerprint, setFingerprint] = useState<string | null>(null);
  const currentWindowLabel = getCurrentWebviewWindow().label;
  const managesCloudsync = currentWindowLabel === "main";
  // Prevents double initSession in React StrictMode, which can cause refresh token races
  const initStartedRef = useRef(false);
  const authTransitionRef = useRef(0);
  const authTransitionEventRef = useRef<AuthChangeEvent | null>(null);
  const authTransitionQueueRef = useRef(Promise.resolve());
  const authAnalyticsQueueRef = useRef(Promise.resolve());
  const authStorageRevisionRef = useRef(0);
  const coordinatedMainSignOutRef = useRef<Promise<boolean> | null>(null);

  const enqueueAuthAnalytics = useCallback((task: () => Promise<void>) => {
    const queued = authAnalyticsQueueRef.current.then(task, task);
    authAnalyticsQueueRef.current = queued.catch(() => {});
    return queued;
  }, []);

  const coordinateMainSignOut = useCallback(() => {
    const existing = coordinatedMainSignOutRef.current;
    if (existing) {
      return existing;
    }

    const request = requestMainSignOut(currentWindowLabel);
    coordinatedMainSignOutRef.current = request;
    void request.then(
      (completed) => {
        if (!completed && coordinatedMainSignOutRef.current === request) {
          coordinatedMainSignOutRef.current = null;
        }
      },
      () => {
        if (coordinatedMainSignOutRef.current === request) {
          coordinatedMainSignOutRef.current = null;
        }
      },
    );
    return request;
  }, [currentWindowLabel]);

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

  const rejectAuthChange = useCallback(
    async (
      transition: number,
      invalidateClientSession = false,
      mainSignOutCompleted = false,
    ) => {
      if (transition !== authTransitionRef.current) {
        return;
      }

      if (
        invalidateClientSession &&
        !managesCloudsync &&
        !mainSignOutCompleted
      ) {
        let completed: boolean;
        try {
          completed = await coordinateMainSignOut();
        } catch {
          console.warn("[auth] rejected session could not be routed to main");
          return;
        }

        if (!completed || transition !== authTransitionRef.current) {
          return;
        }
      }

      if (invalidateClientSession && supabase) {
        try {
          await supabase.auth.stopAutoRefresh();
        } catch {
          console.warn("[auth] session refresh could not be stopped");
        }

        if (transition !== authTransitionRef.current) {
          return;
        }

        try {
          const { error } = await supabase.auth.signOut({ scope: "local" });
          if (error) {
            console.warn("[auth] rejected session could not be invalidated");
          }
        } catch {
          console.warn("[auth] rejected session could not be invalidated");
        }

        if (transition !== authTransitionRef.current) {
          return;
        }
      }

      await clearAuthStorage();
      authStorageRevisionRef.current += 1;

      if (transition !== authTransitionRef.current) {
        return;
      }

      resetTrackedAuthIdentity();
      await enqueueAuthAnalytics(clearAuthAnalyticsGroups);
      setSession(null);
      if (managesCloudsync) {
        await handleCloudsyncAuthChange("SIGNED_OUT", null);
      }
    },
    [coordinateMainSignOut, enqueueAuthAnalytics, managesCloudsync],
  );

  const rejectAccountMismatch = useCallback(
    async (transition: number) => {
      if (transition !== authTransitionRef.current) {
        return;
      }

      sonnerToast.error(
        t`The notes on this device are linked to another Anarlog account. Sign in with the account previously used here.`,
        { id: ACCOUNT_MISMATCH_TOAST_ID },
      );
      await rejectAuthChange(transition, true);

      if (managesCloudsync && transition === authTransitionRef.current) {
        try {
          await emit(AUTH_SIGN_OUT_COMMITTED_EVENT, {
            sourceLabel: currentWindowLabel,
          } satisfies AuthSignOutCommittedPayload);
        } catch {
          console.warn("[auth] account rejection could not be synchronized");
        }
      }
    },
    [currentWindowLabel, managesCloudsync, rejectAuthChange],
  );

  const applyAuthChange = useCallback(
    async (
      event: AuthChangeEvent,
      nextSession: Session | null,
      transition: number,
      storageRevision: number,
    ) => {
      if (transition !== authTransitionRef.current) {
        return;
      }

      if (event === "SIGNED_OUT") {
        let mainSignOutCompleted = false;
        if (event === "SIGNED_OUT" && !managesCloudsync) {
          resetTrackedAuthIdentity();
          setSession(null);

          try {
            mainSignOutCompleted = await coordinateMainSignOut();
          } catch {
            console.warn("[auth] sign-out could not be routed to main");
            return;
          }

          if (
            !mainSignOutCompleted ||
            transition !== authTransitionRef.current
          ) {
            return;
          }
        }

        await rejectAuthChange(transition, false, mainSignOutCompleted);
        return;
      }

      if (transition !== authTransitionRef.current) {
        return;
      }

      if (nextSession) {
        try {
          const claimed = await bindCloudsyncAccountForAuth(
            nextSession.user.id,
          );
          if (transition !== authTransitionRef.current) {
            return;
          }
          if (!claimed) {
            console.warn("[auth] local database belongs to another account");
            await rejectAccountMismatch(transition);
            return;
          }
          sonnerToast.dismiss(ACCOUNT_MISMATCH_TOAST_ID);
        } catch {
          if (transition !== authTransitionRef.current) {
            return;
          }
          console.warn(
            "[auth] local database account verification failed; preserving the local session",
          );
          setSession(nextSession);
          void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));
          return;
        }
      }

      if (nextSession && storageRevision !== authStorageRevisionRef.current) {
        try {
          await persistAuthSession(nextSession);
        } catch {
          console.warn("[auth] accepted session could not be restored");
        }

        if (transition !== authTransitionRef.current) {
          return;
        }
      }

      if (nextSession && supabase) {
        try {
          await supabase.auth.startAutoRefresh();
        } catch {
          console.warn("[auth] session refresh could not be started");
        }

        if (transition !== authTransitionRef.current) {
          return;
        }
      }

      setSession(nextSession);
      void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));

      if (!managesCloudsync) {
        return;
      }

      const rejectCurrentAccountMismatch = () =>
        rejectAccountMismatch(transition);
      const result = await handleCloudsyncAuthChange(
        event,
        nextSession,
        rejectCurrentAccountMismatch,
      );
      if (
        result !== "account_mismatch" ||
        transition !== authTransitionRef.current
      ) {
        return;
      }

      await rejectCurrentAccountMismatch();
    },
    [
      coordinateMainSignOut,
      enqueueAuthAnalytics,
      managesCloudsync,
      rejectAccountMismatch,
      rejectAuthChange,
    ],
  );

  const enqueueAuthChange = useCallback(
    (event: AuthChangeEvent, nextSession: Session | null) => {
      if (event !== "SIGNED_OUT") {
        coordinatedMainSignOutRef.current = null;
      }
      authTransitionEventRef.current = event;
      const transition = ++authTransitionRef.current;
      const storageRevision = authStorageRevisionRef.current;
      const apply = () =>
        applyAuthChange(event, nextSession, transition, storageRevision);
      const queued =
        event === "SIGNED_OUT"
          ? Promise.resolve().then(apply)
          : authTransitionQueueRef.current.then(apply, apply);
      authTransitionQueueRef.current = queued.catch(() => {});
      return queued;
    },
    [applyAuthChange],
  );

  useEffect(() => {
    if (!supabase) {
      return;
    }

    if (!initStartedRef.current) {
      initStartedRef.current = true;
      const initialTransition = authTransitionRef.current;
      void loadInitialSession(supabase).then((initialSession) => {
        if (initialTransition === authTransitionRef.current) {
          void enqueueAuthChange("INITIAL_SESSION", initialSession);
        }
      });
    }

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((event, session) => {
      if (event === "INITIAL_SESSION") {
        return;
      }
      if (event === "SIGNED_OUT") {
        console.warn("[auth] ignoring unsolicited SDK sign-out");
        return;
      }

      console.log(
        `[auth] onAuthStateChange: ${event}`,
        session ? `expires_at=${session.expires_at}` : "no session",
      );
      void enqueueAuthChange(event, session);
    });

    return () => {
      subscription.unsubscribe();
    };
  }, [enqueueAuthChange]);

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
          if (managesCloudsync) {
            void (async () => {
              const transition = authTransitionRef.current;
              try {
                const { data, error } = await client.auth.getSession();
                if (
                  cancelled ||
                  error ||
                  !data.session ||
                  transition !== authTransitionRef.current
                ) {
                  return;
                }

                const currentSession = data.session;
                if (
                  !currentSession.expires_at ||
                  currentSession.expires_at * 1000 <= Date.now() + 120_000
                ) {
                  const refreshed = await client.auth.refreshSession();
                  if (cancelled || refreshed.error || !refreshed.data.session) {
                    return;
                  }
                  return;
                }

                if (cancelled || transition !== authTransitionRef.current) {
                  return;
                }

                const rejectCurrentAccountMismatch = async () => {
                  if (cancelled || transition !== authTransitionRef.current) {
                    return;
                  }

                  await rejectAccountMismatch(transition);
                };
                const result = await refreshCloudsyncForSession(
                  currentSession,
                  rejectCurrentAccountMismatch,
                );
                if (
                  cancelled ||
                  result !== "account_mismatch" ||
                  transition !== authTransitionRef.current
                ) {
                  return;
                }

                await rejectCurrentAccountMismatch();
              } catch {
                console.warn("[cloudsync] session recovery failed");
              }
            })();
          }
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
  }, [managesCloudsync, rejectAccountMismatch]);

  const signIn = useCallback(async () => {
    trackAnalyticsEvent("auth_started", {
      entry_point: "desktop_sign_in",
      method: "browser_handoff",
    });
    try {
      const url = await buildWebAppUrl("/auth");
      await openUrlWithInstruction(url, "sign-in", (u) =>
        openerCommands.openUrl(u, null),
      );
    } catch (error) {
      trackAnalyticsEvent("auth_failed", {
        entry_point: "desktop_sign_in",
        method: "browser_handoff",
        failure_stage: "open_browser",
      });
      throw error;
    }
  }, []);

  const signOutFromMain = useCallback(async (): Promise<boolean> => {
    if (!supabase) {
      return false;
    }

    const transition = authTransitionRef.current;
    const currentSession = session;
    const rejectCurrentAccountMismatch = () =>
      rejectAccountMismatch(transition);
    await prepareCloudsyncSignOut(currentSession, rejectCurrentAccountMismatch);

    if (transition !== authTransitionRef.current) {
      return authTransitionEventRef.current === "SIGNED_OUT";
    }

    let shouldCleanUp = false;
    let signOutError: unknown = null;

    try {
      const { error } = await supabase.auth.signOut({ scope: "local" });
      if (transition !== authTransitionRef.current) {
        return authTransitionEventRef.current === "SIGNED_OUT";
      }

      if (error) {
        if (
          error instanceof AuthRetryableFetchError ||
          error instanceof AuthSessionMissingError
        ) {
          shouldCleanUp = true;
        } else {
          signOutError = error;
        }
      } else {
        shouldCleanUp = true;
      }
    } catch (e) {
      if (transition !== authTransitionRef.current) {
        return authTransitionEventRef.current === "SIGNED_OUT";
      }

      if (
        e instanceof AuthRetryableFetchError ||
        e instanceof AuthSessionMissingError
      ) {
        shouldCleanUp = true;
      } else {
        signOutError = e;
      }
    }

    if (signOutError) {
      if (currentSession) {
        const result = await handleCloudsyncAuthChange(
          "TOKEN_REFRESHED",
          currentSession,
          rejectCurrentAccountMismatch,
        );
        if (result === "account_mismatch") {
          await rejectCurrentAccountMismatch();
          return true;
        }
      }
      throw signOutError;
    }

    if (!shouldCleanUp || transition !== authTransitionRef.current) {
      return false;
    }

    await enqueueAuthChange("SIGNED_OUT", null);
    if (authTransitionEventRef.current !== "SIGNED_OUT") {
      return false;
    }

    try {
      await emit(AUTH_SIGN_OUT_COMMITTED_EVENT, {
        sourceLabel: currentWindowLabel,
      } satisfies AuthSignOutCommittedPayload);
    } catch {
      console.warn("[auth] sign-out could not be synchronized");
    }
    return true;
  }, [currentWindowLabel, enqueueAuthChange, rejectAccountMismatch, session]);
  const signOutFromMainRef = useLatestRef(signOutFromMain);

  useMountEffect(() => {
    if (!managesCloudsync) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | null = null;

    void listen<AuthSignOutRequestPayload>(
      AUTH_SIGN_OUT_REQUEST_EVENT,
      (event) => {
        if (!active || !isAuthSignOutRequestPayload(event.payload)) {
          return;
        }

        const request = event.payload;
        void signOutFromMainRef
          .current()
          .then(
            (completed) =>
              emitTo(request.sourceLabel, AUTH_SIGN_OUT_RESULT_EVENT, {
                requestId: request.requestId,
                completed,
                error: null,
              } satisfies AuthSignOutResultPayload),
            (error) =>
              emitTo(request.sourceLabel, AUTH_SIGN_OUT_RESULT_EVENT, {
                requestId: request.requestId,
                completed: false,
                error: getErrorMessage(error),
              } satisfies AuthSignOutResultPayload),
          )
          .catch(() => {
            console.warn("[auth] sign-out acknowledgement failed");
          });
      },
    )
      .then((fn) => {
        if (active) {
          unlisten = fn;
        } else {
          fn();
        }
      })
      .catch(() => {
        console.warn("[auth] main-window sign-out bridge failed to initialize");
      });

    return () => {
      active = false;
      unlisten?.();
    };
  });

  useMountEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;

    void listen<AuthSignOutCommittedPayload>(
      AUTH_SIGN_OUT_COMMITTED_EVENT,
      (event) => {
        if (
          !active ||
          !isAuthSignOutCommittedPayload(event.payload) ||
          event.payload.sourceLabel === currentWindowLabel
        ) {
          return;
        }

        authTransitionEventRef.current = "SIGNED_OUT";
        const transition = ++authTransitionRef.current;
        void rejectAuthChange(transition, true, true);
      },
    )
      .then((fn) => {
        if (active) {
          unlisten = fn;
        } else {
          fn();
        }
      })
      .catch(() => {
        console.warn("[auth] sign-out synchronization failed to initialize");
      });

    return () => {
      active = false;
      unlisten?.();
    };
  });

  const signOut = useCallback(async () => {
    if (managesCloudsync) {
      await signOutFromMain();
      return;
    }

    const transition = authTransitionRef.current;
    const completed = await coordinateMainSignOut();
    if (!completed || transition !== authTransitionRef.current) {
      return;
    }
    await rejectAuthChange(transition, true, true);
  }, [
    coordinateMainSignOut,
    managesCloudsync,
    rejectAuthChange,
    signOutFromMain,
  ]);

  const refreshSessionMutation = useMutation({
    mutationFn: async (): Promise<Session | null> => {
      if (!supabase) {
        return null;
      }

      const { data, error } = await supabase.auth.refreshSession();
      if (error) {
        return null;
      }
      return data.session;
    },
  });

  const refreshSession = useCallback(
    () => refreshSessionMutation.mutateAsync(),
    [refreshSessionMutation.mutateAsync],
  );

  const getSessionForRequest =
    useCallback(async (): Promise<Session | null> => {
      if (!supabase) {
        return null;
      }

      let requestSession = session ?? null;
      try {
        const { data, error } = await supabase.auth.getSession();
        if (!error && data.session) {
          requestSession = data.session;
        }
      } catch {
        // Fall back to the in-memory session below.
      }

      if (!requestSession) {
        return null;
      }

      const expiresAt = requestSession.expires_at
        ? requestSession.expires_at * 1000
        : null;
      if (!expiresAt || expiresAt > Date.now() + 120_000) {
        return requestSession;
      }

      let refreshedSession: Session | null = null;
      try {
        refreshedSession = await refreshSession();
      } catch {
        // Fall back to the current session while it remains valid.
      }
      if (refreshedSession) {
        return refreshedSession;
      }

      return expiresAt > Date.now() ? requestSession : null;
    }, [refreshSession, session]);

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
    const providerImageUrl = getProviderProfileImageUrl(session?.user);
    if (providerImageUrl) {
      return providerImageUrl;
    }

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
      getSessionForRequest,
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
      getSessionForRequest,
      refreshSessionMutation.isPending,
      handleAuthCallback,
      setSessionFromTokens,
      getHeaders,
      getAvatarUrl,
    ],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
