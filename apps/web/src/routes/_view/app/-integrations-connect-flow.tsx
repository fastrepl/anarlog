import Nango from "@nangohq/frontend";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";

import { createSession } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

export function ConnectFlow() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const [nango] = useState(() => new Nango());
  const [status, setStatus] = useState<
    "idle" | "loading" | "connecting" | "success" | "error"
  >("idle");
  const statusRef = useRef(status);
  const inFlightRef = useRef(false);

  useEffect(() => {
    statusRef.current = status;
  }, [status]);

  const display = getIntegrationDisplay(search.integration_id);
  const showGoogleDisclosure = search.integration_id === "google-calendar";

  const handleConnect = async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    setStatus("loading");

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
        setStatus("error");
        return;
      }
      sessionToken = data.token;
    } catch {
      inFlightRef.current = false;
      setStatus("error");
      return;
    }

    setStatus("connecting");

    const connect = nango.openConnectUI({
      onEvent: (event) => {
        if (event.type === "close") {
          if (
            statusRef.current !== "success" &&
            statusRef.current !== "error"
          ) {
            inFlightRef.current = false;
            statusRef.current = "idle";
            setStatus("idle");
          }
        } else if (event.type === "connect") {
          inFlightRef.current = false;
          statusRef.current = "success";
          setStatus("success");
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

  return (
    <IntegrationPageLayout>
      <div className="flex flex-col gap-3">
        <h1 className="font-serif text-3xl tracking-tight text-stone-700">
          Connect {display.name}
        </h1>
        <p className="text-neutral-600">
          {isConnecting
            ? display.connectingHint
            : showGoogleDisclosure
              ? "Review the disclosure below, then continue to Google to connect your account."
              : display.description}
        </p>
      </div>

      {showGoogleDisclosure && <GoogleCalendarDisclosure />}

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
            : showGoogleDisclosure
              ? "Continue to Google"
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

function GoogleCalendarDisclosure() {
  return (
    <div className="rounded-3xl border border-neutral-200 bg-white/80 p-5 text-left shadow-xs">
      <h2 className="text-sm font-medium text-neutral-900">
        Before you continue
      </h2>
      <p className="mt-2 text-sm leading-6 text-neutral-600">
        By connecting Google Calendar, Char will access the calendars and event
        details you choose to sync, including calendar names, event titles,
        descriptions, locations, meeting links, start and end times, organizers,
        and attendees.
      </p>
      <ul className="mt-3 list-disc space-y-2 pl-5 text-sm leading-6 text-neutral-600">
        <li>
          We use this data to sync meetings into Char, power reminders, show
          meeting context, and link notes to calendar events.
        </li>
        <li>
          Synced calendar data is primarily stored locally on your device. We
          also store limited connection metadata to keep the integration
          working.
        </li>
        <li>
          We do not use Google Calendar data for advertising, marketing
          personalization, or generalized AI model training.
        </li>
      </ul>
      <p className="mt-3 text-xs leading-5 text-neutral-500">
        Continue only if you want to authorize this connection. See our{" "}
        <a href="/legal/privacy" className="underline hover:text-neutral-700">
          Privacy Policy
        </a>{" "}
        for details.
      </p>
    </div>
  );
}
