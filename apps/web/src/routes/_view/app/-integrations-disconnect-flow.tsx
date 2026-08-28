import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { deleteConnection } from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import { captureOperationalError } from "@/lib/error-reporting";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

export function DisconnectFlow() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const { track } = useAnalytics();
  const [status, setStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("loading");

  const display = getIntegrationDisplay(search.integration_id);

  const handleDisconnect = async () => {
    if (!search.connection_id) {
      setStatus("error");
      track("integration_connection_failed", {
        integration: search.integration_id,
        mode: "disconnect",
        flow: search.flow,
        failure_stage: "validation",
      });
      return;
    }

    setStatus("loading");

    try {
      const token = await getAccessToken();
      const client = createClient({
        baseUrl: env.VITE_API_URL,
        headers: { Authorization: `Bearer ${token}` },
      });
      const { data, error } = await deleteConnection({
        client,
        body: {
          connection_id: search.connection_id,
          integration_id: search.integration_id,
        },
      });

      if (error || !data) {
        captureOperationalError(
          error ?? new Error("Integration was not disconnected"),
          {
            operation: "integration_disconnect",
            tags: { integration: search.integration_id },
          },
        );
        setStatus("error");
        track("integration_connection_failed", {
          integration: search.integration_id,
          mode: "disconnect",
          flow: search.flow,
          failure_stage: "disconnect",
        });
        return;
      }
    } catch (error) {
      captureOperationalError(error, {
        operation: "integration_disconnect",
        tags: { integration: search.integration_id },
      });
      setStatus("error");
      track("integration_connection_failed", {
        integration: search.integration_id,
        mode: "disconnect",
        flow: search.flow,
        failure_stage: "disconnect",
      });
      return;
    }

    setStatus("success");
    track("integration_disconnected", {
      integration: search.integration_id,
      flow: search.flow,
    });
    const callbackSearch =
      search.flow === "desktop"
        ? {
            integration_id: search.integration_id,
            status: "success" as const,
            disconnected_connection_id: search.connection_id,
            flow: "desktop" as const,
            scheme: search.scheme,
            return_to: search.return_to,
          }
        : {
            integration_id: search.integration_id,
            status: "success" as const,
            disconnected_connection_id: search.connection_id,
            flow: "web" as const,
            return_to: search.return_to,
          };
    void navigate({
      to: "/callback/integration/",
      search: callbackSearch,
    });
  };

  useMountEffect(() => {
    void handleDisconnect();
  });

  return (
    <IntegrationPageLayout>
      <div className="flex flex-col gap-3">
        <h1 className="font-sans text-3xl tracking-tight text-stone-700">
          Disconnect {display.name}
        </h1>
        <p className="text-neutral-600">
          {status === "error"
            ? `Could not disconnect ${display.name}.`
            : `Disconnecting ${display.name}...`}
        </p>
      </div>

      {status === "error" && (
        <div className="flex flex-col gap-4">
          <p className="text-red-600">
            Could not disconnect this integration. Please try again.
          </p>
          <IntegrationButton variant="danger" onClick={handleDisconnect}>
            Try again
          </IntegrationButton>
        </div>
      )}
    </IntegrationPageLayout>
  );
}
