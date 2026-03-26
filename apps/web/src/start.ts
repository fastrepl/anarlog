import { createStart } from "@tanstack/react-start";

import { bootstrapBrowserTelemetry } from "./telemetry";

bootstrapBrowserTelemetry();

export const startInstance = createStart(() => {
  return {};
});
