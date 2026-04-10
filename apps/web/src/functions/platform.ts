import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader } from "@tanstack/react-start/server";

import { resolvePlatformFromUserAgent } from "@/hooks/use-platform";

export const getRequestPlatform = createServerFn({ method: "GET" }).handler(
  () => {
    return resolvePlatformFromUserAgent(getRequestHeader("user-agent"));
  },
);
