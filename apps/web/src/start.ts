import { createStart } from "@tanstack/react-start";

import { prepareNangoSessionHandoff } from "./lib/integration-handoff";
import { prepareShareRoutePrivacy } from "./lib/share-route-privacy";
import { trailingSlashMiddleware } from "./middleware/trailing-slash";
import { workspaceShareHostMiddleware } from "./middleware/workspace-share-host";
import { bootstrapBrowserTelemetry } from "./telemetry";

prepareShareRoutePrivacy();
prepareNangoSessionHandoff();
bootstrapBrowserTelemetry();

export const startInstance = createStart(() => {
  return {
    requestMiddleware: [workspaceShareHostMiddleware, trailingSlashMiddleware],
  };
});
