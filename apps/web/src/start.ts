import { createStart } from "@tanstack/react-start";

import { prepareShareRoutePrivacy } from "./lib/share-route-privacy";
import { trailingSlashMiddleware } from "./middleware/trailing-slash";
import { workspaceShareHostMiddleware } from "./middleware/workspace-share-host";
import { bootstrapBrowserTelemetry } from "./telemetry";

prepareShareRoutePrivacy();
bootstrapBrowserTelemetry();

export const startInstance = createStart(() => {
  return {
    requestMiddleware: [workspaceShareHostMiddleware, trailingSlashMiddleware],
  };
});
