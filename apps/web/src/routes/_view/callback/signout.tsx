import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { signOutFn } from "@/functions/auth";
import { resetPrivateRouteAnalyticsIdentity } from "@/lib/private-route-analytics";

const validateSearch = z.object({
  redirect: z.string().optional(),
});

export const Route = createFileRoute("/_view/callback/signout")({
  validateSearch,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ search }) => {
    await signOutFn();
    resetPrivateRouteAnalyticsIdentity();
    throw redirect({ to: search.redirect || "/" });
  },
});
