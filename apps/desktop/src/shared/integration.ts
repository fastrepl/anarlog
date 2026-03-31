import { commands as openerCommands } from "@hypr/plugin-opener2";

import { buildWebAppUrl } from "~/shared/utils";

export async function buildIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "reconnect" | "disconnect",
  returnTo?: string,
): Promise<string | undefined> {
  if (!nangoIntegrationId) return undefined;
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
  return buildWebAppUrl("/app/integration", params);
}

export async function openIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "reconnect" | "disconnect",
  returnTo?: string,
) {
  const url = await buildIntegrationUrl(
    nangoIntegrationId,
    connectionId,
    action,
    returnTo,
  );
  if (!url) return;
  await openerCommands.openUrl(url, null);
}
