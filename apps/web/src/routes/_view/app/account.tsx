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
import {
  AccountNav,
  ACCOUNT_SECTIONS,
  useActiveAccountSection,
} from "./-account-nav";
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
    perk: z.enum(["applied", "claimed", "invalid"]),
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
  const activeSectionId = useActiveAccountSection();

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
      <div className="mx-auto w-full max-w-[700px] px-5 pt-10 pb-16 md:px-8 md:pt-12 md:pb-24 lg:max-w-[980px]">
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

        <div className="isolate mt-10 grid items-start gap-8 md:mt-12 lg:mt-16 lg:grid-cols-[12rem_minmax(0,1fr)] lg:gap-x-12 lg:gap-y-0">
          <div className="sticky top-0 z-10 -mx-5 border-b border-[#ede7dc] bg-white py-3 md:-mx-8 lg:top-10 lg:mx-0 lg:border-0 lg:bg-transparent lg:py-0">
            <AccountNav activeId={activeSectionId} />
          </div>

          <div className="flex min-w-0 flex-col gap-14">
            <AccountSection id="profile">
              <ProfileInfoSection email={user?.email} />
            </AccountSection>

            <AccountSection id="plan">
              <PlanSection perk={search.perk} />
            </AccountSection>

            <AccountSection id="referrals">
              <ReferralSection ineligible={search.referral === "ineligible"} />
            </AccountSection>

            <AccountSection id="integrations">
              <IntegrationsSection />
            </AccountSection>

            <AccountSection id="devices">
              <DevicesSection />
            </AccountSection>

            <AccountSection id="shares">
              <SharedNotesSection />
            </AccountSection>

            <AccountSection id="api-keys">
              <ApiKeysSection />
            </AccountSection>

            <AccountSection id="session">
              <AccountAccessSection />
            </AccountSection>

            <AccountSection id="danger">
              <DangerAreaSection />
            </AccountSection>
          </div>
        </div>
      </div>
    </main>
  );
}

function AccountSection({
  id,
  children,
}: {
  id: (typeof ACCOUNT_SECTIONS)[number]["id"];
  children: React.ReactNode;
}) {
  const title = ACCOUNT_SECTIONS.find((section) => section.id === id)?.label;

  return (
    <section id={id} className="scroll-mt-20 lg:scroll-mt-10">
      <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
        {title}
      </h2>
      <div className="mt-6">{children}</div>
    </section>
  );
}
