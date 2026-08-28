import Nango, { type ConnectUI } from "@nangohq/frontend";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";

import { createSession } from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";
import { fonts, media } from "@anlg/design-system/tokens.stylex";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import { captureOperationalError } from "@/lib/error-reporting";
import {
  getConnectionErrorMessage,
  getNangoAuthErrorType,
} from "@/lib/integration-connection-error";
import {
  isConnectSessionFailed,
  usesHeadlessOAuth,
} from "@/lib/integration-headless-auth";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style2: {
    fontFamily: fonts.sans,
    fontSize: "1.875rem",
    lineHeight: "2.25rem",
    letterSpacing: "-.025em",
    color: "#44403c",
  },
  style3: {
    color: "#525252",
  },
  style4: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e7e5e4",
    backgroundColor: "#fafaf9",
    padding: "1.25rem",
    textAlign: "left",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#44403c",
  },
  style5: {
    textDecorationLine: "underline",
  },
  style6: {
    height: "1rem",
    width: "1rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: {
      default: spin,
      [media.reducedMotion]: "none",
    },
    color: "#fff",
  },
  style7: {
    opacity: 0.25,
  },
  style8: {
    opacity: 0.75,
  },
  style9: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style10: {
    color: "#dc2626",
  },
});
export function ConnectFlow({
  sessionToken,
}: {
  sessionToken?: string;
} = {}) {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const { track } = useAnalytics();
  const isGoogleCalendar = search.integration_id === "google-calendar";
  const isOutlookCalendar = search.integration_id === "outlook";
  const isConnectedCalendar = isGoogleCalendar || isOutlookCalendar;
  const skipConnectUI = usesHeadlessOAuth(search.integration_id);
  const [status, setStatus] = useState<
    "idle" | "loading" | "connecting" | "success" | "error"
  >("idle");
  const statusRef = useRef<
    "idle" | "loading" | "connecting" | "success" | "error"
  >("idle");
  const inFlightRef = useRef(false);
  const nangoRef = useRef<Nango | null>(null);
  const connectUIRef = useRef<ConnectUI | null>(null);
  const disposedRef = useRef(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const display = getIntegrationDisplay(search.integration_id);
  const sessionQuery = useQuery({
    queryKey: [
      "nango-connect-session",
      search.integration_id,
      search.action,
      search.connection_id ?? "",
    ],
    queryFn: async () => {
      const token = await getAccessToken();
      const apiClient = createClient({
        baseUrl: env.VITE_API_URL,
        headers: {
          Authorization: `Bearer ${token}`,
        },
      });
      const { data, error } = await createSession({
        client: apiClient,
        body: {
          integration_id: search.integration_id,
          mode: search.action as "connect" | "reconnect",
          connection_id: search.connection_id,
        },
      });
      if (error || !data) {
        throw error ?? new Error("Integration session was not created");
      }
      return data;
    },
    enabled: !sessionToken,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: 1,
  });
  const connectSessionToken = sessionToken ?? sessionQuery.data?.token;
  const sessionFailed = isConnectSessionFailed({
    handedOffToken: sessionToken,
    isError: sessionQuery.isError,
    token: sessionQuery.data?.token,
  });
  const sessionLoading =
    !sessionToken && (sessionQuery.isPending || sessionQuery.isFetching);
  const reportedSessionErrorCountRef = useRef(0);
  const updateStatus = (
    nextStatus: "idle" | "loading" | "connecting" | "success" | "error",
  ) => {
    statusRef.current = nextStatus;
    setStatus(nextStatus);
  };
  const finishWithSessionError = () => {
    if (disposedRef.current) return;
    captureOperationalError(
      sessionQuery.error ?? new Error("Integration session was not created"),
      {
        operation: "integration_connection_session",
        tags: {
          integration: search.integration_id,
          mode: search.action,
        },
      },
    );
    inFlightRef.current = false;
    updateStatus("error");
    track("integration_connection_failed", {
      integration: search.integration_id,
      mode: search.action,
      flow: search.flow,
      failure_stage: "session",
    });
  };
  const finishWithAuthError = (error: unknown) => {
    if (disposedRef.current) return;
    const errorType = getNangoAuthErrorType(error);
    captureOperationalError(
      error instanceof Error
        ? error
        : new Error("Integration authorization failed"),
      {
        operation: "integration_connection_authorization",
        tags: {
          integration: search.integration_id,
          mode: search.action,
          error_type: errorType,
        },
      },
    );
    inFlightRef.current = false;
    setConnectionError(
      getConnectionErrorMessage(errorType, display.name, search.integration_id),
    );
    updateStatus("error");
    connectUIRef.current?.close();
    track("integration_connection_failed", {
      integration: search.integration_id,
      mode: search.action,
      flow: search.flow,
      failure_stage: "authorization",
      error_type: errorType,
    });
    if (!sessionToken) {
      void sessionQuery.refetch();
    }
  };
  const finishWithSuccess = () => {
    if (disposedRef.current) return;
    inFlightRef.current = false;
    updateStatus("success");
    track("integration_connection_succeeded", {
      integration: search.integration_id,
      mode: search.action,
      flow: search.flow,
    });
    const callbackSearch =
      search.flow === "desktop"
        ? {
            integration_id: search.integration_id,
            status: "success" as const,
            flow: "desktop" as const,
            scheme: search.scheme,
            return_to: search.return_to,
          }
        : {
            integration_id: search.integration_id,
            status: "success" as const,
            flow: "web" as const,
            return_to: search.return_to,
          };
    void navigate({
      to: "/callback/integration/",
      search: callbackSearch,
    });
  };
  const startHeadlessAuth = (token: string) => {
    // nango.auth() opens the popup synchronously. Do not await before this
    // call or browsers will block the provider window.
    const nango = new Nango({
      connectSessionToken: token,
    });
    nangoRef.current = nango;
    // The connect/reconnect session already binds the connection. Passing a
    // client-side connection ID is rejected by Nango's session model.
    const auth = nango.auth(search.integration_id, {
      detectClosedAuthWindow: true,
    });
    updateStatus("connecting");
    void auth.then(finishWithSuccess).catch(finishWithAuthError);
  };
  const startConnectUI = (token: string) => {
    const nango = new Nango();
    nangoRef.current = nango;
    const connect = nango.openConnectUI({
      detectClosedAuthWindow: true,
      onEvent: (event) => {
        if (event.type === "close") {
          if (
            statusRef.current !== "success" &&
            statusRef.current !== "error"
          ) {
            inFlightRef.current = false;
            updateStatus("idle");
            track("integration_connection_failed", {
              integration: search.integration_id,
              mode: search.action,
              flow: search.flow,
              failure_stage: "cancelled",
            });
          }
        } else if (event.type === "error") {
          finishWithAuthError({
            type: event.payload.errorType,
          });
        } else if (event.type === "connect") {
          finishWithSuccess();
        }
      },
    });
    connectUIRef.current = connect;
    updateStatus("connecting");
    connect.setSessionToken(token);
  };
  const handleConnect = () => {
    if (inFlightRef.current) return;
    if (sessionLoading) return;
    if (sessionFailed || !connectSessionToken) {
      finishWithSessionError();
      return;
    }
    inFlightRef.current = true;
    setConnectionError(null);
    updateStatus("loading");
    track("integration_connection_started", {
      integration: search.integration_id,
      mode: search.action,
      flow: search.flow,
    });
    if (skipConnectUI) {
      startHeadlessAuth(connectSessionToken);
      return;
    }
    startConnectUI(connectSessionToken);
  };
  useEffect(() => {
    if (!sessionFailed) return;
    if (
      reportedSessionErrorCountRef.current === sessionQuery.errorUpdateCount
    ) {
      return;
    }
    reportedSessionErrorCountRef.current = sessionQuery.errorUpdateCount;
    finishWithSessionError();
  }, [sessionFailed, sessionQuery.errorUpdateCount]);
  useMountEffect(() => {
    disposedRef.current = false;
    return () => {
      disposedRef.current = true;
      nangoRef.current?.clear();
      nangoRef.current = null;
      connectUIRef.current?.close();
      connectUIRef.current = null;
    };
  });
  const isLoading = status === "loading" || sessionLoading;
  const isConnecting = status === "connecting";
  const consentProvider = isGoogleCalendar ? "Google" : "Microsoft";
  return (
    <IntegrationPageLayout>
      <div {...stylex.props(styles.style1)}>
        <h1 {...stylex.props(styles.style2)}>Connect {display.name}</h1>
        <p {...stylex.props(styles.style3)}>
          {isConnecting ? display.connectingHint : display.description}
        </p>
      </div>

      {isConnectedCalendar && !isConnecting && status !== "success" && (
        <div {...stylex.props(styles.style4)}>
          <p>
            Anarlog reads your calendar and event details to show upcoming
            events and link them to private notes. Access is read-only: Anarlog
            cannot create, edit, or delete events.
          </p>
          <p>
            Calendar data passes through Nango's encrypted proxy and is stored
            locally on your device. Nango securely stores the credentials needed
            to keep your calendar connected.
          </p>
          <p>
            If you use encrypted Cloud Sync or share a note, its event context
            may be included.
          </p>
          <p>
            Contact enhancement from event details is processed locally on your
            device. If you choose to use AI on an event-linked note, relevant
            note context such as the event title and participants may go to the
            language model you selected. Local models keep that processing on
            your device.
          </p>
          <p>
            Read our{" "}
            <a {...stylex.props(styles.style5)} href="/privacy">
              Privacy Policy
            </a>{" "}
            and{" "}
            <a
              {...stylex.props(styles.style5)}
              href="https://docs.anarlog.so/calendar#manage-or-delete-connected-calendar-data"
            >
              calendar data instructions
            </a>
            .
          </p>
        </div>
      )}

      {!sessionFailed && (status === "idle" || isLoading) && (
        <IntegrationButton
          onClick={handleConnect}
          disabled={isLoading || !connectSessionToken}
        >
          {(status === "loading" || sessionLoading) && (
            <svg
              {...stylex.props(styles.style6)}
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                {...stylex.props(styles.style7)}
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                {...stylex.props(styles.style8)}
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
              />
            </svg>
          )}
          {status === "loading"
            ? "Connecting…"
            : isConnectedCalendar
              ? `Continue to ${consentProvider}`
              : `Connect ${display.name}`}
        </IntegrationButton>
      )}

      {(status === "error" || (sessionFailed && status === "idle")) && (
        <div {...stylex.props(styles.style9)}>
          <p {...stylex.props(styles.style10)}>
            {connectionError ?? "Something went wrong. Please try again."}
          </p>
          <IntegrationButton
            disabled={sessionQuery.isFetching}
            onClick={() => {
              setConnectionError(null);
              updateStatus("idle");
              if (!connectSessionToken) {
                void sessionQuery.refetch();
                return;
              }
              handleConnect();
            }}
          >
            Try again
          </IntegrationButton>
        </div>
      )}
    </IntegrationPageLayout>
  );
}
