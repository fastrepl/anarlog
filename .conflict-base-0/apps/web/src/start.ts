import { createStart } from "@tanstack/react-start";

import { prepareShareRoutePrivacy } from "./lib/share-route-privacy";
import { bootstrapBrowserTelemetry } from "./telemetry";

prepareShareRoutePrivacy();
bootstrapBrowserTelemetry();

export const startInstance = createStart(() => {
  return {};
});
