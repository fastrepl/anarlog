import Nango, { type ConnectUI } from "@nangohq/frontend";
import { useNavigate } from "@tanstack/react-router";
import { useRef, useState } from "react";

import { createSession } from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import { captureOperationalError } from "@/lib/error-reporting";
import { getConnectionErrorMessage } from "@/lib/integration-connection-error";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

export function ConnectFlow({ sessionToken }: { sessionToken?: string } = {}) {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const { track } = useAnalytics();
  const isGoogleCalendar = search.integration_id === "google-calendar";
  const isOutlookCalendar = search.integration_id === "outlook";
  const isConnectedCalendar = isGoogleCalendar || isOutlookCalendar;
  const [nango] = useState(() => new Nango());
  const [status, setStatus] = useState<
    "idle" | "loading" | "connecting" | "success" | "error"
  >("idle");
  const statusRef = useRef<
    "idle" | "loading" | "connecting" | "success" | "error"
  >("idle");
  const inFlightRef = useRef(false);
  const connectUIRef = useRef<ConnectUI | null>(null);
  const disposedRef = useRef(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);

  const display = getIntegrationDisplay(search.integration_id);

  const updateStatus = (
    nextStatus: "idle" | "loading" | "connecting" | "success" | "error",
  ) => {
    statusRef.current = nextStatus;
    setStatus(nextStatus);
  };

  const handleConnect = async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    setConnectionError(null);
    updateStatus("loading");
    track("integration_connection_started", {
      integration: search.integration_id,
      mode: search.action,
      flow: search.flow,
    });

    let connectSessionToken = sessionToken;

    if (!connectSessionToken) {
      try {
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
          captureOperationalError(
            error ?? new Error("Integration session was not created"),
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
          return;
        }
        connectSessionToken = data.token;
      } catch (error) {
        captureOperationalError(error, {
          operation: "integration_connection_session",
          tags: {
            integration: search.integration_id,
            mode: search.action,
          },
        });
        inFlightRef.current = false;
        updateStatus("error");
        track("integration_connection_failed", {
          integration: search.integration_id,
          mode: search.action,
          flow: search.flow,
          failure_stage: "session",
        });
        return;
      }
    }

    if (disposedRef.current) return;

    updateStatus("connecting");

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
          const { errorType } = event.payload;
          captureOperationalError(
            new Error("Integration authorization failed"),
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
            getConnectionErrorMessage(
              errorType,
              display.name,
              search.integration_id,
            ),
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
        } else if (event.type === "connect") {
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
        }
      },
    });

    connectUIRef.current = connect;
    connect.setSessionToken(connectSessionToken);
  };

  // Nango's Connect UI repeats the connect prompt, so only calendars (which
  // must show the OAuth data-use disclosure first) wait for a manual click.
  // The Connect UI lives outside the React tree, so unmount must close it and
  // stop in-flight session work from opening one on a stale view.
  useMountEffect(() => {
    disposedRef.current = false;
    if (!isConnectedCalendar) {
      void handleConnect();
    }
    return () => {
      disposedRef.current = true;
      connectUIRef.current?.close();
      connectUIRef.current = null;
    };
  });

  const isLoading = status === "loading";
  const isConnecting = status === "connecting";
  const consentProvider = isGoogleCalendar ? "Google" : "Microsoft";

  return (
    <IntegrationPageLayout>
      <div className="flex flex-col gap-3">
        <h1 className="font-sans text-3xl tracking-tight text-stone-700">
          Connect {display.name}
        </h1>
        <p className="text-neutral-600">
          {isConnecting ? display.connectingHint : display.description}
        </p>
      </div>

      {isConnectedCalendar && !isConnecting && status !== "success" && (
        <div className="flex flex-col gap-3 rounded-2xl border border-stone-200 bg-stone-50 p-5 text-left text-sm leading-6 text-stone-700">
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
        </div>
      )}

      {(status === "idle" || isLoading) && (
        <IntegrationButton onClick={handleConnect} disabled={isLoading}>
          {isLoading && (
            <svg
              className="h-4 w-4 animate-spin text-white"
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
              />
            </svg>
          )}
          {isLoading
            ? "Connecting…"
            : isConnectedCalendar
              ? `Continue to ${consentProvider}`
              : `Connect ${display.name}`}
        </IntegrationButton>
      )}

      {status === "error" && (
        <div className="flex flex-col gap-4">
          <p className="text-red-600">
            {connectionError ?? "Something went wrong. Please try again."}
          </p>
          <IntegrationButton onClick={handleConnect}>
            Try again
          </IntegrationButton>
        </div>
      )}
    </IntegrationPageLayout>
  );
}
