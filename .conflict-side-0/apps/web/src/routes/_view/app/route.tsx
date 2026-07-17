import { createFileRoute, redirect } from "@tanstack/react-router";

import { fetchUser } from "@/functions/auth";

export const Route = createFileRoute("/_view/app")({
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ location }) => {
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
