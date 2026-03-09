import { PostHogProvider as PostHogReactProvider } from "@posthog/react";
import posthog from "posthog-js";

import { env } from "../env";

const isDev = import.meta.env.DEV;

if (typeof window !== "undefined" && env.VITE_POSTHOG_API_KEY && !isDev) {
  posthog.init(env.VITE_POSTHOG_API_KEY, {
    api_host: env.VITE_POSTHOG_HOST,
    autocapture: true,
    capture_pageview: true,
  });
}

export function PostHogProvider({ children }: { children: React.ReactNode }) {
  if (!env.VITE_POSTHOG_API_KEY) {
    return <>{children}</>;
  }
  return (
    <PostHogReactProvider client={posthog}>{children}</PostHogReactProvider>
  );
}
