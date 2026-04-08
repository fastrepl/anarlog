import { createFileRoute } from "@tanstack/react-router";
import { jwtDecode } from "jwt-decode";
import { useEffect } from "react";
import { z } from "zod";

import { deriveBillingInfo, type SupabaseJwtPayload } from "@hypr/supabase";

import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import { useAnalytics } from "@/hooks/use-posthog";

import { AccountAccessSection } from "./-account-access";
import { ProfileInfoSection } from "./-account-profile-info";

const validateSearch = z
  .object({
    success: z.coerce.boolean(),
    trial: z.enum(["started"]),
    scheme: desktopSchemeSchema,
  })
  .partial();

export const Route = createFileRoute("/_view/app/account")({
  validateSearch,
  component: Component,
  loader: async ({ context }) => ({ user: context.user }),
});

function Component() {
  const { user } = Route.useLoaderData();
  const search = Route.useSearch();
  const { identify: identifyPosthog } = useAnalytics();

  useEffect(() => {
    if (!search.success && search.trial !== "started") {
      return;
    }

    if (search.scheme) {
      window.location.href = `${search.scheme}://billing/refresh`;
      return;
    }

    const syncBillingAnalytics = async () => {
      const supabase = getSupabaseBrowserClient();
      const { data } = await supabase.auth.refreshSession();
      const accessToken = data.session?.access_token;
      const userId = data.session?.user.id;

      if (!accessToken || !userId) {
        return;
      }

      const billing = deriveBillingInfo(
        jwtDecode<SupabaseJwtPayload>(accessToken),
      );

      identifyPosthog(userId, {
        ...(data.session?.user.email ? { email: data.session.user.email } : {}),
        plan: billing.plan,
        trial_end_date: billing.trialEnd?.toISOString() ?? null,
      });
    };

    void syncBillingAnalytics();
  }, [identifyPosthog, search.scheme, search.success, search.trial]);

  return (
    <div>
      <div className="mx-auto min-h-[calc(100vh-200px)] max-w-6xl border-x border-neutral-100">
        <div className="flex items-center justify-center border-b border-neutral-100 bg-linear-to-b from-stone-50/30 to-stone-100/30 py-20">
          <h1 className="text-center font-serif text-3xl font-medium">
            Welcome back {user?.email?.split("@")[0] || "Guest"}
          </h1>
        </div>

        <div className="mx-auto mt-8 flex max-w-4xl flex-col gap-10 px-4 pb-20">
          <section className="space-y-4">
            <div className="space-y-2 px-1">
              <p className="text-xs font-medium tracking-[0.18em] text-neutral-400 uppercase">
                Account
              </p>
              <div>
                <h2 className="font-serif text-2xl font-medium text-stone-950">
                  Profile and account access
                </h2>
                <p className="text-sm text-neutral-600">
                  Update your email here. Billing and integrations are managed
                  in the desktop app.
                </p>
              </div>
            </div>

            <div className="space-y-6">
              <ProfileInfoSection email={user?.email} />
            </div>
          </section>

          <section className="space-y-4">
            <div className="space-y-2 px-1">
              <p className="text-xs font-medium tracking-[0.18em] text-neutral-400 uppercase">
                Access
              </p>
              <div>
                <h2 className="font-serif text-2xl font-medium text-stone-950">
                  Session controls
                </h2>
                <p className="text-sm text-neutral-600">
                  Sign out quickly, while keeping account deletion tucked behind
                  an extra deliberate step.
                </p>
              </div>
            </div>

            <AccountAccessSection />
          </section>
        </div>
      </div>
    </div>
  );
}
