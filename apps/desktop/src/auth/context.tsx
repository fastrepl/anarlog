import {
  type AuthChangeEvent,
  AuthRetryableFetchError,
  AuthSessionMissingError,
  type Session,
} from "@supabase/supabase-js";
import { useMutation } from "@tanstack/react-query";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { commands as miscCommands } from "@anlg/plugin-misc";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";

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
  AUTH_SIGN_OUT_REQUEST_EVENT,
  AUTH_SIGN_OUT_RESULT_EVENT,
  type AuthSignOutRequestPayload,
  type AuthSignOutResultPayload,
  getErrorMessage,
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

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null | undefined>(undefined);
  const [fingerprint, setFingerprint] = useState<string | null>(null);
  const currentWindowLabel = getCurrentWebviewWindow().label;
  const managesCloudsync = currentWindowLabel === "main";
  // Prevents double initSession in React StrictMode, which can cause refresh token races
  const initStartedRef = useRef(false);
  const authTransitionRef = useRef(0);
  const authTransitionEventRef = useRef<AuthChangeEvent | null>(null);
  const nonInitialAuthTransitionRef = useRef(0);
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

  const applyAuthChange = useCallback(
    async (
      event: AuthChangeEvent,
      nextSession: Session | null,
      transition: number,
      storageRevision: number,
      clearStorage: boolean,
    ) => {
      if (transition !== authTransitionRef.current) {
        return;
      }

      if (clearStorage || event === "SIGNED_OUT") {
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

        await rejectAuthChange(
          transition,
          clearStorage && event !== "SIGNED_OUT",
          mainSignOutCompleted,
        );
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
            await rejectAuthChange(transition, true);
            return;
          }
        } catch {
          if (transition !== authTransitionRef.current) {
            return;
          }
          console.warn("[auth] local database account verification failed");
          await rejectAuthChange(transition, true);
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

      const rejectAccountMismatch = () => rejectAuthChange(transition, true);
      const result = await handleCloudsyncAuthChange(
        event,
        nextSession,
        rejectAccountMismatch,
      );
      if (
        result !== "account_mismatch" ||
        transition !== authTransitionRef.current
      ) {
        return;
      }

      await rejectAccountMismatch();
    },
    [
      coordinateMainSignOut,
      enqueueAuthAnalytics,
      managesCloudsync,
      rejectAuthChange,
    ],
  );

  const enqueueAuthChange = useCallback(
    (
      event: AuthChangeEvent,
      nextSession: Session | null,
      clearStorage = false,
    ) => {
      if (event !== "SIGNED_OUT") {
        coordinatedMainSignOutRef.current = null;
      }
      authTransitionEventRef.current = event;
      const transition = ++authTransitionRef.current;
      const storageRevision = authStorageRevisionRef.current;
      const apply = () =>
        applyAuthChange(
          event,
          nextSession,
          transition,
          storageRevision,
          clearStorage,
        );
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
      const initialNonInitialTransition = nonInitialAuthTransitionRef.current;
      void loadInitialSession(supabase).then((initial) => {
        if (initial.clearStorage) {
          if (
            initialNonInitialTransition === nonInitialAuthTransitionRef.current
          ) {
            void enqueueAuthChange("INITIAL_SESSION", null, true);
          }
          return;
        }

        if (initialTransition === authTransitionRef.current) {
          void enqueueAuthChange("INITIAL_SESSION", initial.session);
        }
      });
    }

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((event, session) => {
      if (event !== "INITIAL_SESSION") {
        nonInitialAuthTransitionRef.current += 1;
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

                const rejectAccountMismatch = async () => {
                  if (cancelled || transition !== authTransitionRef.current) {
                    return;
                  }

                  await rejectAuthChange(transition, true);
                };
                const result = await refreshCloudsyncForSession(
                  currentSession,
                  rejectAccountMismatch,
                );
                if (
                  cancelled ||
                  result !== "account_mismatch" ||
                  transition !== authTransitionRef.current
                ) {
                  return;
                }

                await rejectAccountMismatch();
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
  }, [managesCloudsync, rejectAuthChange]);

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
    const rejectAccountMismatch = () => rejectAuthChange(transition, true);
    await prepareCloudsyncSignOut(currentSession, rejectAccountMismatch);

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
          rejectAccountMismatch,
        );
        if (result === "account_mismatch") {
          await rejectAccountMismatch();
          return true;
        }
      }
      throw signOutError;
    }

    if (!shouldCleanUp || transition !== authTransitionRef.current) {
      return false;
    }

    await enqueueAuthChange("SIGNED_OUT", null);
    return true;
  }, [enqueueAuthChange, rejectAuthChange, session]);
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
