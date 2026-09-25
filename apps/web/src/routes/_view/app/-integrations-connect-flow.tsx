import Nango, { type ConnectUI } from "@nangohq/frontend";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";

import { createSession } from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";
import { CircleNotch } from "@anlg/ui/components/icons";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import { captureOperationalError } from "@/lib/error-reporting";
import {
  getConnectionErrorMessage,
  getNangoAuthErrorType,
  shouldReportConnectionAuthError,
} from "@/lib/integration-connection-error";
import {
  isConnectSessionFailed,
  usesHeadlessOAuth,
} from "@/lib/integration-headless-auth";

import {
  IntegrationButton,
  integrationIcon,
  IntegrationPageLayout,
} from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

export function ConnectFlow({ sessionToken }: { sessionToken?: string } = {}) {
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
        headers: { Authorization: `Bearer ${token}` },
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
    if (shouldReportConnectionAuthError(errorType)) {
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
    }
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
    const nango = new Nango({ connectSessionToken: token });
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
          finishWithAuthError({ type: event.payload.errorType });
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
      <div className="flex flex-col items-center gap-3">
        <span
          aria-hidden="true"
          className="flex size-12 items-center justify-center rounded-2xl border border-stone-200 bg-stone-50"
        >
          {integrationIcon(search.integration_id, 24)}
        </span>
        <h1 className="font-sans text-3xl tracking-tight text-stone-700">
          Connect {display.name}
        </h1>
        <p className="text-neutral-600">
          {isConnecting ? display.connectingHint : display.description}
        </p>
      </div>

      {isConnectedCalendar && !isConnecting && status !== "success" && (
        <ul className="flex list-disc flex-col gap-2 pl-5 text-left text-sm leading-6 text-neutral-600">
          <li>
            Read-only access — Anarlog can’t create, edit, or delete events.
          </li>
          <li>
            Data passes through Nango’s encrypted proxy and stays on your
            device.
          </li>
          <li>
            Event context is only included if you sync, share, or use AI on a
            note.
          </li>
        </ul>
      )}

      {!sessionFailed && (status === "idle" || isLoading) && (
        <IntegrationButton
          onClick={handleConnect}
          disabled={isLoading || !connectSessionToken}
        >
          {(status === "loading" || sessionLoading) && (
            <CircleNotch
              className="size-4 animate-spin text-white"
              aria-hidden="true"
            />
          )}
          {status === "loading"
            ? "Connecting…"
            : isConnectedCalendar
              ? `Continue to ${consentProvider}`
              : `Connect ${display.name}`}
        </IntegrationButton>
      )}

      {isConnectedCalendar && !isConnecting && status !== "success" && (
        <p className="text-xs text-neutral-500">
          Read our{" "}
          <a className="underline" href="/privacy">
            Privacy Policy
          </a>{" "}
          and{" "}
          <a
            className="underline"
            href="https://docs.anarlog.so/calendar#manage-or-delete-connected-calendar-data"
          >
            calendar data instructions
          </a>
          .
        </p>
      )}

      {(status === "error" || (sessionFailed && status === "idle")) && (
        <div className="flex flex-col gap-4">
          <p className="text-red-600">
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
