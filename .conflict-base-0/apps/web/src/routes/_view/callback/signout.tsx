import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { signOutFn } from "@/functions/auth";

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
    throw redirect({ to: search.redirect || "/" });
  },
});
