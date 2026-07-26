import { useMutation } from "@tanstack/react-query";
import { useCallback, useRef } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { openUrlWithInstruction } from "@hypr/plugin-windows";

import { buildWebAppUrl } from "~/shared/utils";

export async function openIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "reconnect" | "disconnect",
  returnTo?: string,
) {
  if (!nangoIntegrationId) return;
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
  const url = await buildWebAppUrl("/app/integration", params);
  await openUrlWithInstruction(
    url,
    "integration",
    (u) => openerCommands.openUrl(u, null),
    { integrationId: nangoIntegrationId },
  );
}

export function useOpenIntegrationUrl() {
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
