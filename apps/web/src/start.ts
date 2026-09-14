import { createMiddleware, createStart } from "@tanstack/react-start";

import { invalidAuthSearchResponse } from "./functions/auth-search";
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
    requestMiddleware: [
      workspaceShareHostMiddleware,
      createMiddleware({ type: "request" }).server(
        ({ request, next }) => invalidAuthSearchResponse(request) ?? next(),
      ),
      trailingSlashMiddleware,
    ],
  };
});
