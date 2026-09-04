import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { lazy, Suspense, useEffect, useState } from "react";
import { z } from "zod";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import {
  ACCOUNT_SECTIONS,
  type AccountSectionId,
  type AccountTabId,
  accountTabForSection,
  resolveAccountTab,
  sectionsForAccountTab,
} from "@/lib/account-tabs";
import { checkoutSourceSchema } from "@/lib/checkout-source";
import { capturePrivateRouteEvent } from "@/lib/private-route-analytics";

import { AccountTabs } from "./-account-nav";
import { accountSessionQueryKey } from "./-account-session";

const loadAccountAccessSection = () => import("./-account-access");
const loadApiKeysSection = () => import("./-account-api-keys");
const loadDangerAreaSection = () => import("./-account-danger");
const loadDevicesSection = () => import("./-account-devices");
const loadIntegrationsSection = () => import("./-account-integrations");
const loadPlanSection = () => import("./-account-plan");
const loadProfileInfoSection = () => import("./-account-profile-info");
const loadReferralSection = () => import("./-account-referrals");
const loadSharedNotesSection = () => import("./-account-shares");

const AccountAccessSection = lazy(() =>
  loadAccountAccessSection().then((module) => ({
    default: module.AccountAccessSection,
  })),
);
const ApiKeysSection = lazy(() =>
  loadApiKeysSection().then((module) => ({ default: module.ApiKeysSection })),
);
const DangerAreaSection = lazy(() =>
  loadDangerAreaSection().then((module) => ({
    default: module.DangerAreaSection,
  })),
);
const DevicesSection = lazy(() =>
  loadDevicesSection().then((module) => ({ default: module.DevicesSection })),
);
const IntegrationsSection = lazy(() =>
  loadIntegrationsSection().then((module) => ({
    default: module.IntegrationsSection,
  })),
);
const PlanSection = lazy(() =>
  loadPlanSection().then((module) => ({ default: module.PlanSection })),
);
const ProfileInfoSection = lazy(() =>
  loadProfileInfoSection().then((module) => ({
    default: module.ProfileInfoSection,
  })),
);
const ReferralSection = lazy(() =>
  loadReferralSection().then((module) => ({
    default: module.ReferralSection,
  })),
);
const SharedNotesSection = lazy(() =>
  loadSharedNotesSection().then((module) => ({
    default: module.SharedNotesSection,
  })),
);

const accountTabPreloaders: Record<AccountTabId, () => Promise<unknown>> = {
  account: () =>
    Promise.all([
      loadProfileInfoSection(),
      loadPlanSection(),
      loadReferralSection(),
      loadAccountAccessSection(),
      loadDangerAreaSection(),
    ]),
  connections: () =>
    Promise.all([
      loadIntegrationsSection(),
      loadDevicesSection(),
      loadSharedNotesSection(),
    ]),
  developer: () => Promise.all([loadApiKeysSection()]),
};

function preloadAccountTab(tabId: AccountTabId) {
  void accountTabPreloaders[tabId]().catch(() => undefined);
}

function scrollHashSectionIntoView(element: HTMLElement | null) {
  if (element && window.location.hash === `#${element.id}`) {
    element.scrollIntoView({ block: "start" });
  }
}

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
    tab: z.enum(["account", "connections", "developer"]),
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
  const navigate = useNavigate({ from: Route.fullPath });
  const queryClient = useQueryClient();
  const [hash, setHash] = useState("");
  const [optimisticTab, setOptimisticTab] = useState<AccountTabId | null>(null);
  const routeTab = resolveAccountTab({ tab: search.tab, hash });
  const activeTab = optimisticTab ?? routeTab;

  useEffect(() => {
    if (!search.success && search.trial !== "started") {
      if (search.checkout === "canceled" || search.checkout === "failed") {
        capturePrivateRouteEvent(`checkout_${search.checkout}`, {
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
      await supabase.auth.refreshSession();
      // The refreshed JWT carries the post-checkout billing claims; cached
      // account-session data is stale until it re-reads the session.
      void queryClient.invalidateQueries({ queryKey: accountSessionQueryKey });
    };

    void syncBillingAnalytics();
  }, [
    queryClient,
    search.checkout,
    search.checkout_type,
    search.scheme,
    search.source,
    search.success,
    search.trial,
  ]);

  useEffect(() => {
    const syncHash = () => setHash(window.location.hash);
    syncHash();
    window.addEventListener("hashchange", syncHash);
    return () => window.removeEventListener("hashchange", syncHash);
  }, []);

  useEffect(() => {
    const sectionId = hash.replace(/^#/, "");
    if (!sectionId || accountTabForSection(sectionId) !== activeTab) {
      return;
    }

    document.getElementById(sectionId)?.scrollIntoView({ block: "start" });
  }, [activeTab, hash]);

  const selectTab = (tabId: AccountTabId) => {
    setHash("");
    setOptimisticTab(tabId);
    void navigate({
      search: (prev) => ({
        ...prev,
        tab: tabId === "account" ? undefined : tabId,
      }),
      // Empty string is treated as omitted and would keep the current hash.
      hash: () => "",
      replace: true,
    }).finally(() => {
      setOptimisticTab((current) => (current === tabId ? null : current));
    });
  };

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

        <div className="mt-10 md:mt-12 lg:mt-16">
          <div className="sticky top-0 z-10 -mx-5 border-b border-[#ede7dc] bg-white px-5 py-3 md:-mx-8 md:px-8">
            <AccountTabs
              activeId={activeTab}
              onSelect={selectTab}
              onPreload={preloadAccountTab}
            />
          </div>

          <div
            role="tabpanel"
            id={`account-tabpanel-${activeTab}`}
            aria-labelledby={`account-tab-${activeTab}`}
            className="mt-10 flex min-w-0 flex-col gap-14"
          >
            <Suspense fallback={<AccountTabFallback tabId={activeTab} />}>
              {sectionsForAccountTab(activeTab).map((section) => (
                <AccountSection key={section.id} id={section.id}>
                  <AccountSectionBody
                    id={section.id}
                    email={user?.email}
                    perk={search.perk}
                    referralIneligible={search.referral === "ineligible"}
                  />
                </AccountSection>
              ))}
            </Suspense>
          </div>
        </div>
      </div>
    </main>
  );
}

function AccountTabFallback({ tabId }: { tabId: AccountTabId }) {
  return (
    <>
      <span role="status" className="sr-only">
        Loading account sections...
      </span>
      {sectionsForAccountTab(tabId).map((section) => (
        <section key={section.id} aria-hidden="true">
          <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
            {section.label}
          </h2>
          <div className="mt-6 h-28 animate-pulse rounded-[24px] border border-[#e5ddcf] bg-[#faf8f4]" />
        </section>
      ))}
    </>
  );
}

function AccountSection({
  id,
  children,
}: {
  id: AccountSectionId;
  children: React.ReactNode;
}) {
  const title = ACCOUNT_SECTIONS.find((section) => section.id === id)?.label;

  return (
    <section ref={scrollHashSectionIntoView} id={id} className="scroll-mt-20">
      {id !== "shares" && (
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          {title}
        </h2>
      )}
      <div className={id === "shares" ? undefined : "mt-6"}>{children}</div>
    </section>
  );
}

function AccountSectionBody({
  id,
  email,
  perk,
  referralIneligible,
}: {
  id: AccountSectionId;
  email?: string;
  perk?: "applied" | "claimed" | "invalid";
  referralIneligible: boolean;
}) {
  switch (id) {
    case "profile":
      return <ProfileInfoSection email={email} />;
    case "plan":
      return <PlanSection perk={perk} />;
    case "referrals":
      return <ReferralSection ineligible={referralIneligible} />;
    case "integrations":
      return <IntegrationsSection />;
    case "devices":
      return <DevicesSection />;
    case "shares":
      return <SharedNotesSection />;
    case "api-keys":
      return <ApiKeysSection />;
    case "session":
      return <AccountAccessSection />;
    case "danger":
      return <DangerAreaSection />;
  }
}
