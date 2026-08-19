import { t } from "@lingui/core/macro";
import { useMutation } from "@tanstack/react-query";
import { useCallback, useRef } from "react";

import { createSession } from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useAuth } from "~/auth";
import { env } from "~/env";
import { captureOperationalError } from "~/error-reporting";
import { addNangoSessionHandoff } from "~/shared/integration-handoff";
import { buildWebAppUrl } from "~/shared/utils";

export async function openIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "reconnect" | "disconnect",
  returnTo?: string,
  headers?: Record<string, string> | null,
  showInstruction = true,
) {
  if (!nangoIntegrationId) return;

  try {
    const params: Record<string, string> = {
      action,
      integration_id: nangoIntegrationId,
    };
    if (returnTo) {
      params.return_to = returnTo;
    }
    if (connectionId) {
      params.connection_id = connectionId;
    }

    let url = await buildWebAppUrl("/app/integration", params);

    if (action !== "disconnect") {
      if (!headers) {
        throw new Error("No authentication session is available");
      }

      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await createSession({
        client,
        body: {
          integration_id: nangoIntegrationId,
          mode: action,
          connection_id: connectionId,
        },
      });

      if (error) {
        throw error;
      }
      if (!data) {
        throw new Error("Integration session was not created");
      }

      url = addNangoSessionHandoff(url, data.token);
    }

    if (!showInstruction) {
      await openerCommands.openUrl(url, null);
      return;
    }

    await openUrlWithInstruction(
      url,
      "integration",
      (u) => openerCommands.openUrl(u, null),
      { integrationId: nangoIntegrationId },
    );
  } catch (error) {
    captureOperationalError(error, {
      operation: "integration_open",
      tags: {
        integration: nangoIntegrationId,
        mode: action,
      },
    });
    sonnerToast.error(t`Could not start the integration setup. Try again.`);
  }
}

export function useOpenIntegrationUrl() {
  const auth = useAuth();
  // React state cannot gate re-entry: a second click can land before the
  // pending state commits, opening a duplicate integration flow.
  const inFlightRef = useRef(false);
  const { mutate, isPending, variables } = useMutation({
    mutationFn: (input: {
      nangoIntegrationId: string | undefined;
      connectionId?: string;
      action: "connect" | "reconnect" | "disconnect";
      returnTo?: string;
    }) =>
      openIntegrationUrl(
        input.nangoIntegrationId,
        input.connectionId,
        input.action,
        input.returnTo,
        auth.getHeaders(),
      ),
  });

  const openIntegration = useCallback<typeof mutate>(
    (input, options) => {
      if (inFlightRef.current) {
        return;
      }
      inFlightRef.current = true;
      mutate(input, {
        ...options,
        onSettled: (...args) => {
          inFlightRef.current = false;
          options?.onSettled?.(...args);
        },
      });
    },
    [mutate],
  );

  return {
    openIntegration,
    openingAction: isPending ? (variables?.action ?? null) : null,
  };
}
