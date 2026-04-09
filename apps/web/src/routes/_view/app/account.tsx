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
      <div className="mx-auto min-h-[calc(100vh-200px)] max-w-6xl">
        <div className="border-color-brand flex items-center justify-start border-b py-20">
          <h1 className="text-color text-left font-mono text-3xl font-medium">
            Welcome back {user?.email?.split("@")[0] || "Guest"}
          </h1>
        </div>

        <div className="mx-auto mt-8 flex flex-col gap-10 pb-20">
          <section className="space-y-4">
            <div className="space-y-2 px-1">
              <div>
                <h2 className="text-fg font-mono text-2xl font-medium">
                  Profile and account access
                </h2>
                <p className="text-fg-muted text-sm">
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
              <div>
                <h2 className="text-color font-mono text-2xl font-medium">
                  Session controls
                </h2>
                <p className="text-color-muted text-sm">
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
