import Nango from "@nangohq/frontend";
import { useNavigate } from "@tanstack/react-router";
import { useRef, useState } from "react";

import { createSession } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

export function ConnectFlow() {
  const search = Route.useSearch();
  const navigate = useNavigate();
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
    updateStatus("loading");

    let sessionToken: string;

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
        inFlightRef.current = false;
        updateStatus("error");
        return;
      }
      sessionToken = data.token;
    } catch {
      inFlightRef.current = false;
      updateStatus("error");
      return;
    }

    updateStatus("connecting");

    const connect = nango.openConnectUI({
      onEvent: (event) => {
        if (event.type === "close") {
          if (
            statusRef.current !== "success" &&
            statusRef.current !== "error"
          ) {
            inFlightRef.current = false;
            updateStatus("idle");
          }
        } else if (event.type === "connect") {
          inFlightRef.current = false;
          updateStatus("success");
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

    connect.setSessionToken(sessionToken);
  };

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
            Anarlog will read your calendar list and event details—including
            titles, times, participants, locations, and meeting links—to show
            upcoming events and associate them with your private notes.
          </p>
          <p>
            Access is read-only. Anarlog cannot create, edit, or delete events.
            Calendar API responses pass through Nango's encrypted proxy before
            reaching Anarlog, and Nango stores the encrypted OAuth credentials
            needed to maintain the connection. Calendar data is then stored
            locally on your device.
          </p>
          <p>
            If you enable encrypted Cloud Sync or share a note, event context
            associated with that note can be included in the content you choose
            to sync or share.
          </p>
          {isGoogleCalendar && (
            <p>
              Google Calendar data is not sent to hosted AI or cloud
              transcription providers. While that data remains on your device,
              hosted and remote AI is disabled; on-device AI remains available.
            </p>
          )}
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
            Something went wrong. Please try again.
          </p>
          <IntegrationButton onClick={handleConnect}>
            Try again
          </IntegrationButton>
        </div>
      )}
    </IntegrationPageLayout>
  );
}
