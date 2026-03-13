import { useMutation, useQuery } from "@tanstack/react-query";
import {
  Brain,
  Cloud,
  ExternalLinkIcon,
  Puzzle,
  Sparkle,
  Sparkles,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useCallback, useEffect, useState } from "react";

import {
  canStartTrial as canStartTrialApi,
  startTrial,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { type SubscriptionStatus } from "@hypr/supabase";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Marquee } from "@hypr/ui/components/ui/marquee";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { configureProSettings } from "~/shared/config/configure-pro-settings";
import * as settings from "~/store/tinybase/store/settings";

const WEB_APP_BASE_URL = env.VITE_APP_URL ?? "http://localhost:3000";
const ACCOUNT_FEATURES = [
  {
    label: "Pro AI models",
    icon: Sparkle,
    comingSoon: false,
    benefit: "Use premium hosted models without managing API keys.",
    accent: {
      icon: "text-blue-900",
      label: "text-blue-950",
    },
  },
  {
    label: "Cloud sync",
    icon: Cloud,
    comingSoon: true,
    benefit: "Keep your notes available across devices with selective sync.",
    accent: {
      icon: "text-sky-700",
      label: "text-sky-900",
    },
  },
  {
    label: "Memory",
    icon: Brain,
    comingSoon: true,
    benefit: "Make Char more personal with saved preferences and context.",
    accent: {
      icon: "text-yellow-700",
      label: "text-yellow-900",
    },
  },
  {
    label: "Integrations",
    icon: Puzzle,
    comingSoon: true,
    benefit: "Connect tools and pull context into Char with less busywork.",
    accent: {
      icon: "text-purple-700",
      label: "text-purple-900",
    },
  },
] as const;

function PlanStatus({
  subscriptionStatus,
  trialDaysRemaining,
}: {
  subscriptionStatus: SubscriptionStatus | null;
  trialDaysRemaining: number | null;
}) {
  if (!subscriptionStatus) {
    return <span className="text-neutral-500">FREE</span>;
  }

  switch (subscriptionStatus) {
    case "active":
      return (
        <span className="inline-flex items-center gap-1 font-medium text-neutral-800">
          <Sparkles size={13} className="text-neutral-500" />
          PRO
        </span>
      );

    case "trialing": {
      const isUrgent = trialDaysRemaining !== null && trialDaysRemaining <= 3;
      let trialText = null;
      if (trialDaysRemaining !== null) {
        if (trialDaysRemaining === 0) {
          trialText = "Trial ends today";
        } else if (trialDaysRemaining === 1) {
          trialText = "Trial ends tomorrow";
        } else {
          trialText = `${trialDaysRemaining} days left`;
        }
      }
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1 font-medium text-neutral-800">
            <Sparkles size={13} className="text-neutral-500" />
            PRO
          </span>
          {trialText && (
            <span
              className={cn(["text-neutral-500", isUrgent && "text-amber-600"])}
            >
              ({trialText})
            </span>
          )}
        </span>
      );
    }

    case "past_due":
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1 font-medium text-neutral-800">
            <Sparkles size={13} className="text-neutral-500" />
            PRO
          </span>
          <span className="text-amber-600">(Payment issue)</span>
        </span>
      );

    case "unpaid":
      return <span className="text-amber-600">Payment failed</span>;

    case "canceled":
      return <span className="text-neutral-500">Canceled</span>;

    case "incomplete":
      return <span className="text-neutral-500">Setup incomplete</span>;

    case "incomplete_expired":
      return <span className="text-neutral-500">Expired</span>;

    case "paused":
      return <span className="text-neutral-500">Paused</span>;

    default:
      return <span className="text-neutral-500">FREE</span>;
  }
}

export function AccountSettings() {
  const auth = useAuth();
  const { subscriptionStatus, trialDaysRemaining } = useBillingAccess();

  const isAuthenticated = !!auth?.session;
  const [isPending, setIsPending] = useState(false);
  const [callbackUrl, setCallbackUrl] = useState("");

  useEffect(() => {
    if (isAuthenticated) {
      setIsPending(false);
    }
  }, [isAuthenticated]);

  const handleOpenAccount = useCallback(() => {
    void openerCommands.openUrl(`${WEB_APP_BASE_URL}/app/account`, null);
  }, []);

  const handleSignIn = useCallback(async () => {
    setIsPending(true);
    try {
      await auth?.signIn();
    } catch {
      setIsPending(false);
    }
  }, [auth]);

  const handleSignOut = useCallback(async () => {
    void analyticsCommands.event({
      event: "user_signed_out",
    });
    void analyticsCommands.setProperties({
      set: {
        is_signed_up: false,
      },
    });

    await auth?.signOut();
  }, [auth]);

  const handleRefreshPlan = useCallback(async () => {
    await auth?.refreshSession();
  }, [auth]);

  if (!isAuthenticated) {
    if (isPending) {
      return (
        <div>
          <h2 className="mb-4 font-serif text-lg font-semibold">Account</h2>
          <Container
            title="Finish sign-in"
            description="Complete the sign-in flow in your browser, then come back here if Char does not reconnect automatically."
            action={
              <Button onClick={handleSignIn} variant="outline">
                Reopen sign-in page
              </Button>
            }
          >
            <div className="flex flex-col gap-3">
              <p className="text-xs text-neutral-500">
                Having trouble? Paste the callback URL manually.
              </p>
              <div className="flex flex-col gap-2 sm:flex-row">
                <Input
                  type="text"
                  className="flex-1 font-mono text-xs"
                  placeholder="hyprnote://deeplink/auth?access_token=..."
                  value={callbackUrl}
                  onChange={(e) => setCallbackUrl(e.target.value)}
                />
                <Button
                  onClick={() => auth?.handleAuthCallback(callbackUrl)}
                  disabled={!callbackUrl}
                >
                  Submit
                </Button>
              </div>
            </div>
          </Container>
        </div>
      );
    }

    return (
      <div>
        <section className="border-b border-neutral-200 pb-4">
          <div className="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 flex-1 flex-col gap-4">
              <h2 className="font-serif text-lg font-semibold">Account</h2>
              <div className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">Sign in to Char</h3>
                <div className="text-sm text-neutral-600">
                  Sign in to unlock powerful AI models, sync across devices,
                  personalization, and workflow integrations.
                </div>
              </div>
              <button
                type="button"
                onClick={handleSignIn}
                className="h-10 w-fit rounded-full border-2 border-stone-600 bg-stone-800 px-6 text-sm font-medium text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200 hover:bg-stone-700"
              >
                Get started
              </button>
            </div>
            <div className="shrink-0">
              <FeatureSpotlight />
            </div>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div>
      <h2 className="mb-4 font-serif text-lg font-semibold">Account</h2>
      <div className="flex flex-col gap-4">
        <Container
          title="Your Account"
          description="Redirect to the web app to manage your account."
          action={
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                onClick={handleOpenAccount}
                className="flex flex-row gap-1.5"
              >
                <span className="text-sm">Open</span>
                <ExternalLinkIcon className="text-neutral-600" size={12} />
              </Button>
              <Button variant="outline" onClick={handleSignOut}>
                Sign out
              </Button>
            </div>
          }
        />

        <Container
          title="Plan & Billing"
          description={
            <span>
              Your current plan is{" "}
              <PlanStatus
                subscriptionStatus={subscriptionStatus}
                trialDaysRemaining={trialDaysRemaining}
              />
            </span>
          }
          action={<BillingButton />}
        >
          <div className="flex items-center gap-1 text-sm text-neutral-600">
            {auth?.isRefreshingSession ? (
              <>
                <Spinner size={14} />
                <span>Refreshing plan status...</span>
              </>
            ) : (
              <>
                Click{" "}
                <span
                  onClick={handleRefreshPlan}
                  className="text-primary cursor-pointer underline"
                >
                  here
                </span>
                <span className="text-neutral-600">
                  {" "}
                  to refresh plan status.
                </span>
              </>
            )}
          </div>
        </Container>
        <Container
          title="Account features"
          description="More account-linked features are rolling out soon."
        >
          <FeatureGrid />
        </Container>
      </div>
    </div>
  );
}

function FeatureGrid() {
  const [shouldMarquee, setShouldMarquee] = useState(false);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [measure, setMeasure] = useState<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!container || !measure) {
      return;
    }

    const update = () => {
      setShouldMarquee(measure.scrollWidth > container.clientWidth);
    };

    update();

    const observer = new ResizeObserver(update);
    observer.observe(container);
    observer.observe(measure);

    return () => observer.disconnect();
  }, [container, measure]);

  return (
    <div className="relative">
      <div ref={setContainer} className="overflow-hidden">
        {shouldMarquee ? (
          <Marquee className="p-0 [--duration:24s] [--gap:0.75rem]">
            <FeatureItems />
          </Marquee>
        ) : (
          <div className="flex gap-3">
            <FeatureItems />
          </div>
        )}
      </div>
      <div className="pointer-events-none absolute inset-0 -z-10 overflow-hidden opacity-0">
        <div ref={setMeasure} className="flex w-max gap-3">
          <FeatureItems />
        </div>
      </div>
    </div>
  );
}

function FeatureItems() {
  return ACCOUNT_FEATURES.map(({ label, icon: Icon, comingSoon }) => (
    <div
      key={label}
      className="flex min-w-[150px] items-center justify-between gap-3 rounded-lg border border-neutral-200 bg-white px-3 py-2.5"
    >
      <div className="flex min-w-0 items-center gap-2">
        <Icon className="h-4 w-4 shrink-0 text-neutral-500" />
        <p className="truncate text-sm text-neutral-700">{label}</p>
      </div>
      {comingSoon && (
        <span className="shrink-0 rounded bg-neutral-100 px-1.5 py-0.5 text-[10px] text-neutral-500">
          Soon
        </span>
      )}
    </div>
  ));
}

function FeatureSpotlight() {
  const [activeIndex, setActiveIndex] = useState(0);
  const [isPaused, setIsPaused] = useState(false);

  useEffect(() => {
    if (isPaused) {
      return;
    }

    const interval = window.setInterval(() => {
      setActiveIndex((current) => (current + 1) % ACCOUNT_FEATURES.length);
    }, 2200);

    return () => window.clearInterval(interval);
  }, [isPaused]);

  const {
    label,
    icon: Icon,
    comingSoon,
    benefit,
    accent,
  } = ACCOUNT_FEATURES[activeIndex];

  return (
    <div className="group relative flex w-full max-w-[220px] min-w-[180px] items-center justify-center p-2">
      <div className="relative min-h-[88px] w-full">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={label}
            initial={{ opacity: 0, y: 10, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.96 }}
            transition={{ duration: 0.22, ease: "easeOut" }}
            className="absolute inset-0"
          >
            <motion.button
              type="button"
              initial={{ opacity: 0, y: 10, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -10, scale: 0.96 }}
              transition={{ duration: 0.22, ease: "easeOut" }}
              onMouseEnter={() => setIsPaused(true)}
              onMouseLeave={() => setIsPaused(false)}
              onFocus={() => setIsPaused(true)}
              onBlur={() => setIsPaused(false)}
              className="absolute inset-0 flex flex-col items-center justify-center gap-3 text-center outline-none"
              aria-label={`${label}. ${benefit}`}
            >
              <motion.div
                initial={{ scale: 0.86, rotate: -10 }}
                animate={{
                  scale: isPaused ? 1.08 : 1,
                  rotate: 0,
                  y: isPaused ? -2 : 0,
                }}
                exit={{ scale: 0.9, rotate: 10 }}
                transition={{ duration: 0.28, ease: "easeOut" }}
                className="flex h-12 w-12 items-center justify-center"
              >
                <motion.div
                  animate={
                    isPaused ? { rotate: [0, -4, 4, 0] } : { y: [0, -2, 0] }
                  }
                  transition={{
                    duration: isPaused ? 0.9 : 1.6,
                    repeat: Number.POSITIVE_INFINITY,
                    ease: "easeInOut",
                  }}
                >
                  <Icon className={cn(["h-5 w-5", accent.icon])} />
                </motion.div>
              </motion.div>
              <p className={cn(["text-sm font-medium", accent.label])}>
                {label}
              </p>
            </motion.button>
          </motion.div>
        </AnimatePresence>
      </div>
      <AnimatePresence>
        {isPaused ? (
          <motion.div
            initial={{ opacity: 0, y: 8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 6, scale: 0.98 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="pointer-events-none absolute top-full right-0 z-10 mt-1.5 w-[208px] rounded-xl border border-neutral-200 bg-white/95 p-2.5 text-left shadow-lg backdrop-blur-sm"
          >
            <div className="flex items-center justify-between gap-3">
              <p className={cn(["text-sm font-medium", accent.label])}>
                {label}
              </p>
              {comingSoon ? (
                <span className="text-xs text-neutral-400">Soon</span>
              ) : null}
            </div>
            <p className="mt-1 text-xs leading-[1.45] text-neutral-600">
              {benefit}
            </p>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function BillingButton() {
  const auth = useAuth();
  const { isPro } = useBillingAccess();
  const store = settings.UI.useStore(settings.STORE_ID);

  const canTrialQuery = useQuery({
    enabled: !!auth?.session && !isPro,
    queryKey: [auth?.session?.user.id ?? "", "canStartTrial"],
    queryFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        return false;
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await canStartTrialApi({ client });
      if (error) {
        throw error;
      }

      return data?.canStartTrial ?? false;
    },
  });

  const startTrialMutation = useMutation({
    mutationFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        throw new Error("Not authenticated");
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { error } = await startTrial({
        client,
        query: { interval: "monthly" },
      });
      if (error) {
        throw error;
      }

      await new Promise((resolve) => setTimeout(resolve, 3000));
    },
    onSuccess: async () => {
      if (store) {
        configureProSettings(store);
      }
      await auth?.refreshSession();
    },
  });

  const handleProUpgrade = useCallback(() => {
    void analyticsCommands.event({
      event: "upgrade_clicked",
      plan: "pro",
    });
    void openerCommands.openUrl(
      `${WEB_APP_BASE_URL}/app/checkout?period=monthly`,
      null,
    );
  }, []);

  const handleOpenAccount = useCallback(() => {
    void openerCommands.openUrl(`${WEB_APP_BASE_URL}/app/account`, null);
  }, []);

  if (isPro) {
    return (
      <Button variant="outline" onClick={handleOpenAccount} className="gap-1.5">
        <span className="text-sm">Manage</span>
        <ExternalLinkIcon className="text-neutral-600" size={12} />
      </Button>
    );
  }

  if (canTrialQuery.data) {
    return (
      <Button
        variant="outline"
        onClick={() => startTrialMutation.mutate()}
        disabled={startTrialMutation.isPending}
      >
        <span>Start Pro Trial</span>
      </Button>
    );
  }

  return (
    <Button variant="outline" onClick={handleProUpgrade} className="gap-1.5">
      <span>Upgrade to Pro</span>
      <ExternalLinkIcon className="text-neutral-600" size={12} />
    </Button>
  );
}

function Container({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <section className="border-b border-neutral-200 pb-4 last:border-b-0">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <h3 className="text-sm font-medium">{title}</h3>
          {description && (
            <div className="text-sm text-neutral-600">{description}</div>
          )}
        </div>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      {children ? <div className="mt-4">{children}</div> : null}
    </section>
  );
}
