import { ArrowRight } from "@phosphor-icons/react";
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

import { AccountAccessSection } from "./-account-access";
import { ApiKeysSection } from "./-account-api-keys";
import { DangerAreaSection } from "./-account-danger";
import { DevicesSection } from "./-account-devices";
import { IntegrationsSection } from "./-account-integrations";
import { PlanSection } from "./-account-plan";
import { ProfileInfoSection } from "./-account-profile-info";
import { accountSessionQueryKey } from "./-account-session";
import { SharedNotesSection } from "./-account-shares";

const validateSearch = z
  .object({
    success: z.coerce.boolean(),
    trial: z.enum(["started"]),
    scheme: desktopSchemeSchema,
    checkout: z.enum(["trial", "paid", "canceled", "failed"]),
    checkout_type: z.enum(["trial", "paid"]),
    source: z.enum([
      "onboarding",
      "settings",
      "trial_ended",
      "feature_gate",
      "unknown",
    ]),
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
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              The email attached to your account, and how long you've been
              around.
            </p>
            <div className="mt-6">
              <ProfileInfoSection email={user?.email} />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Your plan
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Billing runs through Stripe, and you can also manage it from the
              desktop app.
            </p>
            <div className="mt-6">
              <PlanSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Integrations
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Calendars and tools connected to Anarlog.
            </p>
            <div className="mt-6">
              <IntegrationsSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Synced devices
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Anarlog syncs up to five devices. Remove one to free a slot.
            </p>
            <div className="mt-6">
              <DevicesSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Shared notes
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Notes you've shared with others. Restricting a note turns off link
              and public access.
            </p>
            <div className="mt-6">
              <SharedNotesSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Cloud API keys
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Keys that let your own tools talk to the Anarlog Cloud API.
            </p>
            <div className="mt-6">
              <ApiKeysSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Session controls
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Sign out quickly whenever you need to.
            </p>
            <div className="mt-6">
              <AccountAccessSection />
            </div>
          </section>

          <section>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Danger area
            </h2>
            <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
              Account deletion lives here, tucked behind an extra deliberate
              step.
            </p>
            <div className="mt-6">
              <DangerAreaSection />
            </div>
          </section>

          <section>
            <article
              className="overflow-hidden rounded-[3px] border border-[#eadfce] bg-[#fffaf0] px-7 py-9 shadow-[0_18px_50px_rgba(68,54,36,0.12)] sm:px-10 sm:py-12"
              style={{
                backgroundImage:
                  "linear-gradient(115deg, rgba(255, 250, 240, 0.9), rgba(246, 236, 218, 0.82)), url('/textures/crumpled-paper.webp')",
                backgroundPosition: "center",
                backgroundSize: "cover",
              }}
            >
              <h2 className="font-hand text-3xl leading-none font-semibold text-[#363029]">
                Anarlog lives on your desktop
              </h2>
              <p className="mt-4 max-w-xl text-base leading-7 text-[#363029]">
                Notes, transcripts, and integrations all live in the app, on
                your device. Grab it if you haven't already.
              </p>
              <Link
                to="/download/"
                className="mt-6 inline-flex items-center gap-2 rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white"
              >
                <span>Download for free</span>
                <ArrowRight size={16} weight="bold" aria-hidden="true" />
              </Link>
            </article>
          </section>
        </div>
      </div>
    </main>
  );
}
