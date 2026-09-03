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
  type CloudsyncAccountAdmission,
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
// Account admission runs behind the CloudSync plugin queue. If that queue is
// jammed, auth must not stay hidden forever; admission finishes late instead.
const ACCOUNT_ADMISSION_TIMEOUT_MS = 15_000;
// A CloudSync refresh (e.g. on window focus) can preempt the bind before it
// runs. Retry a few times; each attempt itself supersedes that refresh.
const ACCOUNT_ADMISSION_MAX_ATTEMPTS = 3;

async function settleWithin<T>(
  operation: Promise<T>,
  timeoutMs: number,
): Promise<{ status: "settled"; value: T } | { status: "timed_out" }> {
  let timeout: ReturnType<typeof setTimeout> | null = null;
  try {
    return await Promise.race([
      operation.then((value) => ({ status: "settled" as const, value })),
      new Promise<{ status: "timed_out" }>((resolve) => {
        timeout = setTimeout(() => resolve({ status: "timed_out" }), timeoutMs);
      }),
    ]);
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
  }
}

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
  // The committed session, readable synchronously so request headers never
  // lag behind React state. Admission is the account id whose local database
  // binding was verified; only admitted accounts get fast-path token refreshes.
  const sessionRef = useRef<Session | null>(null);
  const admittedUserIdRef = useRef<string | null>(null);

  const commitSession = useCallback(
    (nextSession: Session | null, admitted = false) => {
      sessionRef.current = nextSession;
      admittedUserIdRef.current =
        nextSession &&
        (admitted || admittedUserIdRef.current === nextSession.user.id)
          ? nextSession.user.id
          : null;
      setSession(nextSession);
    },
    [],
  );

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

      // Cleanup is about to clear storage and sign the account out. Any token
      // that arrives meanwhile must re-run admission instead of fast-pathing
      // past this rejection.
      admittedUserIdRef.current = null;

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
      commitSession(null);
      if (managesCloudsync) {
        await handleCloudsyncAuthChange("SIGNED_OUT", null);
      }
    },
    [
      commitSession,
      coordinateMainSignOut,
      enqueueAuthAnalytics,
      managesCloudsync,
    ],
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

  const applyCloudsyncAuthChange = useCallback(
    async (
      event: AuthChangeEvent,
      nextSession: Session | null,
      transition: number,
    ) => {
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
    [managesCloudsync, rejectAccountMismatch],
  );

  // Runs once an account is admitted: write the session back if storage was
  // cleared since the event was enqueued, and make sure the SDK refresh ticker
  // is running (a preceding sign-out or rejection may have stopped it).
  const restoreAdmittedSession = useCallback(
    async (
      nextSession: Session,
      transition: number,
      storageRevision: number,
    ) => {
      if (storageRevision !== authStorageRevisionRef.current) {
        try {
          await persistAuthSession(nextSession);
        } catch {
          console.warn("[auth] accepted session could not be restored");
        }

        if (transition !== authTransitionRef.current) {
          return false;
        }
      }

      if (supabase) {
        try {
          await supabase.auth.startAutoRefresh();
        } catch {
          console.warn("[auth] session refresh could not be started");
        }

        if (transition !== authTransitionRef.current) {
          return false;
        }
      }

      return true;
    },
    [],
  );

  const admitAccount = useCallback(
    async (
      accountUserId: string,
      transition: number,
    ): Promise<CloudsyncAccountAdmission> => {
      let admission: CloudsyncAccountAdmission = "superseded";
      for (
        let attempt = 0;
        attempt < ACCOUNT_ADMISSION_MAX_ATTEMPTS &&
        transition === authTransitionRef.current;
        attempt += 1
      ) {
        admission = await bindCloudsyncAccountForAuth(accountUserId);
        if (admission !== "superseded") {
          break;
        }
      }
      return admission;
    },
    [],
  );

  const finishLateAdmission = useCallback(
    (
      event: AuthChangeEvent,
      nextSession: Session,
      transition: number,
      storageRevision: number,
      admission: Promise<CloudsyncAccountAdmission>,
    ) => {
      void admission
        .then(
          async (outcome) => {
            if (transition !== authTransitionRef.current) {
              return;
            }
            if (outcome === "mismatch") {
              console.warn("[auth] local database belongs to another account");
              await rejectAccountMismatch(transition);
              return;
            }
            if (outcome === "superseded") {
              console.warn(
                "[auth] local database account verification was superseded; keeping the local session unadmitted",
              );
              return;
            }
            sonnerToast.dismiss(ACCOUNT_MISMATCH_TOAST_ID);
            if (
              !(await restoreAdmittedSession(
                nextSession,
                transition,
                storageRevision,
              ))
            ) {
              return;
            }
            commitSession(nextSession, true);
            await applyCloudsyncAuthChange(event, nextSession, transition);
          },
          () => {},
        )
        .catch(() => {
          console.warn("[cloudsync] late admission could not start sync");
        });
    },
    [
      applyCloudsyncAuthChange,
      commitSession,
      rejectAccountMismatch,
      restoreAdmittedSession,
    ],
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
          commitSession(null);

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
        const admission = admitAccount(nextSession.user.id, transition);
        let outcome: CloudsyncAccountAdmission;
        try {
          const settled = await settleWithin(
            admission,
            ACCOUNT_ADMISSION_TIMEOUT_MS,
          );
          if (transition !== authTransitionRef.current) {
            return;
          }
          if (settled.status === "timed_out") {
            console.warn(
              "[auth] local database account verification is stalled; preserving the local session",
            );
            commitSession(nextSession);
            void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));
            void supabase?.auth.startAutoRefresh().catch(() => {
              console.warn("[auth] session refresh could not be started");
            });
            finishLateAdmission(
              event,
              nextSession,
              transition,
              storageRevision,
              admission,
            );
            return;
          }
          outcome = settled.value;
        } catch {
          if (transition !== authTransitionRef.current) {
            return;
          }
          console.warn(
            "[auth] local database account verification failed; preserving the local session",
          );
          commitSession(nextSession);
          void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));
          return;
        }
        if (outcome === "mismatch") {
          console.warn("[auth] local database belongs to another account");
          await rejectAccountMismatch(transition);
          return;
        }
        if (outcome === "superseded") {
          console.warn(
            "[auth] local database account verification was superseded; preserving the local session",
          );
          commitSession(nextSession);
          void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));
          return;
        }
        sonnerToast.dismiss(ACCOUNT_MISMATCH_TOAST_ID);
        if (
          !(await restoreAdmittedSession(
            nextSession,
            transition,
            storageRevision,
          ))
        ) {
          return;
        }
      }

      commitSession(nextSession, true);
      void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));
      await applyCloudsyncAuthChange(event, nextSession, transition);
    },
    [
      admitAccount,
      applyCloudsyncAuthChange,
      commitSession,
      coordinateMainSignOut,
      enqueueAuthAnalytics,
      finishLateAdmission,
      managesCloudsync,
      rejectAccountMismatch,
      rejectAuthChange,
      restoreAdmittedSession,
    ],
  );

  const beginAuthTransition = useCallback((event: AuthChangeEvent) => {
    if (event !== "SIGNED_OUT") {
      coordinatedMainSignOutRef.current = null;
    }
    authTransitionEventRef.current = event;
    return ++authTransitionRef.current;
  }, []);

  const enqueueAuthChange = useCallback(
    (event: AuthChangeEvent, nextSession: Session | null) => {
      const transition = beginAuthTransition(event);
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
    [applyAuthChange, beginAuthTransition],
  );

  // A new token for the already-admitted account is a credential update, not
  // an identity change. It must never wait on the transition queue, account
  // admission, or CloudSync: a stalled handshake would otherwise leave every
  // hosted request signing with an expired token until the user re-logs in.
  const applyAdmittedSessionRefresh = useCallback(
    (event: AuthChangeEvent, nextSession: Session) => {
      const transition = beginAuthTransition(event);
      commitSession(nextSession, true);
      void enqueueAuthAnalytics(() => trackAuthEvent(event, nextSession));
      void applyCloudsyncAuthChange(event, nextSession, transition).catch(
        (error) => {
          console.warn(
            "[cloudsync] refreshed credentials could not be applied",
            error,
          );
        },
      );
    },
    [
      applyCloudsyncAuthChange,
      beginAuthTransition,
      commitSession,
      enqueueAuthAnalytics,
    ],
  );

  // While an explicit sign-out is clearing storage, a refresh must queue behind
  // it so the accepted session is written back after the clear completes.
  const isAdmittedSessionRefresh = useCallback(
    (nextSession: Session | null): nextSession is Session =>
      nextSession !== null &&
      admittedUserIdRef.current === nextSession.user.id &&
      authTransitionEventRef.current !== "SIGNED_OUT",
    [],
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
      if (isAdmittedSessionRefresh(session)) {
        applyAdmittedSessionRefresh(event, session);
        return;
      }
      void enqueueAuthChange(event, session);
    });

    return () => {
      subscription.unsubscribe();
    };
  }, [
    applyAdmittedSessionRefresh,
    enqueueAuthChange,
    isAdmittedSessionRefresh,
  ]);

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

      let requestSession = sessionRef.current;
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
    }, [refreshSession]);

  const getHeaders = useCallback(() => {
    if (!session) {
      return null;
    }

    // Callers may hold this closure across renders; sign with the committed
    // session so a refreshed token is used even before React re-renders.
    const committed = sessionRef.current;
    const current =
      committed && committed.user.id === session.user.id ? committed : session;
    const headers: Record<string, string> = {
      Authorization: `${current.token_type} ${current.access_token}`,
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
