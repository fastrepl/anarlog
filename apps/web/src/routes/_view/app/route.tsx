import { createFileRoute, redirect } from "@tanstack/react-router";

import { fetchUser } from "@/functions/auth";

export const Route = createFileRoute("/_view/app")({
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ location }) => {
    // TODO: remove this stub — temporary bypass for local dev without Supabase
    const DEV_BYPASS_AUTH = true;
    if (DEV_BYPASS_AUTH) {
      return {
        user: {
          id: "00000000-0000-0000-0000-000000000000",
          email: "dev@localhost",
        },
      };
    }

    const user = await fetchUser();
    if (!user) {
      const searchStr =
        Object.keys(location.search).length > 0
          ? `?${new URLSearchParams(location.search as Record<string, string>).toString()}`
          : "";
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          redirect: location.pathname + searchStr,
        },
      });
    }
    return { user };
  },
});
