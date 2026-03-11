import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { deleteConnection } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { getIntegrationDisplay, Route } from "./integration";

export function DisconnectFlow() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const [status, setStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");

  const display = getIntegrationDisplay(search.integration_id);

  const handleDisconnect = async () => {
    if (!search.connection_id) {
      setStatus("error");
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
        setStatus("error");
        return;
      }
    } catch {
      setStatus("error");
      return;
    }

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
  };

  return (
    <IntegrationPageLayout>
      <div className="flex flex-col gap-3">
        <h1 className="font-serif text-3xl tracking-tight text-stone-700">
          Disconnect {display.name}
        </h1>
        <p className="text-neutral-600">
          This will stop syncing data from {display.name}.
        </p>
      </div>

      {status !== "error" && (
        <IntegrationButton
          variant="danger"
          onClick={handleDisconnect}
          disabled={status === "loading" || !search.connection_id}
        >
          {status === "loading" ? "Disconnecting..." : "Disconnect"}
        </IntegrationButton>
      )}

      {status === "error" && (
        <div className="flex flex-col gap-4">
          <p className="text-red-600">
            Could not disconnect this integration. Please try again.
          </p>
          <IntegrationButton onClick={handleDisconnect}>
            Try again
          </IntegrationButton>
        </div>
      )}
    </IntegrationPageLayout>
  );
}
