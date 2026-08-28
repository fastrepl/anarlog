import * as stylex from "@stylexjs/stylex";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { jwtDecode } from "jwt-decode";
import { lazy, Suspense, useEffect, useState } from "react";
import { z } from "zod";

import { fonts, media } from "@anlg/design-system/tokens.stylex";
import { deriveBillingInfo, type SupabaseJwtPayload } from "@anlg/supabase";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import { useAnalytics } from "@/hooks/use-posthog";
import {
  ACCOUNT_SECTIONS,
  type AccountSectionId,
  type AccountTabId,
  accountTabForSection,
  resolveAccountTab,
  sectionsForAccountTab,
} from "@/lib/account-tabs";
import { checkoutSourceSchema } from "@/lib/checkout-source";

import { AccountTabs } from "./-account-nav";
import { accountSessionQueryKey } from "./-account-session";

const pulse = stylex.keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.5 },
});

const styles = stylex.create({
  style1: {
    minHeight: "100vh",
    backgroundColor: "#fff",
    color: "#181613",
  },
  style2: {
    marginInline: "auto",
    width: "100%",
    maxWidth: "700px",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
    },
    paddingBottom: {
      default: "4rem",
      "@media (width >= 48rem)": "6rem",
    },
  },
  style3: {
    display: "inline-flex",
  },
  style4: {
    height: "2rem",
    width: "auto",
  },
  style5: {
    marginTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
  },
  style6: {
    fontFamily: fonts.hand,
    fontSize: "1.5rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#756b5d",
  },
  style7: {
    marginTop: "1rem",
    fontFamily: fonts.hand,
    fontSize: {
      default: "3rem",
      "@media (width >= 48rem)": "3.75rem",
    },
    lineHeight: {
      default: 0.98,
      "@media (width >= 48rem)": 1,
    },
    fontWeight: 600,
    letterSpacing: 0,
    textWrap: "balance",
  },
  style8: {
    backgroundColor: "#fff0b3",
    paddingInline: ".25rem",
    color: "#181613",
  },
  style9: {
    marginTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
      "@media (width >= 64rem)": "4rem",
    },
  },
  style10: {
    position: "sticky",
    top: 0,
    zIndex: 10,
    marginInline: {
      default: "-1.25rem",
      "@media (width >= 48rem)": "-2rem",
    },
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: "#ede7dc",
    backgroundColor: "#fff",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingBlock: ".75rem",
  },
  style11: {
    marginTop: "2.5rem",
    display: "flex",
    minWidth: 0,
    flexDirection: "column",
    gap: "3.5rem",
  },
  style12: {
    clipPath: "inset(50%)",
    whiteSpace: "nowrap",
    borderWidth: 0,
    width: "1px",
    height: "1px",
    margin: "-1px",
    padding: 0,
    position: "absolute",
    overflow: "hidden",
  },
  style13: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#756b5d",
  },
  style14: {
    marginTop: "1.5rem",
    height: "7rem",
    animationDuration: "2s",
    animationTimingFunction: "cubic-bezier(.4, 0, .6, 1)",
    animationIterationCount: "infinite",
    animationName: {
      default: pulse,
      [media.reducedMotion]: "none",
    },
    borderRadius: "24px",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e5ddcf",
    backgroundColor: "#faf8f4",
  },
  style15: {
    scrollMarginTop: "5rem",
  },
  sectionBody: {
    marginTop: "1.5rem",
  },
});
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
  loadApiKeysSection().then((module) => ({
    default: module.ApiKeysSection,
  })),
);
const DangerAreaSection = lazy(() =>
  loadDangerAreaSection().then((module) => ({
    default: module.DangerAreaSection,
  })),
);
const DevicesSection = lazy(() =>
  loadDevicesSection().then((module) => ({
    default: module.DevicesSection,
  })),
);
const IntegrationsSection = lazy(() =>
  loadIntegrationsSection().then((module) => ({
    default: module.IntegrationsSection,
  })),
);
const PlanSection = lazy(() =>
  loadPlanSection().then((module) => ({
    default: module.PlanSection,
  })),
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
    element.scrollIntoView({
      block: "start",
    });
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
  loader: async ({ context }) => ({
    user: context.user,
  }),
});
function Component() {
  const { user } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({
    from: Route.fullPath,
  });
  const { identify: identifyPosthog, track } = useAnalytics();
  const queryClient = useQueryClient();
  const [hash, setHash] = useState("");
  const [optimisticTab, setOptimisticTab] = useState<AccountTabId | null>(null);
  const routeTab = resolveAccountTab({
    tab: search.tab,
    hash,
  });
  const activeTab = optimisticTab ?? routeTab;
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
      void queryClient.invalidateQueries({
        queryKey: accountSessionQueryKey,
      });
      const accessToken = data.session?.access_token;
      const userId = data.session?.user.id;
      if (!accessToken || !userId) {
        return;
      }
      const billing = deriveBillingInfo(
        jwtDecode<SupabaseJwtPayload>(accessToken),
      );
      identifyPosthog(userId, {
        ...(data.session?.user.email
          ? {
              email: data.session.user.email,
            }
          : {}),
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
    document.getElementById(sectionId)?.scrollIntoView({
      block: "start",
    });
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
    <main {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <Link to="/" aria-label="Anarlog home" {...stylex.props(styles.style3)}>
          <AnarlogLogo sx={styles.style4} />
        </Link>

        <header {...stylex.props(styles.style5)}>
          <p {...stylex.props(styles.style6)}>Your account</p>
          <h1 {...stylex.props(styles.style7)}>
            Welcome back,{" "}
            <mark {...stylex.props(styles.style8)}>
              {user?.email?.split("@")[0] || "Guest"}
            </mark>
          </h1>
        </header>

        <div {...stylex.props(styles.style9)}>
          <div {...stylex.props(styles.style10)}>
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
            {...stylex.props(styles.style11)}
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
      <span role="status" {...stylex.props(styles.style12)}>
        Loading account sections...
      </span>
      {sectionsForAccountTab(tabId).map((section) => (
        <section key={section.id} aria-hidden="true">
          <h2 {...stylex.props(styles.style13)}>{section.label}</h2>
          <div {...stylex.props(styles.style14)} />
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
    <section
      ref={scrollHashSectionIntoView}
      id={id}
      {...stylex.props(styles.style15)}
    >
      {id !== "shares" && <h2 {...stylex.props(styles.style13)}>{title}</h2>}
      <div {...stylex.props(id !== "shares" && styles.sectionBody)}>
        {children}
      </div>
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
