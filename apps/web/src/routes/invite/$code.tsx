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
  component: () => (
    <main className="bg-page text-color flex min-h-screen items-center justify-center p-6">
      <div className="bg-surface border-border-subtle flex max-w-lg flex-col gap-4 rounded-xl border p-8">
        <h1 className="font-mono text-xl">This referral link is unavailable</h1>
        <p className="text-color-muted">
          This invite may have been used or the sender may have reached their
          limit. The referral's 30-day offer can't be applied through this link.
        </p>
        <a href="/auth/" className="underline">
          Create an account without this referral
        </a>
      </div>
    </main>
  ),
  beforeLoad: async ({ params }) => {
    const result = await persistReferralAttribution({ data: params.code });
    if (result === "unavailable") return;
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
