import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { persistReferralAttribution } from "@/functions/referrals";

export const Route = createFileRoute("/invite/$code")({
  params: {
    parse: (params) => ({
      code: z
        .string()
        .regex(/^[a-f0-9]{24}$/)
        .parse(params.code),
    }),
  },
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ params }) => {
    const result = await persistReferralAttribution({ data: params.code });
    if (result === "existing_account") {
      throw redirect({ href: "/app/account?referral=ineligible" } as any);
    }

    throw redirect({
      to: "/auth/",
      search: {
        flow: "web",
        redirect: "/app/account#referrals",
      },
    });
  },
});
