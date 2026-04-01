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
  await openUrlWithInstruction(url, "integration", (u) =>
    openerCommands.openUrl(u, null),
  );
}
