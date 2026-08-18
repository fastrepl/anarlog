import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { jwtDecode } from "jwt-decode";
import { useEffect } from "react";
import { z } from "zod";

import { deriveBillingInfo, type SupabaseJwtPayload } from "@anlg/supabase";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import { useAnalytics } from "@/hooks/use-posthog";
import { checkoutSourceSchema } from "@/lib/checkout-source";

import { AccountAccessSection } from "./-account-access";
import { ApiKeysSection } from "./-account-api-keys";
import { DangerAreaSection } from "./-account-danger";
import { DevicesSection } from "./-account-devices";
import { IntegrationsSection } from "./-account-integrations";
import { PlanSection } from "./-account-plan";
import { ProfileInfoSection } from "./-account-profile-info";
import { ReferralSection } from "./-account-referrals";
import { accountSessionQueryKey } from "./-account-session";
import { SharedNotesSection } from "./-account-shares";

const validateSearch = z
  .object({
    success: z.coerce.boolean(),
    trial: z.enum(["started"]),
    scheme: desktopSchemeSchema,
    checkout: z.enum(["trial", "paid", "canceled", "failed"]),
    checkout_type: z.enum(["trial", "paid"]),
    source: checkoutSourceSchema,
    referral: z.enum(["ineligible"]),
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
  const { identify: identifyPosthog, track } = useAnalytics();
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!search.success && search.trial !== "started") {
      if (search.checkout === "canceled" || search.checkout === "failed") {
        track(`checkout_${search.checkout}`, {
          checkout_type: search.checkout_type ?? "unknown",
          entry_source: search.source ?? "unknown",
        });
      }
      return;
    }

    if (search.scheme) {
      window.location.href = `${search.scheme}://billing/refresh`;
      return;
    }

    const syncBillingAnalytics = async () => {
      const supabase = getSupabaseBrowserClient();
      const { data } = await supabase.auth.refreshSession();
      // The refreshed JWT carries the post-checkout billing claims; cached
      // account-session data is stale until it re-reads the session.
      void queryClient.invalidateQueries({ queryKey: accountSessionQueryKey });
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
  }, [
    identifyPosthog,
    queryClient,
    search.checkout,
    search.checkout_type,
    search.scheme,
    search.source,
    search.success,
    search.trial,
    track,
  ]);

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 pt-10 pb-16 md:px-8 md:pt-12 md:pb-24">
        <Link to="/" aria-label="Anarlog home" className="inline-flex">
          <AnarlogLogo className="h-8 w-auto" />
        </Link>

        <header className="mt-12 md:mt-16">
          <p className="font-hand text-2xl leading-none font-semibold text-[#756b5d]">
            Your account
          </p>
          <h1 className="font-hand mt-4 text-5xl leading-[0.98] font-semibold tracking-normal text-balance md:text-6xl">
            Welcome back,{" "}
            <mark className="bg-[#fff0b3] px-1 text-[#181613]">
              {user?.email?.split("@")[0] || "Guest"}
            </mark>
          </h1>
        </header>

        <div className="mt-14 flex flex-col gap-14 md:mt-16">
          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Profile
            </h2>
            <div className="mt-6">
              <ProfileInfoSection email={user?.email} />
            </div>
          </section>

          <section id="referrals" className="scroll-mt-8">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Refer friends
            </h2>
            <div className="mt-6">
              <ReferralSection ineligible={search.referral === "ineligible"} />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Your plan
            </h2>
            <div className="mt-6">
              <PlanSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Integrations
            </h2>
            <div className="mt-6">
              <IntegrationsSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Synced devices
            </h2>
            <div className="mt-6">
              <DevicesSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Shared notes
            </h2>
            <div className="mt-6">
              <SharedNotesSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Cloud API keys
            </h2>
            <div className="mt-6">
              <ApiKeysSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Session controls
            </h2>
            <div className="mt-6">
              <AccountAccessSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Danger area
            </h2>
            <div className="mt-6">
              <DangerAreaSection />
            </div>
          </section>
        </div>
      </div>
    </main>
  );
}
